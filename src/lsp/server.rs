// LSP server implementation
// "The bridge between Palladium and your IDE"

use super::LanguageServer;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Mutex};

/// LSP server that handles JSON-RPC communication
pub struct LspServer {
    /// Language server state
    pub(super) server: Arc<Mutex<LanguageServer>>,
    /// Request ID counter
    #[allow(dead_code)]
    next_id: i64,
}

/// JSON-RPC request
#[derive(Debug, Deserialize)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

/// JSON-RPC response
#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ResponseError>,
}

/// JSON-RPC notification
#[derive(Debug, Serialize)]
struct Notification {
    jsonrpc: String,
    method: String,
    params: Value,
}

/// Response error
#[derive(Debug, Serialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

// Error codes
#[allow(dead_code)]
const PARSE_ERROR: i32 = -32700;
#[allow(dead_code)]
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;

impl Default for LspServer {
    fn default() -> Self {
        Self {
            server: Arc::new(Mutex::new(LanguageServer::new())),
            next_id: 1,
        }
    }
}

impl LspServer {
    /// Create a new LSP server
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the LSP server
    pub fn run(&mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        let mut reader = BufReader::new(stdin);

        loop {
            // Read message
            match self.read_message(&mut reader) {
                Ok(Some(message)) => {
                    // Process message
                    if let Some(response) = self.process_message(&message)? {
                        // Send response
                        self.send_message(&response, &mut stdout.lock())?;
                    }
                }
                Ok(None) => {
                    // EOF reached
                    break;
                }
                Err(e) => {
                    eprintln!("Error reading message: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Read a message from stdin
    fn read_message(&self, reader: &mut BufReader<std::io::Stdin>) -> Result<Option<String>> {
        let mut headers = HashMap::new();

        // Read headers
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                return Ok(None); // EOF
            }

            let line = line.trim_end();
            if line.is_empty() {
                break; // End of headers
            }

            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim();
                let value = line[colon_pos + 1..].trim();
                headers.insert(key.to_string(), value.to_string());
            }
        }

        // Get content length
        let content_length = headers
            .get("Content-Length")
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or_else(|| {
                crate::errors::CompileError::Generic("Missing Content-Length header".to_string())
            })?;

        // Read content
        let mut content = vec![0u8; content_length];
        reader.read_exact(&mut content)?;

        Ok(Some(String::from_utf8(content).map_err(|e| {
            crate::errors::CompileError::Generic(e.to_string())
        })?))
    }

    /// Send a message to stdout
    fn send_message(&self, message: &str, writer: &mut std::io::StdoutLock) -> Result<()> {
        write!(
            writer,
            "Content-Length: {}\r\n\r\n{}",
            message.len(),
            message
        )?;
        writer.flush()?;
        Ok(())
    }

    /// Process a message and return response if needed
    fn process_message(&mut self, message: &str) -> Result<Option<String>> {
        // Parse JSON-RPC request
        let request: Request = serde_json::from_str(message).map_err(|e| {
            crate::errors::CompileError::Generic(format!("Failed to parse request: {}", e))
        })?;

        // Handle request
        if let Some(id) = request.id {
            // Request with ID - needs response
            let response = self.handle_request(id, &request.method, request.params)?;
            Ok(Some(serde_json::to_string(&response).map_err(|e| {
                crate::errors::CompileError::Generic(e.to_string())
            })?))
        } else {
            // Notification - no response needed
            self.handle_notification(&request.method, request.params)?;
            Ok(None)
        }
    }

    /// Handle a request that requires a response
    fn handle_request(
        &mut self,
        id: Value,
        method: &str,
        params: Option<Value>,
    ) -> Result<Response> {
        let result = match method {
            "initialize" => self.handle_initialize(params),
            "shutdown" => self.handle_shutdown(),
            "textDocument/completion" => self.handle_completion(params),
            "textDocument/hover" => self.handle_hover(params),
            "textDocument/definition" => self.handle_definition(params),
            "textDocument/references" => self.handle_references(params),
            "textDocument/documentSymbol" => self.handle_document_symbols(params),
            "textDocument/formatting" => self.handle_formatting(params),
            "textDocument/rename" => self.handle_rename(params),
            "workspace/symbol" => self.handle_workspace_symbols(params),
            _ => Err(ResponseError {
                code: METHOD_NOT_FOUND,
                message: format!("Method not found: {}", method),
                data: None,
            }),
        };

        match result {
            Ok(value) => Ok(Response {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(value),
                error: None,
            }),
            Err(error) => Ok(Response {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(error),
            }),
        }
    }

    /// Handle a notification
    fn handle_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        match method {
            "initialized" => {
                self.handle_initialized();
                Ok(())
            }
            "textDocument/didOpen" => self.handle_did_open(params),
            "textDocument/didChange" => self.handle_did_change(params),
            "textDocument/didClose" => self.handle_did_close(params),
            "textDocument/didSave" => self.handle_did_save(params),
            _ => {
                // Unknown notification - ignore
                eprintln!("Unknown notification: {}", method);
                Ok(())
            }
        }
    }

    /// Handle initialize request
    fn handle_initialize(
        &mut self,
        params: Option<Value>,
    ) -> std::result::Result<Value, ResponseError> {
        #[derive(Deserialize)]
        struct InitializeParams {
            #[serde(rename = "rootUri")]
            root_uri: Option<String>,
        }

        let params: InitializeParams = if let Some(p) = params {
            serde_json::from_value(p).map_err(|_| ResponseError {
                code: INVALID_PARAMS,
                message: "Invalid initialize params".to_string(),
                data: None,
            })?
        } else {
            InitializeParams { root_uri: None }
        };

        // Initialize server
        self.server
            .lock()
            .unwrap()
            .initialize(params.root_uri)
            .map_err(|e| ResponseError {
                code: INTERNAL_ERROR,
                message: e.to_string(),
                data: None,
            })?;

        // Return capabilities
        let capabilities = &self.server.lock().unwrap().capabilities;
        Ok(json!({
            "capabilities": capabilities
        }))
    }

    /// Handle initialized notification
    fn handle_initialized(&mut self) {
        // Server is fully initialized
        eprintln!("LSP server initialized");
    }

    /// Handle shutdown request
    fn handle_shutdown(&mut self) -> std::result::Result<Value, ResponseError> {
        Ok(Value::Null)
    }

    /// Handle textDocument/didOpen notification
    fn handle_did_open(&mut self, params: Option<Value>) -> Result<()> {
        #[derive(Deserialize)]
        struct DidOpenParams {
            #[serde(rename = "textDocument")]
            text_document: TextDocumentItem,
        }

        #[derive(Deserialize)]
        struct TextDocumentItem {
            uri: String,
            version: i32,
            text: String,
        }

        if let Some(params) = params {
            let params: DidOpenParams = serde_json::from_value(params)
                .map_err(|e| crate::errors::CompileError::Generic(e.to_string()))?;
            let doc = params.text_document;

            self.server
                .lock()
                .unwrap()
                .open_document(doc.uri.clone(), doc.version, doc.text)?;

            // Send diagnostics
            self.send_diagnostics(&doc.uri)?;
        }

        Ok(())
    }

    /// Handle textDocument/didChange notification
    fn handle_did_change(&mut self, params: Option<Value>) -> Result<()> {
        #[derive(Deserialize)]
        struct DidChangeParams {
            #[serde(rename = "textDocument")]
            text_document: VersionedTextDocumentIdentifier,
            #[serde(rename = "contentChanges")]
            content_changes: Vec<TextDocumentContentChangeEvent>,
        }

        #[derive(Deserialize)]
        struct VersionedTextDocumentIdentifier {
            uri: String,
            version: i32,
        }

        #[derive(Deserialize)]
        struct TextDocumentContentChangeEvent {
            text: String,
        }

        if let Some(params) = params {
            let params: DidChangeParams = serde_json::from_value(params)
                .map_err(|e| crate::errors::CompileError::Generic(e.to_string()))?;

            if let Some(change) = params.content_changes.first() {
                self.server.lock().unwrap().update_document(
                    params.text_document.uri.clone(),
                    params.text_document.version,
                    change.text.clone(),
                )?;

                // Send diagnostics
                self.send_diagnostics(&params.text_document.uri)?;
            }
        }

        Ok(())
    }

    /// Handle textDocument/didClose notification
    fn handle_did_close(&mut self, params: Option<Value>) -> Result<()> {
        #[derive(Deserialize)]
        struct DidCloseParams {
            #[serde(rename = "textDocument")]
            text_document: TextDocumentIdentifier,
        }

        #[derive(Deserialize)]
        struct TextDocumentIdentifier {
            uri: String,
        }

        if let Some(params) = params {
            let params: DidCloseParams = serde_json::from_value(params)
                .map_err(|e| crate::errors::CompileError::Generic(e.to_string()))?;

            self.server
                .lock()
                .unwrap()
                .close_document(params.text_document.uri)?;
        }

        Ok(())
    }

    /// Handle textDocument/didSave notification
    fn handle_did_save(&mut self, _params: Option<Value>) -> Result<()> {
        // Nothing to do for now
        Ok(())
    }

    /// Send diagnostics for a document
    fn send_diagnostics(&mut self, uri: &str) -> Result<()> {
        let diagnostics = self
            .server
            .lock()
            .unwrap()
            .diagnostics
            .get(uri)
            .cloned()
            .unwrap_or_default();

        let notification = Notification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/publishDiagnostics".to_string(),
            params: json!({
                "uri": uri,
                "diagnostics": diagnostics
            }),
        };

        let message = serde_json::to_string(&notification)
            .map_err(|e| crate::errors::CompileError::Generic(e.to_string()))?;
        self.send_message(&message, &mut std::io::stdout().lock())?;

        Ok(())
    }

}
