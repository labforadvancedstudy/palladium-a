// Code completion for Palladium LSP
// "Intelligent suggestions for legendary code"

use super::{LanguageServer, Position};
use crate::ast::{Item, Program, Type};
use serde::{Deserialize, Serialize};

/// Completion item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    /// The label of this completion item
    pub label: String,
    /// The kind of this completion item
    pub kind: Option<CompletionItemKind>,
    /// A human-readable string with additional information
    pub detail: Option<String>,
    /// A human-readable string that represents a doc-comment
    pub documentation: Option<String>,
    /// The text to insert
    pub insert_text: Option<String>,
    /// The format of the insert text
    pub insert_text_format: Option<InsertTextFormat>,
    /// Additional text edits
    pub additional_text_edits: Option<Vec<TextEdit>>,
}

/// Completion item kind
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
    Folder = 19,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    Event = 23,
    Operator = 24,
    TypeParameter = 25,
}

/// Insert text format
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum InsertTextFormat {
    PlainText = 1,
    Snippet = 2,
}

/// Text edit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: super::Range,
    pub new_text: String,
}

/// Completion context
pub struct CompletionContext {
    /// The position where completion was triggered
    pub position: Position,
    /// The trigger character if any
    pub trigger_character: Option<String>,
    /// The current line text
    pub line_text: String,
    /// The word being typed
    pub word: String,
    /// Is this after a dot (method/field access)?
    pub is_dot_access: bool,
    /// Is this after :: (module/type access)?
    pub is_module_access: bool,
}

impl LanguageServer {
    /// Get completions at a position
    pub fn get_completions(&self, uri: &str, position: Position) -> Vec<CompletionItem> {
        let mut completions = Vec::new();

        // Get document
        let doc = match self.documents.get(uri) {
            Some(doc) => doc,
            None => return completions,
        };

        // Get completion context
        let context = match self.get_completion_context(&doc.content, position) {
            Some(ctx) => ctx,
            None => return completions,
        };

        // Get completions based on context
        if context.is_dot_access {
            // Method/field completions
            completions.extend(self.get_member_completions(&context, doc.ast.as_ref()));
        } else if context.is_module_access {
            // Module completions
            completions.extend(self.get_module_completions(&context));
        } else {
            // General completions
            completions.extend(self.get_general_completions(&context, doc.ast.as_ref()));
        }

        completions
    }

    /// Get completion context from position
    fn get_completion_context(
        &self,
        content: &str,
        position: Position,
    ) -> Option<CompletionContext> {
        let lines: Vec<&str> = content.lines().collect();
        let line_text = lines.get(position.line as usize)?.to_string();

        // Find the word being typed
        let char_pos = position.character as usize;
        let line_chars: Vec<char> = line_text.chars().collect();

        // Find word start
        let mut word_start = char_pos;
        while word_start > 0 && line_chars.get(word_start - 1)?.is_alphanumeric() {
            word_start -= 1;
        }

        let word = line_chars[word_start..char_pos].iter().collect();

        // Check for trigger characters
        let is_dot_access = word_start > 0 && line_chars.get(word_start - 1) == Some(&'.');
        let is_module_access = word_start > 1
            && line_chars.get(word_start - 2) == Some(&':')
            && line_chars.get(word_start - 1) == Some(&':');

        Some(CompletionContext {
            position,
            trigger_character: None,
            line_text,
            word,
            is_dot_access,
            is_module_access,
        })
    }

