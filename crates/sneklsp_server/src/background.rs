use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender, bounded};
use lsp_types::Uri;

use sneklsp_ast::AstArena;
use sneklsp_index::ModuleIndex;
use sneklsp_parser::ParseError;
use sneklsp_text::LineIndex;

#[derive(Debug)]
pub struct ParseRequest {
    pub uri: Uri,
    pub content: String,
    pub version: i32,
    pub request_id: u64,
}

pub struct ParseResult {
    pub uri: Uri,
    pub version: i32,
    pub errors: Vec<ParseError>,
    pub line_index: LineIndex,
    pub request_id: u64,
    pub content: String,
    pub index: Option<IndexedModule>,
}

impl std::fmt::Debug for ParseResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParseResult")
            .field("uri", &self.uri)
            .field("version", &self.version)
            .field("errors", &self.errors)
            .field("has_index", &self.index.is_some())
            .finish()
    }
}

pub struct IndexedModule {
    pub symbols: Vec<IndexedSymbol>,
    pub scopes: Vec<IndexedScope>,
    pub references: Vec<IndexedReference>,
}

#[derive(Debug)]
pub struct IndexedSymbol {
    pub id: u32,
    pub name: String,
    pub kind: sneklsp_index::SymbolKind,
    pub range: sneklsp_text::TextRange,
    pub selection_range: sneklsp_text::TextRange,
    pub scope: u32,
    pub visibility: sneklsp_index::Visibility,
}

#[derive(Debug)]
pub struct IndexedScope {
    pub id: u32,
    pub kind: sneklsp_index::ScopeKind,
    pub parent: Option<u32>,
    pub range: sneklsp_text::TextRange,
    pub symbols: Vec<u32>,
    pub children: Vec<u32>,
}

#[derive(Debug)]
pub struct IndexedReference {
    pub id: u32,
    pub name: String,
    pub range: sneklsp_text::TextRange,
    pub resolved: Option<u32>,
}

pub struct BackgroundParser {
    request_tx: Sender<ParseRequest>,
    result_rx: Receiver<ParseResult>,
    _handle: JoinHandle<()>,
    next_request_id: AtomicU64,
}

impl BackgroundParser {
    pub fn new() -> Self {
        let (request_tx, request_rx) = bounded::<ParseRequest>(4);
        let (result_tx, result_rx) = bounded::<ParseResult>(16);

        let handle = thread::Builder::new()
            .name("sneklsp-parser".to_string())
            .spawn(move || {
                Self::parser_thread(request_rx, result_tx);
            })
            .expect("failed to spawn parser thread");

        Self {
            request_tx,
            result_rx,
            _handle: handle,
            next_request_id: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn parse(&self, uri: Uri, content: String, version: i32) -> Option<u64> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let request = ParseRequest {
            uri,
            content,
            version,
            request_id,
        };

        match self.request_tx.try_send(request) {
            Ok(()) => {
                tracing::debug!(request_id, "submitted parse request");
                Some(request_id)
            }
            Err(crossbeam_channel::TrySendError::Full(req)) => {
                tracing::debug!(request_id, "parse queue full. dropping oldest");
                let _ = self.request_tx.try_send(req);
                Some(request_id)
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                tracing::error!("parser thread disconnected");
                None
            }
        }
    }

    #[inline]
    pub fn results(&self) -> &Receiver<ParseResult> {
        &self.result_rx
    }

    fn parser_thread(request_rx: Receiver<ParseRequest>, result_tx: Sender<ParseResult>) {
        tracing::info!("parser thread started");

        while let Ok(request) = request_rx.recv() {
            let start = std::time::Instant::now();

            tracing::debug!(
                request_id = request.request_id,
                ?request.uri,
                "parsing document"
            );

            let line_index = LineIndex::new(&request.content);

            let arena = AstArena::new();
            let (errors, index) = match sneklsp_parser::parse(&request.content, &arena) {
                Ok(module) => {
                    let idx = sneklsp_index::index_module(&request.content, &module);
                    let owned_index = Self::to_owned_index(&idx);
                    (Vec::new(), Some(owned_index))
                }
                Err(_) => {
                    let errors = sneklsp_parser::parse_and_collect_errors(&request.content);
                    (errors, None)
                }
            };

            let elapsed = start.elapsed();
            tracing::debug!(
                request_id = request.request_id,
                ?elapsed,
                error_count = errors.len(),
                "parsing complete"
            );

            let result = ParseResult {
                uri: request.uri,
                version: request.version,
                errors,
                line_index,
                request_id: request.request_id,
                content: request.content,
                index,
            };

            if result_tx.try_send(result).is_err() {
                tracing::warn!("result channel full. dropping parse result");
            }

            tracing::info!("parser thread shutting down");
        }
    }

    fn to_owned_index(index: &ModuleIndex<'_>) -> IndexedModule {
        let symbols = index
            .symbols()
            .iter()
            .map(|s| IndexedSymbol {
                id: s.id.as_u32(),
                name: s.name.to_string(),
                kind: s.kind,
                range: s.range,
                selection_range: s.selection_range,
                scope: s.scope.as_u32(),
                visibility: s.visibility,
            })
            .collect();

        let scopes = index
            .scopes()
            .iter()
            .map(|s| IndexedScope {
                id: s.id.as_u32(),
                kind: s.kind,
                parent: s.parent.map(|p| p.as_u32()),
                range: s.range,
                symbols: s.symbols.iter().map(|id| id.as_u32()).collect(),
                children: s.children.iter().map(|id| id.as_u32()).collect(),
            })
            .collect();

        let references = index
            .references()
            .iter()
            .map(|r| IndexedReference {
                id: r.id.as_u32(),
                name: r.name.to_string(),
                range: r.range,
                resolved: r.resolved.map(|id| id.as_u32()),
            })
            .collect();

        IndexedModule {
            symbols,
            scopes,
            references,
        }
    }
}

impl Default for BackgroundParser {
    fn default() -> Self {
        Self::new()
    }
}
