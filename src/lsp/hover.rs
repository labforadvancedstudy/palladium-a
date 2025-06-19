// Hover information provider
// "Type information at your fingertips"

use super::{LanguageServer, Position, Range};
use serde::{Serialize, Deserialize};

/// Hover response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    /// The hover contents
    pub contents: MarkupContent,
    /// An optional range
    pub range: Option<Range>,
}

/// Markup content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkupContent {
    /// The type of the markup
    pub kind: MarkupKind,
    /// The content
    pub value: String,
}

/// Markup kind
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkupKind {
    #[serde(rename = "plaintext")]
    PlainText,
    #[serde(rename = "markdown")]
    Markdown,
}

impl LanguageServer {
    /// Get hover information at a position
    pub fn get_hover(&self, uri: &str, position: Position) -> Option<Hover> {
        let doc = self.documents.get(uri)?;
        let ast = doc.ast.as_ref()?;
        
        // Find what's at the position
        let symbol = self.find_symbol_at_position(&doc.content, position)?;
        
        // Look up type information
        if let Some(type_info) = &doc.type_info {
            // Check if it's a variable
            if let Some(var_type) = type_info.variables.get(&symbol) {
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```palladium\nlet {}: {}\n```", symbol, self.type_to_string(var_type)),
                    },
                    range: None,
                });
            }
            
            // Check if it's a function
            if let Some(func_sig) = type_info.functions.get(&symbol) {
                let params: Vec<String> = func_sig.params.iter()
                    .map(|(name, ty)| format!("{}: {}", name, self.type_to_string(ty)))
                    .collect();
                
                let signature = if let Some(ret) = &func_sig.return_type {
                    format!("fn {}({}) -> {}", symbol, params.join(", "), self.type_to_string(ret))
                } else {
                    format!("fn {}({})", symbol, params.join(", "))
                };
                
                let mut hover_text = format!("```palladium\n{}\n```", signature);
                
                if func_sig.is_async {
                    hover_text.push_str("\n\n*Async function*");
                }
                
                if !func_sig.effects.is_empty() {
                    hover_text.push_str(&format!("\n\n**Effects**: {}", func_sig.effects.join(", ")));
                }
                
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: hover_text,
                    },
                    range: None,
                });
            }
            
            // Check if it's a type alias
            if let Some(alias_type) = type_info.type_aliases.get(&symbol) {
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```palladium\ntype {} = {}\n```", symbol, self.type_to_string(alias_type)),
                    },
                    range: None,
                });
            }
        }
        
        // Check built-in functions
        let builtins = vec![
            ("print", "fn print(s: String)", "Print a string to stdout"),
            ("print_int", "fn print_int(n: i64)", "Print an integer to stdout"),
            ("string_len", "fn string_len(s: String) -> i64", "Get the length of a string"),
            ("string_concat", "fn string_concat(a: String, b: String) -> String", "Concatenate two strings"),
            ("int_to_string", "fn int_to_string(n: i64) -> String", "Convert an integer to a string"),
            ("string_to_int", "fn string_to_int(s: String) -> Option<i64>", "Parse an integer from a string"),
        ];
        
        for (name, sig, doc) in builtins {
            if name == symbol {
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```palladium\n{}\n```\n\n{}", sig, doc),
                    },
                    range: None,
                });
            }
        }
        
        // Check built-in types
        let types = vec![
            ("i32", "32-bit signed integer"),
            ("i64", "64-bit signed integer"),
            ("u32", "32-bit unsigned integer"),
            ("u64", "64-bit unsigned integer"),
            ("bool", "Boolean type"),
            ("String", "UTF-8 string type"),
            ("Vec", "Dynamic array type"),
            ("HashMap", "Hash map type"),
            ("Option", "Optional type"),
            ("Result", "Result type"),
        ];
        
        for (type_name, description) in types {
            if type_name == symbol {
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```palladium\ntype {}\n```\n\n{}", type_name, description),
                    },
                    range: None,
                });
            }
        }
        
        None
    }
    
    /// Find symbol at position
    pub fn find_symbol_at_position(&self, content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        let line = lines.get(position.line as usize)?;
        let chars: Vec<char> = line.chars().collect();
        
        let pos = position.character as usize;
        if pos >= chars.len() {
            return None;
        }
        
        // Find word boundaries
        let mut start = pos;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        
        let mut end = pos;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        
        if start == end {
            return None;
        }
        
        Some(chars[start..end].iter().collect())
    }
}