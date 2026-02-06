use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{select, tick};
use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    PublishDiagnostics,
};
use lsp_types::request::{
    Completion, DocumentHighlightRequest, DocumentSymbolRequest, GotoDefinition, HoverRequest, References, Rename, Request as _
};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, OneOf, PublishDiagnosticsParams, SaveOptions,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, Uri,
};

use crate::background::{BackgroundParser, ParseResult};
use crate::debouncer::Debouncer;
use crate::diagnostics::to_diagnostics;
use crate::document::{Document, EditRecord};
use crate::handlers;

pub fn run_server() -> Result<()> {
    tracing::info!("starting sneklsp server");

    let (connection, io_threads) = Connection::stdio();

    // wait for initialize request
    let (id, params) = connection.initialize_start()?;
    let init_params: InitializeParams = serde_json::from_value(params)?;

    tracing::info!("received initialize request");
    tracing::debug!(?init_params);

    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                will_save: Some(false),
                will_save_wait_until: Some(false),
                save: Some(SaveOptions::default().into()),
            },
        )),
        completion_provider: Some(lsp_types::CompletionOptions {
            trigger_characters: Some(vec![".".to_string()]),
            ..Default::default()
        }),
        document_symbol_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        ..Default::default()
    };

    let result = InitializeResult {
        capabilities,
        server_info: Some(ServerInfo {
            name: "sneklsp".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };

    connection.initialize_finish(id, serde_json::to_value(result)?)?;
    tracing::info!("server initialized");

    // server main loop
    let mut server = Server::new(connection);
    server.run()?;

    io_threads.join()?;

    tracing::info!("server shutdown complete");
    Ok(())
}

pub struct DocumentState {
    pub document: Document,
    pub pending_request_id: Option<u64>,
}

struct Server {
    connection: Connection,
    documents: HashMap<Uri, DocumentState>,
    parser: BackgroundParser,
    debouncer: Debouncer,
}

impl Server {
    fn new(connection: Connection) -> Self {
        Self {
            connection,
            documents: HashMap::new(),
            parser: BackgroundParser::new(),
            debouncer: Debouncer::new(),
        }
    }

    fn run(&mut self) -> Result<()> {
        tracing::info!("server main loop starting");

        let ticker = tick(Duration::from_millis(50));

        loop {
            select! {
                // handle incoming LSP requests
                recv(self.connection.receiver) -> msg => {
                    match msg {
                        Ok(Message::Request(req)) => {
                            if self.connection.handle_shutdown(&req)? {
                                tracing::info!("received shutdown request");
                                return Ok(());
                            }
                            self.handle_request(req)?;
                        }
                        Ok(Message::Response(resp)) => {
                            tracing::debug!(?resp, "received response");
                        }
                        Ok(Message::Notification(notif)) => {
                            self.handle_notification(notif)?;
                        }
                        Err(e) => {
                            tracing::error!(?e, "error receiving message");
                            return Err(e.into());
                        }
                    }
                }

                // handle parse results from background thread
                recv(self.parser.results()) -> result => {
                    if let Ok(result) = result {
                        self.handle_parse_result(result);
                    }
                }

                recv(ticker) -> _ => {
                    self.process_debounced();
                }
            }
        }
    }

    fn process_debounced(&mut self) {
        for (uri, version) in self.debouncer.take_ready() {
            if let Some(state) = self.documents.get_mut(&uri) {
                if state.document.version == version {
                    tracing::debug!(?uri, version, "debounce complete. submitting parse");

                    let content = state.document.take_content_for_parse();
                    let edits = state.document.take_edits();
                    let has_prior_index = state.document.index.is_some();

                    let (old_tokens, old_content) =
                        if edits.len() == 1 && state.document.has_tokens() {
                            let old_content = Self::reconstruct_old_content(&content, &edits);
                            (Some(state.document.tokens.clone()), Some(old_content))
                        } else {
                            (None, None)
                        };

                    let request_id = self.parser.parse(
                        uri.clone(),
                        content,
                        version,
                        edits,
                        has_prior_index,
                        old_tokens,
                        old_content,
                    );
                    state.pending_request_id = request_id;
                }
            }
        }
    }

    fn reconstruct_old_content(current: &str, edits: &[EditRecord]) -> String {
        if edits.len() != 1 {
            return current.to_string();
        }

        let edit = &edits[0];
        let start = edit.range.start().to_usize();
        let new_end = start + edit.new_len.to_usize();

        if new_end > current.len() {
            return current.to_string();
        }

        let mut old =
            String::with_capacity(current.len() - edit.new_len.to_usize() + edit.old_content.len());
        old.push_str(&current[..start]);
        old.push_str(&edit.old_content);
        old.push_str(&current[new_end..]);
        old
    }

    fn handle_parse_result(&mut self, result: ParseResult) {
        let ParseResult {
            uri,
            version,
            errors,
            line_index,
            request_id,
            index,
            tokens,
        } = result;

        if let Some(state) = self.documents.get_mut(&uri) {
            // document might have changed since parse was requested
            if state.document.version != version {
                tracing::debug!(
                    ?uri,
                    result_version = version,
                    current_version = state.document.version,
                    "ignoring stale parse result"
                );
                return;
            }

            // ignore if newer parse request is pending
            if let Some(pending_id) = state.pending_request_id {
                if pending_id > request_id {
                    tracing::debug!(
                        ?uri,
                        request_id,
                        pending_id,
                        "ignoring superseded parse result"
                    );
                    return;
                }
            }

            state.document.set_tokens(tokens);
            if let Some(idx) = index {
                state.document.set_index(idx, line_index.clone());
                tracing::debug!(?uri, "index updated");
            }

            state.pending_request_id = None;
        } else {
            tracing::debug!(?uri, "ignoring parse result for closed document");
            return;
        }

        tracing::debug!(
            ?uri,
            version,
            error_count = errors.len(),
            "publishing diagnostics"
        );

        let diagnostics = to_diagnostics(&errors, &line_index);
        self.send_diagnostics(&uri, diagnostics);
    }

    fn handle_request(&mut self, req: Request) -> Result<()> {
        tracing::debug!(?req.method, "handling request");

        match req.method.as_str() {
            Completion::METHOD => {
                let (id, params) = cast_request::<Completion>(req)?;
                let result = handlers::handle_completion(params, &self.documents);
                self.send_response(id, result);
            }

            DocumentSymbolRequest::METHOD => {
                let (id, params) = cast_request::<DocumentSymbolRequest>(req)?;
                let result = handlers::handle_document_symbol(params, &self.documents);
                self.send_response(id, result);
            }

            GotoDefinition::METHOD => {
                let (id, params) = cast_request::<GotoDefinition>(req)?;
                let result = handlers::handle_goto_definition(params, &self.documents);
                self.send_response(id, result);
            }

            HoverRequest::METHOD => {
                let (id, params) = cast_request::<HoverRequest>(req)?;
                let result = handlers::handle_hover(params, &self.documents);
                self.send_response(id, result);
            }

            References::METHOD => {
                let (id, params) = cast_request::<References>(req)?;
                let result = handlers::handle_references(params, &self.documents);
                self.send_response(id, result);
            }

            Rename::METHOD => {
                let (id, params) = cast_request::<Rename>(req)?;
                let result = handlers::handle_rename(params, &self.documents);
                self.send_response(id, result);
            }

            DocumentHighlightRequest::METHOD => {
                let (id, params) = cast_request::<DocumentHighlightRequest>(req)?;
                let result = handlers::handle_document_highlight(params, &self.documents);
                self.send_response(id, result);
            }

            _ => {
                tracing::debug!(method = ?req.method, "unhandled request");
            }
        }

        Ok(())
    }

    fn handle_notification(&mut self, notif: Notification) -> Result<()> {
        tracing::debug!(?notif.method, "handling notification");

        match notif.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params: DidOpenTextDocumentParams = serde_json::from_value(notif.params)?;
                self.on_did_open(params)?;
            }
            DidChangeTextDocument::METHOD => {
                let params: DidChangeTextDocumentParams = serde_json::from_value(notif.params)?;
                self.on_did_change(params)?;
            }
            DidCloseTextDocument::METHOD => {
                let params: DidCloseTextDocumentParams = serde_json::from_value(notif.params)?;
                self.on_did_close(params)?;
            }
            _ => {
                tracing::debug!(?notif.method, "unhandled notification");
            }
        }

        Ok(())
    }

    fn on_did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<()> {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let version = params.text_document.version;

        tracing::info!(?uri, "document openened");

        // submit for background parsing. content is restored when result arrives
        let document = Document::new(String::new(), version);
        let request_id =
            self.parser
                .parse(uri.clone(), content, version, Vec::new(), false, None, None);
        self.documents.insert(
            uri,
            DocumentState {
                document,
                pending_request_id: request_id,
            },
        );

        Ok(())
    }

    fn on_did_change(&mut self, params: DidChangeTextDocumentParams) -> Result<()> {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        tracing::debug!(?uri, version, "document changed");

        if let Some(state) = self.documents.get_mut(&uri) {
            state
                .document
                .apply_changes(params.content_changes, version);

            // submit updated content for backgroun parsing
            self.debouncer.schedule(uri, version);
        } else {
            // fall back to using full content from last change
            let content = params
                .content_changes
                .into_iter()
                .last()
                .map(|c| c.text)
                .unwrap_or_default();

            let document = Document::new(content, version);
            self.documents.insert(
                uri.clone(),
                DocumentState {
                    document,
                    pending_request_id: None,
                },
            );

            self.debouncer.schedule(uri, version);
        };

        Ok(())
    }

    fn on_did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<()> {
        let uri = params.text_document.uri;

        tracing::info!(?uri, "document closed");

        self.debouncer.cancel(&uri);
        self.documents.remove(&uri);
        self.send_diagnostics(&uri, vec![]);
        Ok(())
    }

    #[inline]
    fn send_diagnostics(&self, uri: &Uri, diagnostics: Vec<lsp_types::Diagnostic>) {
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: None,
        };

        let notif = Notification::new(PublishDiagnostics::METHOD.to_string(), params);

        if let Err(e) = self.connection.sender.send(Message::Notification(notif)) {
            tracing::error!(?e, "failed to send diagnostics");
        }
    }

    #[inline]
    fn send_response<T: serde::Serialize>(&self, id: RequestId, result: Option<T>) {
        let response = match result {
            Some(r) => Response::new_ok(id, serde_json::to_value(r).unwrap()),
            None => Response::new_ok(id, serde_json::Value::Null),
        };

        if let Err(e) = self.connection.sender.send(Message::Response(response)) {
            tracing::error!(?e, "failed to send response");
        }
    }
}

fn cast_request<R>(req: Request) -> Result<(RequestId, R::Params)>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    let params = serde_json::from_value(req.params)?;
    Ok((req.id, params))
}
