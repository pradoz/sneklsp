use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{Receiver, select, tick};
use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::{
    CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare,
    CodeActionRequest, Completion, DocumentHighlightRequest, DocumentSymbolRequest,
    FoldingRangeRequest, GotoDefinition, HoverRequest, InlayHintRequest, PrepareRenameRequest,
    References, Rename, Request as _, SelectionRangeRequest, SemanticTokensFullRequest,
    SemanticTokensRangeRequest, SignatureHelpRequest, WorkspaceSymbolRequest,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, FileChangeType, InitializeParams, InitializeResult, OneOf,
    PublishDiagnosticsParams, SaveOptions, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
};

use crate::analysis::AnalysisHost;
use crate::debouncer::Debouncer;
use crate::document::Document;
use crate::handlers;
use sneklsp_vfs::{FileId, VfsPath};
use sneklsp_workspace::{FileState, Workspace};

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
        call_hierarchy_provider: Some(lsp_types::CallHierarchyServerCapability::Simple(true)),
        code_action_provider: Some(lsp_types::CodeActionProviderCapability::Simple(true)),
        completion_provider: Some(lsp_types::CompletionOptions {
            trigger_characters: Some(vec![".".to_string()]),
            ..Default::default()
        }),
        definition_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        inlay_hint_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(lsp_types::OneOf::Right(lsp_types::RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
        semantic_tokens_provider: Some(
            lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(
                lsp_types::SemanticTokensOptions {
                    legend: crate::semantic_tokens::legend(),
                    full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
                    range: Some(true),
                    work_done_progress_options: Default::default(),
                },
            ),
        ),
        signature_help_provider: Some(lsp_types::SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            retrigger_characters: Some(vec![",".to_string()]),
            work_done_progress_options: Default::default(),
        }),
        workspace_symbol_provider: Some(lsp_types::OneOf::Left(true)),
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

    let mut server = Server::new(connection);

    // discover workspace roots
    if let Some(folders) = init_params.workspace_folders {
        for f in folders {
            if let Some(path) = VfsPath::from_uri(&f.uri) {
                let file_ids = server.workspace.add_root(path.as_path());
                server.index_files_background(file_ids);
            }
        }
    }

    server.run()?;

    io_threads.join()?;

    tracing::info!("server shutdown complete");
    Ok(())
}

pub struct DocumentState {
    pub document: Document,
    pub file_id: FileId,
}

struct Server {
    connection: Connection,
    documents: HashMap<Uri, DocumentState>,
    workspace: Workspace,
    debouncer: Debouncer,
    workspace_index_rx: Option<Receiver<(FileId, FileState)>>,
    analysis: AnalysisHost,
}

impl Server {
    fn new(connection: Connection) -> Self {
        Self {
            connection,
            documents: HashMap::new(),
            workspace: Workspace::new(),
            debouncer: Debouncer::new(),
            workspace_index_rx: None,
            analysis: AnalysisHost::new(),
        }
    }

    fn index_files_background(&mut self, file_ids: Vec<FileId>) {
        if file_ids.is_empty() {
            return;
        }

        tracing::info!(
            file_count = file_ids.len(),
            "starting background workspace indexing"
        );

        // collect paths and defer reading to the background thread
        let files: Vec<(FileId, std::path::PathBuf)> = file_ids
            .iter()
            .filter_map(|&id| {
                let path = self.workspace.vfs.file_path(id).as_path().to_path_buf();
                Some((id, path))
            })
            .collect();

        let (tx, rx) = crossbeam_channel::bounded(files.len().max(1));
        self.workspace_index_rx = Some(rx);

        std::thread::Builder::new()
            .name("sneklsp-workspace-index".to_string())
            .spawn(move || {
                for (file_id, path) in files {
                    // read from disk on background thread
                    let content = match std::fs::read_to_string(&path) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    let line_index = sneklsp_text::LineIndex::new(&content);
                    let arena =
                        sneklsp_ast::AstArena::with_capacity((content.len() * 50).max(4096));
                    let output = sneklsp_parser::parse_recovering(&content, &arena);

                    let index = if !output.module.body.is_empty() || output.errors.is_empty() {
                        let idx = sneklsp_index::index_module(&content, &output.module);
                        Some(sneklsp_index::OwnedIndex::new(content.clone(), &idx))
                    } else {
                        None
                    };

                    let tokens = sneklsp_lexer::tokenize(&content);

                    let _ = tx.send((
                        file_id,
                        sneklsp_workspace::FileState {
                            index,
                            line_index,
                            tokens,
                            version: None,
                        },
                    ));
                }
                tracing::info!("background workspace indexing complete");
            })
            .expect("failed to spawn workspace indexing thread");
    }

