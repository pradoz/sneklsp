use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender, bounded};
use lsp_types::Uri;

use crate::document::EditRecord;
use sneklsp_ast::AstArena;
use sneklsp_index::ModuleIndex;
use sneklsp_lexer::{TextEdit as LexerEdit, Token, relex};
use sneklsp_parser::ParseError;
use sneklsp_text::LineIndex;

#[derive(Debug)]
pub struct ParseRequest {
    pub uri: Uri,
    pub content: String,
    pub version: i32,
    pub request_id: u64,
    pub edits: Vec<EditRecord>,
    pub has_prior_index: bool,
    pub old_tokens: Option<Vec<Token>>,
    pub old_content: Option<String>,
}

pub struct ParseResult {
    pub uri: Uri,
    pub version: i32,
    pub errors: Vec<ParseError>,
    pub line_index: LineIndex,
    pub request_id: u64,
    pub content: String,
    pub index: Option<IndexedModule>,
    pub tokens: Vec<Token>,
}

impl std::fmt::Debug for ParseResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParseResult")
            .field("uri", &self.uri)
            .field("version", &self.version)
            .field("errors", &self.errors)
            .field("has_index", &self.index.is_some())
            .field("token_count", &self.tokens.len())
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

trait IntoOwned<T> {
    fn into_owned(&self) -> T;
}

impl IntoOwned<IndexedSymbol> for sneklsp_index::Symbol<'_> {
    fn into_owned(&self) -> IndexedSymbol {
        IndexedSymbol {
            id: self.id.as_u32(),
            name: self.name.to_string(),
            kind: self.kind,
            range: self.range,
            selection_range: self.selection_range,
            scope: self.scope.as_u32(),
            visibility: self.visibility,
        }
    }
}

impl IntoOwned<IndexedScope> for sneklsp_index::Scope {
    fn into_owned(&self) -> IndexedScope {
        IndexedScope {
            id: self.id.as_u32(),
            kind: self.kind,
            parent: self.parent.map(|p| p.as_u32()),
            range: self.range,
            symbols: self.symbols.iter().map(|id| id.as_u32()).collect(),
            children: self.children.iter().map(|id| id.as_u32()).collect(),
        }
    }
}

impl IntoOwned<IndexedReference> for sneklsp_index::Reference<'_> {
    fn into_owned(&self) -> IndexedReference {
        IndexedReference {
            id: self.id.as_u32(),
            name: self.name.to_string(),
            range: self.range,
            resolved: self.resolved.map(|id| id.as_u32()),
        }
    }
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
    pub fn parse(
        &self,
        uri: Uri,
        content: String,
        version: i32,
        edits: Vec<EditRecord>,
        has_prior_index: bool,
        old_tokens: Option<Vec<Token>>,
        old_content: Option<String>,
    ) -> Option<u64> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let request = ParseRequest {
            uri,
            content,
            version,
            request_id,
            edits,
            has_prior_index,
            old_tokens,
            old_content,
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
                edit_count = request.edits.len(),
                has_prior_index = request.has_prior_index,
                has_old_tokens = request.old_tokens.is_some(),
                "parsing document"
            );

            let tokens = Self::do_lex(
                &request.content,
                &request.edits,
                request.old_tokens.as_deref(),
                request.old_content.as_deref(),
            );

            let line_index = LineIndex::new(&request.content);

            let arena_size = (request.content.len() * 50).max(4096);
            let arena = AstArena::with_capacity(arena_size);
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
                token_count = tokens.len(),
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
                tokens,
            };

            if result_tx.try_send(result).is_err() {
                tracing::warn!("result channel full. dropping parse result");
            }
        }
        tracing::info!("parser thread shutting down");
    }

    fn do_lex(
        content: &str,
        edits: &[EditRecord],
        old_tokens: Option<&[Token]>,
        old_content: Option<&str>,
    ) -> Vec<Token> {
        match (old_tokens, old_content, edits) {
            (Some(old_toks), Some(old_src), [edit]) => {
                let lexer_edit = LexerEdit::new(edit.range, edit.new_len);
                let result = relex(old_toks, old_src, content, lexer_edit);

                tracing::debug!(
                    fully_relexed = result.fully_relexed,
                    "incremental lex"
                );

                result.tokens
            }
            _ => {
                tracing::debug!("full tokenize");
                sneklsp_lexer::tokenize(content)
            }

        }
    }

    fn to_owned_index(index: &ModuleIndex<'_>) -> IndexedModule {
        IndexedModule {
            symbols: index.symbols().iter().map(|s| s.into_owned()).collect(),
            scopes: index.scopes().iter().map(|s| s.into_owned()).collect(),
            references: index.references().iter().map(|r| r.into_owned()).collect(),
        }
    }
}

impl Default for BackgroundParser {
    fn default() -> Self {
        Self::new()
    }
}
