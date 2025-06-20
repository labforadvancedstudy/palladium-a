// Symbol information provider
// "Navigate your code with legendary precision"

use super::{LanguageServer, Location, Range, SymbolKind};
use serde::{Deserialize, Serialize};

/// Document symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbol {
    /// The name of this symbol
    pub name: String,
    /// More detail for this symbol
    pub detail: Option<String>,
    /// The kind of this symbol
    pub kind: SymbolKind,
    /// The range enclosing this symbol
    pub range: Range,
    /// The range that should be selected and revealed
    pub selection_range: Range,
    /// Children of this symbol
    pub children: Vec<DocumentSymbol>,
}

/// Symbol information for workspace symbol search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInformation {
    /// The name of this symbol
    pub name: String,
    /// The kind of this symbol
    pub kind: SymbolKind,
    /// The location of this symbol
    pub location: Location,
    /// The name of the symbol containing this symbol
    pub container_name: Option<String>,
}

impl LanguageServer {
    /// Get document symbols
    pub fn get_document_symbols(&self, uri: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();

        let doc = match self.documents.get(uri) {
            Some(doc) => doc,
            None => return symbols,
        };

        let ast = match &doc.ast {
            Some(ast) => ast,
            None => return symbols,
        };

        for item in &ast.items {
            match item {
                crate::ast::Item::Function(func) => {
                    let children = self.get_function_symbols(func);

                    symbols.push(DocumentSymbol {
                        name: func.name.clone(),
                        detail: Some(self.function_signature(func)),
                        kind: SymbolKind::Function,
                        range: self.span_to_range(func.span),
                        selection_range: self.span_to_range(func.span),
                        children,
                    });
                }
                crate::ast::Item::Struct(struct_def) => {
                    let mut children = Vec::new();

                    for (field_name, field_ty) in &struct_def.fields {
                        children.push(DocumentSymbol {
                            name: field_name.clone(),
                            detail: Some(self.type_to_string(field_ty)),
                            kind: SymbolKind::Field,
                            range: self.span_to_range(struct_def.span),
                            selection_range: self.span_to_range(struct_def.span),
                            children: Vec::new(),
                        });
                    }

                    symbols.push(DocumentSymbol {
                        name: struct_def.name.clone(),
                        detail: Some(format!("struct {}", struct_def.name)),
                        kind: SymbolKind::Struct,
                        range: self.span_to_range(struct_def.span),
                        selection_range: self.span_to_range(struct_def.span),
                        children,
                    });
                }
                crate::ast::Item::Enum(enum_def) => {
                    let mut children = Vec::new();

                    for variant in &enum_def.variants {
                        let detail = match &variant.data {
                            crate::ast::EnumVariantData::Unit => None,
                            crate::ast::EnumVariantData::Tuple(types) => {
                                let type_strs: Vec<String> =
                                    types.iter().map(|t| self.type_to_string(t)).collect();
                                Some(format!("({})", type_strs.join(", ")))
                            }
                            crate::ast::EnumVariantData::Struct(fields) => {
                                let field_strs: Vec<String> = fields
                                    .iter()
                                    .map(|(name, ty)| {
                                        format!("{}: {}", name, self.type_to_string(ty))
                                    })
                                    .collect();
                                Some(format!("{{ {} }}", field_strs.join(", ")))
                            }
                        };

                        children.push(DocumentSymbol {
                            name: variant.name.clone(),
                            detail,
                            kind: SymbolKind::EnumVariant,
                            range: self.span_to_range(enum_def.span),
                            selection_range: self.span_to_range(enum_def.span),
                            children: Vec::new(),
                        });
                    }

                    symbols.push(DocumentSymbol {
                        name: enum_def.name.clone(),
                        detail: Some(format!("enum {}", enum_def.name)),
                        kind: SymbolKind::Enum,
                        range: self.span_to_range(enum_def.span),
                        selection_range: self.span_to_range(enum_def.span),
                        children,
                    });
                }
                crate::ast::Item::Trait(trait_def) => {
                    let mut children = Vec::new();

                    for method in &trait_def.methods {
                        children.push(DocumentSymbol {
                            name: method.name.clone(),
                            detail: Some(self.method_signature(method)),
                            kind: SymbolKind::Method,
                            range: self.span_to_range(method.span),
                            selection_range: self.span_to_range(method.span),
                            children: Vec::new(),
                        });
                    }

                    symbols.push(DocumentSymbol {
                        name: trait_def.name.clone(),
                        detail: Some(format!("trait {}", trait_def.name)),
                        kind: SymbolKind::Trait,
                        range: self.span_to_range(trait_def.span),
                        selection_range: self.span_to_range(trait_def.span),
                        children,
                    });
                }
                crate::ast::Item::TypeAlias(type_alias) => {
                    symbols.push(DocumentSymbol {
                        name: type_alias.name.clone(),
                        detail: Some(format!(
                            "type {} = {}",
                            type_alias.name,
                            self.type_to_string(&type_alias.ty)
                        )),
                        kind: SymbolKind::TypeAlias,
                        range: self.span_to_range(type_alias.span),
                        selection_range: self.span_to_range(type_alias.span),
                        children: Vec::new(),
                    });
                }
                _ => {}
            }
        }

        symbols
    }

    /// Get workspace symbols matching query
    pub fn get_workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let mut symbols = Vec::new();
        let query_lower = query.to_lowercase();

        for symbol_list in self.symbol_index.symbols.values() {
            for symbol in symbol_list {
                if symbol.name.to_lowercase().contains(&query_lower) {
                    symbols.push(SymbolInformation {
                        name: symbol.name.clone(),
                        kind: symbol.kind,
                        location: symbol.location.clone(),
                        container_name: symbol.container_name.clone(),
                    });
                }
            }
        }

        symbols
    }

    /// Get symbols from a function
    fn get_function_symbols(&self, func: &crate::ast::Function) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();

        // Add parameters as symbols
        for param in &func.params {
            symbols.push(DocumentSymbol {
                name: param.name.clone(),
                detail: Some(self.type_to_string(&param.ty)),
                kind: SymbolKind::Variable,
                range: self.span_to_range(func.span),
                selection_range: self.span_to_range(func.span),
                children: Vec::new(),
            });
        }

        // TODO: Parse function body for local variables

        symbols
    }

    /// Get function signature
    fn function_signature(&self, func: &crate::ast::Function) -> String {
        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.type_to_string(&p.ty)))
            .collect();

        if let Some(ret) = &func.return_type {
            format!(
                "fn {}({}) -> {}",
                func.name,
                params.join(", "),
                self.type_to_string(ret)
            )
        } else {
            format!("fn {}({})", func.name, params.join(", "))
        }
    }

    /// Get method signature
    fn method_signature(&self, method: &crate::ast::TraitMethod) -> String {
        let params: Vec<String> = method
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.type_to_string(&p.ty)))
            .collect();

        if let Some(ret) = &method.return_type {
            format!(
                "fn {}({}) -> {}",
                method.name,
                params.join(", "),
                self.type_to_string(ret)
            )
        } else {
            format!("fn {}({})", method.name, params.join(", "))
        }
    }
}