    fn drain_workspace_index(&mut self) {
        let done = if let Some(ref rx) = self.workspace_index_rx {
            // drain without blocking, process whatever is ready
            let mut count = 0;
            while let Ok((file_id, state)) = rx.try_recv() {
                self.workspace.set_file_state(file_id, state);

                let vfs_path = self.workspace.vfs.file_path(file_id);
                let path_str = vfs_path.as_path().display().to_string();

                if let Some(content) = self.workspace.vfs.read(file_id) {
                    if let Some(module_name) = self.workspace.resolve_module_name(file_id) {
                        self.analysis.queue_module(
                            file_id,
                            module_name,
                            path_str,
                            content.to_string(),
                        );
                    } else {
                        self.analysis
                            .set_file_content(file_id, &path_str, content.to_string());
                    }
                }

                count += 1;
            }

            if count > 0 {
                // flush queued modules in one batch
                self.analysis.flush_modules();
                tracing::debug!(count, "drained workspace index results");
            }

            // channel empty + sender dropped = done
            rx.is_empty() && rx.len() == 0
        } else {
            false
        };

        if done {
            self.workspace_index_rx = None;
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

                recv(ticker) -> _ => {
                    self.process_debounced();
                    self.drain_workspace_index();
                }
            }
        }
    }

    fn process_debounced(&mut self) {
        for (uri, version) in self.debouncer.take_ready() {
            let Some(state) = self.documents.get_mut(&uri) else {
                continue;
            };
            if state.document.version != version {
                continue;
            }

            tracing::debug!(?uri, version, "debounce complete. submitting parse");

            let file_id = state.file_id;
            let content = state.document.content_for_parse();
            let path = self
                .workspace
                .vfs
                .file_path(file_id)
                .as_path()
                .display()
                .to_string();

            self.analysis.set_file_content(file_id, &path, content);

            let start = std::time::Instant::now();
            let Some(analysis) = self.analysis.analyze_file(file_id) else {
                continue;
            };

            let elapsed = start.elapsed();
            tracing::debug!(
                ?uri,
                ?elapsed,
                error_count = analysis.errors.len(),
                token_count = analysis.tokens.len(),
                "salsa analysis complete"
            );

            // reborrow state mutably after analysis
            let state = self.documents.get_mut(&uri).unwrap();
            state.document.set_tokens(analysis.tokens.clone());
            if let Some(ref idx) = analysis.index {
                state
                    .document
                    .set_index_from_analysis(idx, &analysis.line_index);
            }

            let mut diagnostics = crate::diagnostics::serialized_errors_to_diagnostics(
                &analysis.errors,
                &analysis.line_index,
            );
            if let Some(ref idx) = analysis.index {
                diagnostics.extend(crate::diagnostics::semantic_diagnostics(
                    idx,
                    &analysis.line_index,
                    &self.analysis,
                ));
            }
            self.send_diagnostics(&uri, diagnostics);
        }
    }

    fn handle_request(&mut self, req: Request) -> Result<()> {
        tracing::debug!(?req.method, "handling request");

        match req.method.as_str() {
            CallHierarchyPrepare::METHOD => {
                let (id, params) = cast_request::<CallHierarchyPrepare>(req)?;
                let result = handlers::handle_prepare_call_hierarchy(params, &self.documents);
                self.send_response(id, result);
            }
            CallHierarchyIncomingCalls::METHOD => {
                let (id, params) = cast_request::<CallHierarchyIncomingCalls>(req)?;
                let result = handlers::handle_incoming_calls(
                    params,
                    &self.documents,
                    &self.analysis,
                    &self.workspace,
                );
                self.send_response(id, result);
            }
            CallHierarchyOutgoingCalls::METHOD => {
                let (id, params) = cast_request::<CallHierarchyOutgoingCalls>(req)?;
                let result = handlers::handle_outgoing_calls(params, &self.documents);
                self.send_response(id, result);
            }

            CodeActionRequest::METHOD => {
                let (id, params) = cast_request::<CodeActionRequest>(req)?;
                let result = handlers::handle_code_action(params, &self.documents);
                self.send_response(id, result);
            }

            Completion::METHOD => {
                let (id, params) = cast_request::<Completion>(req)?;
                let result = handlers::handle_completion(
                    params,
                    &self.documents,
                    &self.analysis,
                    &self.workspace,
                );
                self.send_response(id, result);
            }

            DocumentHighlightRequest::METHOD => {
                let (id, params) = cast_request::<DocumentHighlightRequest>(req)?;
                let result = handlers::handle_document_highlight(params, &self.documents);
                self.send_response(id, result);
            }

            DocumentSymbolRequest::METHOD => {
                let (id, params) = cast_request::<DocumentSymbolRequest>(req)?;
                let result = handlers::handle_document_symbol(params, &self.documents);
                self.send_response(id, result);
            }

            GotoDefinition::METHOD => {
                let (id, params) = cast_request::<GotoDefinition>(req)?;
                let result = handlers::handle_goto_definition(
                    params,
                    &self.documents,
                    &self.workspace,
                    &self.analysis,
                );
                self.send_response(id, result);
            }

            InlayHintRequest::METHOD => {
                let (id, params) = cast_request::<InlayHintRequest>(req)?;
                let result = handlers::handle_inlay_hint(params, &self.documents);
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

            FoldingRangeRequest::METHOD => {
                let (id, params) = cast_request::<FoldingRangeRequest>(req)?;
                let result = handlers::handle_folding_range(params, &self.documents);
                self.send_response(id, result);
            }

            SelectionRangeRequest::METHOD => {
                let (id, params) = cast_request::<SelectionRangeRequest>(req)?;
                let result = handlers::handle_selection_range(params, &self.documents);
                self.send_response(id, result);
            }

            SemanticTokensFullRequest::METHOD => {
                let (id, params) = cast_request::<SemanticTokensFullRequest>(req)?;
                let result = handlers::handle_semantic_tokens(params, &self.documents);
                self.send_response(id, result);
            }

            SemanticTokensRangeRequest::METHOD => {
                let (id, params) = cast_request::<SemanticTokensRangeRequest>(req)?;
                let result = handlers::handle_semantic_tokens_range(params, &self.documents);
                self.send_response(id, result);
            }

            SignatureHelpRequest::METHOD => {
                let (id, params) = cast_request::<SignatureHelpRequest>(req)?;
                let result = handlers::handle_signature_help(params, &self.documents);
                self.send_response(id, result);
            }

            Rename::METHOD => {
                let (id, params) = cast_request::<Rename>(req)?;
                let result = handlers::handle_rename(
                    params,
                    &self.documents,
                    &self.analysis,
                    &self.workspace,
                );
                self.send_response(id, result);
            }

            PrepareRenameRequest::METHOD => {
                let (id, params) = cast_request::<PrepareRenameRequest>(req)?;
                let result = handlers::handle_prepare_rename(params, &self.documents);
                self.send_response(id, result);
            }

            WorkspaceSymbolRequest::METHOD => {
                let (id, params) = cast_request::<WorkspaceSymbolRequest>(req)?;
                let result = handlers::handle_workspace_symbol(
                    params,
                    &self.documents,
                    &self.analysis,
                    &self.workspace,
                );
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
            DidChangeWatchedFiles::METHOD => {
                let params: DidChangeWatchedFilesParams = serde_json::from_value(notif.params)?;
                self.on_did_change_watched_files(params)?;
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

        let file_id = self.workspace.file_id_for_uri(&uri).unwrap_or_else(|| {
            let path = VfsPath::from_uri(&uri)
                .unwrap_or_else(|| VfsPath::new(std::path::PathBuf::from(uri.path().as_str())));
            self.workspace.vfs.intern_path(path)
        });

        self.workspace
            .vfs
            .set_overlay(file_id, content.clone(), version);

        let document = Document::new(content.clone(), version);
        self.documents
            .insert(uri.clone(), DocumentState { document, file_id });

        // schedule through debouncer instead of background parser
        self.debouncer.schedule(uri, version);

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
            let content = params
                .content_changes
                .into_iter()
                .last()
                .map(|c| c.text)
                .unwrap_or_default();

            let file_id = self.workspace.file_id_for_uri(&uri).unwrap_or_else(|| {
                let path = VfsPath::from_uri(&uri)
                    .unwrap_or_else(|| VfsPath::new(std::path::PathBuf::from(uri.path().as_str())));
                self.workspace.vfs.intern_path(path)
            });

            let document = Document::new(content, version);
            self.documents
                .insert(uri.clone(), DocumentState { document, file_id });

            self.debouncer.schedule(uri, version);
        };

        Ok(())
    }

    fn on_did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<()> {
        let uri = params.text_document.uri;

        tracing::info!(?uri, "document closed");

        if let Some(state) = self.documents.get(&uri) {
            self.workspace.vfs.remove_overlay(state.file_id);
        }

        self.debouncer.cancel(&uri);
        self.documents.remove(&uri);
        self.send_diagnostics(&uri, vec![]);
        Ok(())
    }

    fn on_did_change_watched_files(&mut self, params: DidChangeWatchedFilesParams) -> Result<()> {
        for change in params.changes {
            let uri = change.uri;
            if self.documents.contains_key(&uri) {
                continue;
            }

            match change.typ {
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    if let Some(file_id) = self.workspace.file_id_for_uri(&uri) {
                        tracing::debug!(?uri, "re-indexing changed file");
                        self.workspace.index_file(file_id);
                    }
                }
                FileChangeType::DELETED => {
                    if let Some(file_id) = self.workspace.file_id_for_uri(&uri) {
                        tracing::debug!(?uri, "removing deleted file");
                        self.workspace.remove_file_state(file_id);
                    }
                }
                _ => {}
            }
        }

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
