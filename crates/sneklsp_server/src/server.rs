use std::collections::HashMap;

use anyhow::Result;
use lsp_server::{Connection, Message, Notification, Request};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    PublishDiagnostics,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, PublishDiagnosticsParams, SaveOptions, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
};

use crate::diagnostics::to_diagnostics;
use crate::document::Document;

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

struct Server {
    connection: Connection,
    documents: HashMap<Uri, Document>,
}

impl Server {
    fn new(connection: Connection) -> Self {
        Self {
            connection,
            documents: HashMap::new(),
        }
    }

    fn run(&mut self) -> Result<()> {
        tracing::info!("server main loop starting");

        loop {
            let msg = self.connection.receiver.recv()?;
            match msg {
                Message::Request(req) => {
                    if self.connection.handle_shutdown(&req)? {
                        tracing::info!("received shutdown request");
                        return Ok(());
                    }
                    self.handle_request(req)?;
                }
                Message::Response(resp) => {
                    tracing::debug!(?resp, "received response");
                }
                Message::Notification(notif) => {
                    self.handle_notification(notif)?;
                }
            }
        }
    }

    fn handle_request(&mut self, req: Request) -> Result<()> {
        tracing::debug!(?req.method, "handling request");

        // TODO: handle requests like hover, completion, goto definition, etc.

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

        let document = Document::new(content, version);
        self.publish_diagnostics(&uri, &document);
        self.documents.insert(uri, document);

        Ok(())
    }

    fn on_did_change(&mut self, params: DidChangeTextDocumentParams) -> Result<()> {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        tracing::debug!(?uri, version, "document changed");

        let diagnostics = if let Some(document) = self.documents.get_mut(&uri) {
            document.apply_changes(params.content_changes, version);
            to_diagnostics(&document.errors, &document.line_index)
        } else {
            // fall back to using full content from last change
            let content = params
                .content_changes
                .into_iter()
                .last()
                .map(|c| c.text)
                .unwrap_or_default();

            let doc = Document::new(content, version);
            let diagnostics = to_diagnostics(&doc.errors, &doc.line_index);
            self.documents.insert(uri.clone(), doc);
            diagnostics
        };

        self.send_diagnostics(&uri, diagnostics);
        Ok(())
    }

    fn on_did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<()> {
        let uri = params.text_document.uri;

        tracing::info!(?uri, "document closed");

        self.documents.remove(&uri);
        self.send_diagnostics(&uri, vec![]);
        Ok(())
    }

    fn publish_diagnostics(&self, uri: &Uri, document: &Document) {
        let diagnostics = to_diagnostics(&document.errors, &document.line_index);
        self.send_diagnostics(uri, diagnostics);
    }

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
}
