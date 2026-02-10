use rustc_hash::FxHashMap;
use std::thread;

use crossbeam_channel::{Receiver, Sender, bounded};
use lsp_types::Uri;
use salsa::Setter;

use sneklsp_db::{Database, File, FileAnalysis, file_line_index, parse_file_recovering};
use sneklsp_text::LineIndex;
use sneklsp_vfs::FileId;

#[derive(Debug)]
pub struct AnalysisRequest {
    pub uri: Uri,
    pub file_id: FileId,
    pub version: i32,
}

pub struct AnalysisResult {
    pub uri: Uri,
    pub file_id: FileId,
    pub version: i32,
    pub analysis: FileAnalysis,
}

pub struct AnalysisHost {
    db: Database,
    file_map: FxHashMap<FileId, File>,
    request_tx: Sender<AnalysisRequest>,
    result_rx: Receiver<AnalysisResult>,
    _handle: thread::JoinHandle<()>,
}

impl AnalysisHost {
    pub fn new() -> Self {
        let (request_tx, request_rx) = bounded::<AnalysisRequest>(16);
        let (result_tx, result_rx) = bounded::<AnalysisResult>(16);

        // TODO: handle when the main thread will call analyze_sync and post results
        let handle = thread::Builder::new()
            .name("sneklsp-analysis".to_string())
            .spawn(move || {
                tracing::info!("analysis thread started");
                while let Ok(_req) = request_rx.recv() {
                    // queries run asynchronously on snapshot
                }
                tracing::info!("analysis thread shutting down");
            })
            .expect("failed to spawn analysis thread");

        Self {
            db: Database::default(),
            file_map: FxHashMap::default(),
            request_tx,
            result_rx,
            _handle: handle,
        }
    }

    pub fn set_file_content(&mut self, file_id: FileId, path: &str, content: String) {
        if let Some(&file) = self.file_map.get(&file_id) {
            file.set_content(&mut self.db).to(content);
        } else {
            let file = File::new(&self.db, path.to_string(), content);
            self.file_map.insert(file_id, file);
        }
    }

    pub fn analyze_file(&self, file_id: FileId) -> Option<&FileAnalysis> {
        let file = self.file_map.get(&file_id)?;
        Some(parse_file_recovering(&self.db, *file))
    }

    pub fn line_index(&self, file_id: FileId) -> Option<&LineIndex> {
        let file = self.file_map.get(&file_id)?;
        Some(file_line_index(&self.db, *file))
    }

    #[inline]
    pub fn has_file(&self, file_id: FileId) -> bool {
        self.file_map.contains_key(&file_id)
    }

    #[inline]
    pub fn db(&self) -> &Database {
        &self.db
    }

    #[inline]
    pub fn results(&self) -> &Receiver<AnalysisResult> {
        &self.result_rx
    }
}

impl Default for AnalysisHost {
    fn default() -> Self {
        Self::new()
    }
}
