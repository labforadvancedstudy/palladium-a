// LSP request handlers
// "Handling your IDE's requests with legendary efficiency"

use super::server::{LspServer, ResponseError};
use serde_json::{json, Value};

// Error codes
const INVALID_PARAMS: i32 = -32602;

impl LspServer {
    /// Handle textDocument/hover request
    pub fn handle_hover(&self, params: Option<Value>) -> std::result::Result<Value, ResponseError> {
        #[derive(serde::Deserialize)]
        struct HoverParams {
            #[serde(rename = "textDocument")]
            text_document: TextDocumentIdentifier,
            position: super::Position,
        }
        
        #[derive(serde::Deserialize)]
        struct TextDocumentIdentifier {
            uri: String,
        }
        
        if let Some(params) = params {
            let params: HoverParams = serde_json::from_value(params)
                .map_err(|_| ResponseError {
                    code: INVALID_PARAMS,
                    message: "Invalid hover params".to_string(),
                    data: None,
                })?;
            
            if let Some(hover) = self.server.lock().unwrap()
                .get_hover(&params.text_document.uri, params.position) {
                Ok(serde_json::to_value(hover).unwrap())
            } else {
                Ok(Value::Null)
            }
        } else {
            Ok(Value::Null)
        }
    }
    
    /// Handle textDocument/definition request
    pub fn handle_definition(&self, params: Option<Value>) -> std::result::Result<Value, ResponseError> {
        #[derive(serde::Deserialize)]
        struct DefinitionParams {
            #[serde(rename = "textDocument")]
            text_document: TextDocumentIdentifier,
            position: super::Position,
        }
        
        #[derive(serde::Deserialize)]
        struct TextDocumentIdentifier {
            uri: String,
        }
        
        if let Some(params) = params {
            let params: DefinitionParams = serde_json::from_value(params)
                .map_err(|_| ResponseError {
                    code: INVALID_PARAMS,
                    message: "Invalid definition params".to_string(),
                    data: None,
                })?;
            
            if let Some(location) = self.server.lock().unwrap()
                .find_definition(&params.text_document.uri, params.position) {
                Ok(serde_json::to_value(location).unwrap())
            } else {
                Ok(Value::Null)
            }
        } else {
            Ok(Value::Null)
        }
    }
    
    /// Handle textDocument/references request
    pub fn handle_references(&self, params: Option<Value>) -> std::result::Result<Value, ResponseError> {
        #[derive(serde::Deserialize)]
        struct ReferencesParams {
            #[serde(rename = "textDocument")]
            text_document: TextDocumentIdentifier,
            position: super::Position,
            context: ReferenceContext,
        }
        
        #[derive(serde::Deserialize)]
        struct TextDocumentIdentifier {
            uri: String,
        }
        
        #[derive(serde::Deserialize)]
        struct ReferenceContext {
            #[serde(rename = "includeDeclaration")]
            include_declaration: bool,
        }
        
        if let Some(params) = params {
            let params: ReferencesParams = serde_json::from_value(params)
                .map_err(|_| ResponseError {
                    code: INVALID_PARAMS,
                    message: "Invalid references params".to_string(),
                    data: None,
                })?;
            
            let references = self.server.lock().unwrap()
                .find_references(
                    &params.text_document.uri,
                    params.position,
                    params.context.include_declaration,
                );
            
            Ok(serde_json::to_value(references).unwrap())
        } else {
            Ok(json!([]))
        }
    }
    
    /// Handle textDocument/documentSymbol request
    pub fn handle_document_symbols(&self, params: Option<Value>) -> std::result::Result<Value, ResponseError> {
        #[derive(serde::Deserialize)]
        struct DocumentSymbolParams {
            #[serde(rename = "textDocument")]
            text_document: TextDocumentIdentifier,
        }
        
        #[derive(serde::Deserialize)]
        struct TextDocumentIdentifier {
            uri: String,
        }
        
        if let Some(params) = params {
            let params: DocumentSymbolParams = serde_json::from_value(params)
                .map_err(|_| ResponseError {
                    code: INVALID_PARAMS,
                    message: "Invalid document symbol params".to_string(),
                    data: None,
                })?;
            
            let symbols = self.server.lock().unwrap()
                .get_document_symbols(&params.text_document.uri);
            
            Ok(serde_json::to_value(symbols).unwrap())
        } else {
            Ok(json!([]))
        }
    }
    
    /// Handle workspace/symbol request
    pub fn handle_workspace_symbols(&self, params: Option<Value>) -> std::result::Result<Value, ResponseError> {
        #[derive(serde::Deserialize)]
        struct WorkspaceSymbolParams {
            query: String,
        }
        
        if let Some(params) = params {
            let params: WorkspaceSymbolParams = serde_json::from_value(params)
                .map_err(|_| ResponseError {
                    code: INVALID_PARAMS,
                    message: "Invalid workspace symbol params".to_string(),
                    data: None,
                })?;
            
            let symbols = self.server.lock().unwrap()
                .get_workspace_symbols(&params.query);
            
            Ok(serde_json::to_value(symbols).unwrap())
        } else {
            Ok(json!([]))
        }
    }
    
    /// Handle textDocument/rename request
    pub fn handle_rename(&self, params: Option<Value>) -> std::result::Result<Value, ResponseError> {
        #[derive(serde::Deserialize)]
        struct RenameParams {
            #[serde(rename = "textDocument")]
            text_document: TextDocumentIdentifier,
            position: super::Position,
            #[serde(rename = "newName")]
            new_name: String,
        }
        
        #[derive(serde::Deserialize)]
        struct TextDocumentIdentifier {
            uri: String,
        }
        
        if let Some(params) = params {
            let params: RenameParams = serde_json::from_value(params)
                .map_err(|_| ResponseError {
                    code: INVALID_PARAMS,
                    message: "Invalid rename params".to_string(),
                    data: None,
                })?;
            
            let edits = self.server.lock().unwrap()
                .compute_rename_edits(
                    &params.text_document.uri,
                    params.position,
                    &params.new_name,
                );
            
            Ok(json!({
                "changes": edits
            }))
        } else {
            Ok(Value::Null)
        }
    }
    
    /// Handle textDocument/formatting request
    pub fn handle_formatting(&self, params: Option<Value>) -> std::result::Result<Value, ResponseError> {
        // TODO: Implement formatting
        Ok(json!([]))
    }
}