//! Stdio-based LSP server exposing ravel's formatter as
//! `textDocument/formatting`. v1: no diagnostics, no range formatting.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tower_lsp_server::jsonrpc::Result as JsonRpcResult;
use tower_lsp_server::ls_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, InitializeParams, InitializeResult, InitializedParams, MessageType,
    OneOf, Position, Range, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Uri,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

use crate::config::Config;
use crate::formatter::{FormatStyle, format_with_style};

/// Run the language server on stdio until the client disconnects.
pub async fn run() {
    let (service, socket) = LspService::new(Backend::new);
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}

#[derive(Debug)]
struct Backend {
    client: Client,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    documents: HashMap<Uri, String>,
    config_cache: HashMap<PathBuf, FormatStyle>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            state: Mutex::new(State::default()),
        }
    }

    fn resolve_style(&self, uri: &Uri) -> Result<FormatStyle, ConfigResolveError> {
        if !uri.scheme().as_str().eq_ignore_ascii_case("file") {
            return Err(ConfigResolveError::NonFileUri);
        }
        let path = uri
            .to_file_path()
            .ok_or(ConfigResolveError::NonFileUri)?
            .into_owned();
        let anchor = path
            .parent()
            .ok_or(ConfigResolveError::NoParentDirectory)?
            .to_path_buf();

        {
            let state = self.state.lock().expect("state mutex poisoned");
            if let Some(style) = state.config_cache.get(&anchor) {
                return Ok(*style);
            }
        }

        let (config, _source) = Config::resolve(None, false, &anchor)
            .map_err(|err| ConfigResolveError::Config(err.to_string()))?;
        let style = FormatStyle::from(&config.format);

        let mut state = self.state.lock().expect("state mutex poisoned");
        state.config_cache.insert(anchor, style);
        Ok(style)
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> JsonRpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "ravel".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ravel LSP ready")
            .await;
    }

    async fn shutdown(&self) -> JsonRpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let mut state = self.state.lock().expect("state mutex poisoned");
        state
            .documents
            .insert(params.text_document.uri, params.text_document.text);
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        let Some(change) = params.content_changes.pop() else {
            return;
        };
        let mut state = self.state.lock().expect("state mutex poisoned");
        state
            .documents
            .insert(params.text_document.uri, change.text);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut state = self.state.lock().expect("state mutex poisoned");
        state.documents.remove(&params.text_document.uri);
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> JsonRpcResult<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let text = {
            let state = self.state.lock().expect("state mutex poisoned");
            state.documents.get(&uri).cloned()
        };
        let Some(text) = text else {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("format request for unknown document: {}", uri.as_str()),
                )
                .await;
            return Ok(None);
        };

        let style = match self.resolve_style(&uri) {
            Ok(style) => style,
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("config error for {}: {err}", uri.as_str()),
                    )
                    .await;
                return Ok(None);
            }
        };

        match compute_format_edits(&text, style) {
            Some(edits) => Ok(Some(edits)),
            None => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("ravel could not format {}", uri.as_str()),
                    )
                    .await;
                Ok(None)
            }
        }
    }
}

#[derive(Debug)]
enum ConfigResolveError {
    NonFileUri,
    NoParentDirectory,
    Config(String),
}

impl std::fmt::Display for ConfigResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFileUri => write!(f, "URI is not a file:// URI"),
            Self::NoParentDirectory => write!(f, "file has no parent directory"),
            Self::Config(msg) => f.write_str(msg),
        }
    }
}

/// Compute the LSP `TextEdit`s to format `text` with `style`.
///
/// Returns `None` when the formatter rejects the input (e.g. parse error).
/// An empty `Vec` means the document is already formatted.
pub fn compute_format_edits(text: &str, style: FormatStyle) -> Option<Vec<TextEdit>> {
    let formatted = format_with_style(text, style).ok()?;
    if formatted == text {
        return Some(Vec::new());
    }
    Some(vec![TextEdit {
        range: full_range(text),
        new_text: formatted,
    }])
}

fn full_range(text: &str) -> Range {
    Range {
        start: Position::new(0, 0),
        end: end_position(text),
    }
}

fn end_position(text: &str) -> Position {
    let mut line: u32 = 0;
    let mut last_line_start: usize = 0;
    for (offset, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            line += 1;
            last_line_start = offset + 1;
        }
    }
    let character = text[last_line_start..].encode_utf16().count() as u32;
    Position::new(line, character)
}