    /// Get general completions (keywords, functions, variables)
    fn get_general_completions(
        &self,
        context: &CompletionContext,
        ast: Option<&Program>,
    ) -> Vec<CompletionItem> {
        let mut completions = Vec::new();

        // Keywords
        let keywords = vec![
            ("fn", "Function declaration"),
            ("let", "Variable declaration"),
            ("mut", "Mutable variable"),
            ("if", "If statement"),
            ("else", "Else clause"),
            ("while", "While loop"),
            ("for", "For loop"),
            ("return", "Return statement"),
            ("struct", "Struct declaration"),
            ("enum", "Enum declaration"),
            ("trait", "Trait declaration"),
            ("impl", "Implementation block"),
            ("pub", "Public visibility"),
            ("async", "Async function"),
            ("await", "Await expression"),
            ("match", "Match expression"),
            ("break", "Break statement"),
            ("continue", "Continue statement"),
            ("const", "Constant declaration"),
            ("type", "Type alias"),
            ("use", "Import statement"),
            ("mod", "Module declaration"),
            ("Self", "Self type"),
            ("self", "Self parameter"),
            ("true", "Boolean true"),
            ("false", "Boolean false"),
        ];

        for (keyword, detail) in keywords {
            if keyword.starts_with(&context.word) {
                completions.push(CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::Keyword),
                    detail: Some(detail.to_string()),
                    documentation: None,
                    insert_text: Some(keyword.to_string()),
                    insert_text_format: Some(InsertTextFormat::PlainText),
                    additional_text_edits: None,
                });
            }
        }

        // Built-in types
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

        for (type_name, detail) in types {
            if type_name.starts_with(&context.word) {
                completions.push(CompletionItem {
                    label: type_name.to_string(),
                    kind: Some(CompletionItemKind::Class),
                    detail: Some(detail.to_string()),
                    documentation: None,
                    insert_text: Some(type_name.to_string()),
                    insert_text_format: Some(InsertTextFormat::PlainText),
                    additional_text_edits: None,
                });
            }
        }

        // Functions from AST
        if let Some(ast) = ast {
            for item in &ast.items {
                match item {
                    Item::Function(func) => {
                        if func.name.starts_with(&context.word) {
                            let params: Vec<String> = func
                                .params
                                .iter()
                                .map(|p| format!("{}: {}", p.name, self.type_to_string(&p.ty)))
                                .collect();

                            let signature = format!(
                                "fn {}({}){}",
                                func.name,
                                params.join(", "),
                                func.return_type
                                    .as_ref()
                                    .map(|t| format!(" -> {}", self.type_to_string(t)))
                                    .unwrap_or_default()
                            );

                            completions.push(CompletionItem {
                                label: func.name.clone(),
                                kind: Some(CompletionItemKind::Function),
                                detail: Some(signature),
                                documentation: None,
                                insert_text: Some(format!("{}(", func.name)),
                                insert_text_format: Some(InsertTextFormat::PlainText),
                                additional_text_edits: None,
                            });
                        }
                    }
                    Item::Struct(struct_def) => {
                        if struct_def.name.starts_with(&context.word) {
                            completions.push(CompletionItem {
                                label: struct_def.name.clone(),
                                kind: Some(CompletionItemKind::Struct),
                                detail: Some(format!("struct {}", struct_def.name)),
                                documentation: None,
                                insert_text: Some(struct_def.name.clone()),
                                insert_text_format: Some(InsertTextFormat::PlainText),
                                additional_text_edits: None,
                            });
                        }
                    }
                    Item::Enum(enum_def) => {
                        if enum_def.name.starts_with(&context.word) {
                            completions.push(CompletionItem {
                                label: enum_def.name.clone(),
                                kind: Some(CompletionItemKind::Enum),
                                detail: Some(format!("enum {}", enum_def.name)),
                                documentation: None,
                                insert_text: Some(enum_def.name.clone()),
                                insert_text_format: Some(InsertTextFormat::PlainText),
                                additional_text_edits: None,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        // Built-in functions
        let builtins = vec![
            ("print", "fn print(s: String)", "Print a string to stdout"),
            (
                "print_int",
                "fn print_int(n: i64)",
                "Print an integer to stdout",
            ),
            (
                "string_len",
                "fn string_len(s: String) -> i64",
                "Get string length",
            ),
            (
                "string_concat",
                "fn string_concat(a: String, b: String) -> String",
                "Concatenate strings",
            ),
            (
                "int_to_string",
                "fn int_to_string(n: i64) -> String",
                "Convert integer to string",
            ),
            (
                "string_to_int",
                "fn string_to_int(s: String) -> Option<i64>",
                "Parse integer from string",
            ),
        ];

        for (name, signature, doc) in builtins {
            if name.starts_with(&context.word) {
                completions.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::Function),
                    detail: Some(signature.to_string()),
                    documentation: Some(doc.to_string()),
                    insert_text: Some(format!("{}(", name)),
                    insert_text_format: Some(InsertTextFormat::PlainText),
                    additional_text_edits: None,
                });
            }
        }

        completions
    }

    /// Get member completions (after dot)
    fn get_member_completions(
        &self,
        context: &CompletionContext,
        ast: Option<&Program>,
    ) -> Vec<CompletionItem> {
        let mut completions = Vec::new();
        
        // Extract the expression before the dot
        let line_before_dot = &context.line_text[..context.position.character as usize - 1];
        
        // Simple heuristic: find the identifier before the dot
        let expr_end = line_before_dot.rfind(|c: char| !c.is_alphanumeric() && c != '_');
        let expr_start = expr_end.map(|i| i + 1).unwrap_or(0);
        let var_name = &line_before_dot[expr_start..];
        
        // Look for struct types in AST
        if let Some(ast) = ast {
            // Check if var_name is a known struct type
            for item in &ast.items {
                if let Item::Struct(struct_def) = item {
                    // Simple case: if var name matches struct name (constructor pattern)
                    if var_name == struct_def.name || var_name.ends_with(&format!(" {}", struct_def.name)) {
                        // Add field completions
                        for (field_name, field_ty) in &struct_def.fields {
                            completions.push(CompletionItem {
                                label: field_name.clone(),
                                kind: Some(CompletionItemKind::Field),
                                detail: Some(format!("{}: {}", field_name, self.type_to_string(field_ty))),
                                documentation: None,
                                insert_text: Some(field_name.clone()),
                                insert_text_format: Some(InsertTextFormat::PlainText),
                                additional_text_edits: None,
                            });
                        }
                    }
                }
            }
            
            // String methods
            if var_name.contains("string") || var_name.contains("str") {
                let string_methods = vec![
                    ("len", "fn len() -> i64", "Get the length of the string"),
                    ("is_empty", "fn is_empty() -> bool", "Check if string is empty"),
                    ("to_uppercase", "fn to_uppercase() -> String", "Convert to uppercase"),
                    ("to_lowercase", "fn to_lowercase() -> String", "Convert to lowercase"),
                    ("trim", "fn trim() -> String", "Remove leading and trailing whitespace"),
                    ("split", "fn split(sep: String) -> Vec<String>", "Split string by separator"),
                    ("contains", "fn contains(s: String) -> bool", "Check if string contains substring"),
                    ("starts_with", "fn starts_with(s: String) -> bool", "Check if string starts with prefix"),
                    ("ends_with", "fn ends_with(s: String) -> bool", "Check if string ends with suffix"),
                ];
                
                for (name, signature, doc) in string_methods {
                    if name.starts_with(&context.word) {
                        completions.push(CompletionItem {
                            label: name.to_string(),
                            kind: Some(CompletionItemKind::Method),
                            detail: Some(signature.to_string()),
                            documentation: Some(doc.to_string()),
                            insert_text: Some(format!("{}(", name)),
                            insert_text_format: Some(InsertTextFormat::PlainText),
                            additional_text_edits: None,
                        });
                    }
                }
            }
            
            // Array methods
            if var_name.contains('[') || var_name.contains("array") || var_name.contains("vec") {
                let array_methods = vec![
                    ("len", "fn len() -> i64", "Get the length of the array"),
                    ("is_empty", "fn is_empty() -> bool", "Check if array is empty"),
                    ("push", "fn push(item: T)", "Add an element to the end"),
                    ("pop", "fn pop() -> Option<T>", "Remove and return the last element"),
                    ("get", "fn get(index: i64) -> Option<T>", "Get element at index"),
                    ("first", "fn first() -> Option<T>", "Get the first element"),
                    ("last", "fn last() -> Option<T>", "Get the last element"),
                    ("sort", "fn sort()", "Sort the array in place"),
                    ("reverse", "fn reverse()", "Reverse the array in place"),
                ];
                
                for (name, signature, doc) in array_methods {
                    if name.starts_with(&context.word) {
                        completions.push(CompletionItem {
                            label: name.to_string(),
                            kind: Some(CompletionItemKind::Method),
                            detail: Some(signature.to_string()),
                            documentation: Some(doc.to_string()),
                            insert_text: Some(if name == "push" || name == "get" {
                                format!("{}(", name)
                            } else {
                                format!("{}()", name)
                            }),
                            insert_text_format: Some(InsertTextFormat::PlainText),
                            additional_text_edits: None,
                        });
                    }
                }
            }
        }
        
        completions
    }

    /// Get module completions (after ::)
    fn get_module_completions(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let mut completions = Vec::new();
        
        // Extract the module path before ::
        let line_before_colons = &context.line_text[..context.position.character as usize - 2];
        
        // Find the module/type name
        let expr_end = line_before_colons.rfind(|c: char| !c.is_alphanumeric() && c != '_');
        let expr_start = expr_end.map(|i| i + 1).unwrap_or(0);
        let module_name = &line_before_colons[expr_start..];
        
        // Standard library modules
        if module_name == "std" {
            let std_modules = vec![
                ("io", "Input/output functionality"),
                ("fs", "File system operations"),
                ("collections", "Data structures"),
                ("string", "String utilities"),
                ("math", "Mathematical functions"),
                ("vec", "Vector operations"),
                ("hashmap", "Hash map implementation"),
                ("result", "Result type utilities"),
                ("option", "Option type utilities"),
            ];
            
            for (name, doc) in std_modules {
                if name.starts_with(&context.word) {
                    completions.push(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::Module),
                        detail: Some(format!("mod {}", name)),
                        documentation: Some(doc.to_string()),
                        insert_text: Some(name.to_string()),
                        insert_text_format: Some(InsertTextFormat::PlainText),
                        additional_text_edits: None,
                    });
                }
            }
        }
        
        // Result type methods
        if module_name == "Result" {
            let result_methods = vec![
                ("Ok", "Result::Ok(T) -> Result<T, E>", "Construct a successful result"),
                ("Err", "Result::Err(E) -> Result<T, E>", "Construct an error result"),
            ];
            
            for (name, signature, doc) in result_methods {
                if name.starts_with(&context.word) {
                    completions.push(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::Constructor),
                        detail: Some(signature.to_string()),
                        documentation: Some(doc.to_string()),
                        insert_text: Some(format!("{}(", name)),
                        insert_text_format: Some(InsertTextFormat::PlainText),
                        additional_text_edits: None,
                    });
                }
            }
        }
        
        // Option type methods
        if module_name == "Option" {
            let option_methods = vec![
                ("Some", "Option::Some(T) -> Option<T>", "Construct a Some value"),
                ("None", "Option::None -> Option<T>", "Construct a None value"),
            ];
            
            for (name, signature, doc) in option_methods {
                if name.starts_with(&context.word) {
                    completions.push(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::Constructor),
                        detail: Some(signature.to_string()),
                        documentation: Some(doc.to_string()),
                        insert_text: Some(if name == "Some" {
                            format!("{}(", name)
                        } else {
                            name.to_string()
                        }),
                        insert_text_format: Some(InsertTextFormat::PlainText),
                        additional_text_edits: None,
                    });
                }
            }
        }
        
        // Enum variants from AST
        if let Some(doc) = self.documents.get(&context.position.line.to_string()) {
            if let Some(ast) = &doc.ast {
                for item in &ast.items {
                    if let Item::Enum(enum_def) = item {
                        if enum_def.name == module_name {
                            for variant in &enum_def.variants {
                                if variant.name.starts_with(&context.word) {
                                    let has_fields = match &variant.data {
                                        crate::ast::EnumVariantData::Unit => false,
                                        crate::ast::EnumVariantData::Tuple(_) => true,
                                        crate::ast::EnumVariantData::Struct(_) => true,
                                    };
                                    completions.push(CompletionItem {
                                        label: variant.name.clone(),
                                        kind: Some(CompletionItemKind::EnumMember),
                                        detail: Some(format!("{}::{}", enum_def.name, variant.name)),
                                        documentation: None,
                                        insert_text: Some(if has_fields {
                                            format!("{}(", variant.name)
                                        } else {
                                            variant.name.clone()
                                        }),
                                        insert_text_format: Some(InsertTextFormat::PlainText),
                                        additional_text_edits: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        
        completions
    }

    /// Convert type to string
    #[allow(clippy::only_used_in_recursion)]
    pub fn type_to_string(&self, ty: &Type) -> String {
        match ty {
            Type::I32 => "i32".to_string(),
            Type::I64 => "i64".to_string(),
            Type::U32 => "u32".to_string(),
            Type::U64 => "u64".to_string(),
            Type::Bool => "bool".to_string(),
            Type::String => "String".to_string(),
            Type::Unit => "()".to_string(),
            Type::Custom(name) => name.clone(),
            Type::Array(elem, size) => format!("[{}; {}]", self.type_to_string(elem), size),
            Type::Reference { mutable, inner, .. } => {
                if *mutable {
                    format!("&mut {}", self.type_to_string(inner))
                } else {
                    format!("&{}", self.type_to_string(inner))
                }
            }
            Type::Future { output } => format!("Future<{}>", self.type_to_string(output)),
            Type::Generic { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let arg_strs: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
                    format!("{}<{}>", name, arg_strs.join(", "))
                }
            }
            Type::TypeParam(name) => name.clone(),
            Type::Tuple(types) => {
                let type_strs: Vec<String> = types.iter().map(|t| self.type_to_string(t)).collect();
                format!("({})", type_strs.join(", "))
            }
        }
    }
}
