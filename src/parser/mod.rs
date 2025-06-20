// Parser for Palladium
// "Constructing legends from tokens"

use crate::ast::{AssignTarget, Param, UnaryOp, *};
use crate::errors::{CompileError, Result, Span};
use crate::lexer::Token;

pub struct Parser {
    tokens: Vec<(Token, Span)>,
    current: usize,
    /// Type parameters currently in scope (for parsing generic functions)
    type_params_in_scope: Vec<String>,
    /// Cache for current token to avoid repeated bounds checking
    current_token_cache: Option<(Token, Span)>,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, Span)>) -> Self {
        let current_token_cache = if !tokens.is_empty() {
            Some(tokens[0].clone())
        } else {
            None
        };

        Self {
            tokens,
            current: 0,
            type_params_in_scope: Vec::new(),
            current_token_cache,
        }
    }

    /// Parse generic parameters (<'a, T, const N: usize>)
    #[allow(clippy::type_complexity)]
    fn parse_generic_params(&mut self) -> Result<(Vec<String>, Vec<String>, Vec<(String, Type)>)> {
        let mut lifetime_params = Vec::new();
        let mut type_params = Vec::new();
        let mut const_params = Vec::new();

        if self.check(&Token::Lt) {
            self.advance()?; // consume '<'

            loop {
                // Check if it's a lifetime parameter
                if self.check(&Token::SingleQuote) {
                    self.advance()?; // consume single quote
                    let lifetime_name = match self.advance()? {
                        (Token::Identifier(name), _) => format!("'{}", name),
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    lifetime_params.push(lifetime_name);
                } else if self.check(&Token::Const) {
                    // It's a const parameter
                    self.advance()?; // consume 'const'
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "const parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    self.consume(Token::Colon, "Expected ':' after const parameter name")?;
                    let param_type = self.parse_type()?;
                    const_params.push((param_name, param_type));
                } else {
                    // It's a type parameter
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "type parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    type_params.push(param_name.clone());
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?; // consume ','
            }

            self.consume(Token::Gt, "Expected '>' after generic parameters")?;
        }

        Ok((lifetime_params, type_params, const_params))
    }

    /// Get current token
    pub fn current_token(&self) -> &Token {
        if let Some((ref token, _)) = self.current_token_cache {
            token
        } else {
            &Token::Eof
        }
    }

    /// Update cache when current position changes
    fn update_cache(&mut self) {
        self.current_token_cache = if self.current < self.tokens.len() {
            Some(self.tokens[self.current].clone())
        } else {
            None
        };
    }

    /// Get the current span for error reporting
    fn current_span(&self) -> Option<crate::errors::Span> {
        if self.current < self.tokens.len() {
            let span = &self.tokens[self.current].1;
            Some(crate::errors::Span::new(
                span.start,
                span.end,
                span.line,
                span.column,
            ))
        } else if self.current > 0 && self.current - 1 < self.tokens.len() {
            // Use previous token's span if at end
            let span = &self.tokens[self.current - 1].1;
            Some(crate::errors::Span::new(
                span.start,
                span.end,
                span.line,
                span.column,
            ))
        } else {
            None
        }
    }

    /// Get span from an expression
    fn expr_span(expr: &Expr) -> Span {
        match expr {
            // Expressions without span field return dummy span for now
            Expr::Integer(_) => Span::dummy(),
            Expr::String(_) => Span::dummy(),
            Expr::Bool(_) => Span::dummy(),
            Expr::Ident(_) => Span::dummy(),
            // Expressions with span field
            Expr::Binary { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::ArrayLiteral { span, .. } => *span,
            Expr::ArrayRepeat { span, .. } => *span,
            Expr::StructLiteral { span, .. } => *span,
            Expr::FieldAccess { span, .. } => *span,
            Expr::EnumConstructor { span, .. } => *span,
            Expr::Range { span, .. } => *span,
            Expr::Reference { span, .. } => *span,
            Expr::Deref { span, .. } => *span,
            Expr::Question { span, .. } => *span,
            Expr::MacroInvocation { span, .. } => *span,
            Expr::Await { span, .. } => *span,
        }
    }

    /// Parse a complete program
    pub fn parse(&mut self) -> Result<Program> {
        let mut imports = Vec::new();
        let mut items = Vec::new();

        // Parse imports first
        while self.check(&Token::Import) {
            imports.push(self.parse_import()?);
        }

        // Then parse items
        while !self.is_at_end() {
            items.push(self.parse_item()?);
        }

        Ok(Program { imports, items })
    }

    /// Parse an import statement
    fn parse_import(&mut self) -> Result<crate::ast::Import> {
        let start_span = self.consume(Token::Import, "Expected 'import'")?;

        let mut path = Vec::new();
        let mut items = None;

        // Parse first part of path
        let first = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "module name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };
        path.push(first);

        // Parse remaining path segments
        while self.check(&Token::DoubleColon) {
            self.advance()?; // consume '::'

            // Check if this might be a specific item import
            if matches!(self.peek(), Ok(Token::Identifier(_))) {
                // Look ahead to see if this is the last segment (followed by ; or ,)
                let next_is_terminator = self
                    .tokens
                    .get(self.current + 1)
                    .map(|(t, _)| matches!(t, Token::Semicolon | Token::Comma | Token::LeftBrace))
                    .unwrap_or(false);

                if next_is_terminator {
                    // This is a specific item import
                    items = Some(self.parse_import_items()?);
                    break;
                } else {
                    // This is another module in the path
                    let segment = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "module name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    path.push(segment);
                }
            } else if self.check(&Token::Star) {
                // Wildcard import
                self.advance()?; // consume '*'
                items = None; // Explicitly None for wildcard
                break;
            } else if self.check(&Token::LeftBrace) {
                // Multiple item import: import std::math::{pd_abs, pd_sin}
                items = Some(self.parse_import_items()?);
                break;
            } else {
                return Err(CompileError::UnexpectedToken {
                    expected: "module name, item name, or '*'".to_string(),
                    found: self.peek()?.to_string(),
                    span: self.current_span(),
                });
            }
        }

        // Parse optional alias
        let mut alias = None;
        if self.check(&Token::As) {
            self.advance()?; // consume 'as'
            match self.advance()? {
                (Token::Identifier(name), _) => {
                    alias = Some(name);
                }
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "alias name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            }
        }

        let end_span = self.consume(Token::Semicolon, "Expected ';' after import")?;

        Ok(crate::ast::Import {
            path,
            items,
            alias,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse import items - either single item or multiple items in braces
    fn parse_import_items(&mut self) -> Result<Vec<String>> {
        let mut items = Vec::new();

        if self.check(&Token::LeftBrace) {
            // Multiple items: {pd_abs, pd_sin, pd_cos}
            self.advance()?; // consume '{'

            loop {
                match self.advance()? {
                    (Token::Identifier(name), _) => {
                        items.push(name);

                        if self.check(&Token::RightBrace) {
                            self.advance()?; // consume '}'
                            break;
                        } else {
                            self.consume(Token::Comma, "Expected ',' or '}' after import item")?;
                            // Allow trailing comma
                            if self.check(&Token::RightBrace) {
                                self.advance()?; // consume '}'
                                break;
                            }
                        }
                    }
                    (token, _) => {
                        return Err(CompileError::UnexpectedToken {
                            expected: "item name".to_string(),
                            found: token.to_string(),
                            span: self.current_span(),
                        });
                    }
                }
            }
        } else {
            // Single item
            match self.advance()? {
                (Token::Identifier(name), _) => {
                    items.push(name);
                }
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "item name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            }
        }

        Ok(items)
    }

    /// Parse a top-level item
    fn parse_item(&mut self) -> Result<Item> {
        // Check for visibility modifier
        let visibility = if self.check(&Token::Pub) {
            self.advance()?; // consume 'pub'
            crate::ast::Visibility::Public
        } else {
            crate::ast::Visibility::Private
        };

        // Check for async modifier
        let is_async = if self.check(&Token::Async) {
            self.advance()?; // consume 'async'
            true
        } else {
            false
        };

        match self.peek()? {
            Token::Fn => {
                let mut func = self.parse_function()?;
                func.visibility = visibility;
                func.is_async = is_async;
                Ok(Item::Function(func))
            }
            Token::Struct => {
                if is_async {
                    return Err(CompileError::SyntaxError {
                        message: "async can only be used with functions".to_string(),
                        span: self.current_span(),
                    });
                }
                let mut struct_def = self.parse_struct()?;
                struct_def.visibility = visibility;
                Ok(Item::Struct(struct_def))
            }
            Token::Enum => Ok(Item::Enum(self.parse_enum()?)),
            Token::Trait => {
                let mut trait_def = self.parse_trait()?;
                trait_def.visibility = visibility;
                Ok(Item::Trait(trait_def))
            }
            Token::Impl => Ok(Item::Impl(self.parse_impl()?)),
            Token::Type => {
                let mut type_alias = self.parse_type_alias()?;
                type_alias.visibility = visibility;
                Ok(Item::TypeAlias(type_alias))
            }
            Token::Macro => Ok(Item::Macro(self.parse_macro()?)),
            _ => {
                if is_async {
                    Err(CompileError::SyntaxError {
                        message: "async can only be used with function declarations".to_string(),
                        span: self.current_span(),
                    })
                } else {
                    Err(CompileError::SyntaxError {
                        message: "Expected function, struct, enum, trait, type, impl, or macro declaration".to_string(),
                        span: self.current_span(),
                    })
                }
            }
        }
    }

    /// Parse a function declaration
    fn parse_function(&mut self) -> Result<Function> {
        let start_span = self.consume(Token::Fn, "Expected 'fn'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "function name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes, types, and consts) if present
        let (lifetime_params, type_params, const_params) = self.parse_generic_params()?;

        // Set type parameters in scope for parsing function signature and body
        self.type_params_in_scope = type_params.clone();

        self.consume(Token::LeftParen, "Expected '('")?;

        // Parse function parameters
        let mut params = Vec::new();

        if !self.check(&Token::RightParen) {
            loop {
                // Check for optional 'mut' keyword
                let mutable = if self.check(&Token::Mut) {
                    self.advance()?; // consume 'mut'
                    true
                } else {
                    false
                };

                // Parse parameter name
                let param_name = match self.advance()? {
                    (Token::Identifier(name), _) => name,
                    (token, _) => {
                        return Err(CompileError::UnexpectedToken {
                            expected: "parameter name".to_string(),
                            found: token.to_string(),
                            span: self.current_span(),
                        });
                    }
                };

                // Parse parameter type
                self.consume(Token::Colon, "Expected ':' after parameter name")?;
                let param_type = self.parse_type()?;

                params.push(Param {
                    name: param_name,
                    ty: param_type,
                    mutable,
                });

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?; // consume ','
            }
        }

        self.consume(Token::RightParen, "Expected ')'")?;

        // Parse return type if present
        let return_type = if self.check(&Token::Arrow) {
            self.advance()?; // consume '->'
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(Token::LeftBrace, "Expected '{'")?;

        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}'")?;

        // Clear type parameters from scope
        self.type_params_in_scope.clear();

        Ok(Function {
            visibility: crate::ast::Visibility::Private, // TODO: parse pub keyword
            is_async: false,                             // Will be set by parse_item
            name,
            lifetime_params,
            type_params,
            const_params,
            params,
            return_type,
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
            effects: None, // Effects will be inferred during analysis
        })
    }

    /// Parse a struct definition
    fn parse_struct(&mut self) -> Result<StructDef> {
        let start_span = self.consume(Token::Struct, "Expected 'struct'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "struct name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes, types, and consts) if present
        let (lifetime_params, type_params, const_params) = self.parse_generic_params()?;

        self.consume(Token::LeftBrace, "Expected '{' after struct name")?;

        let mut fields = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // Parse field name
            let field_name = match self.advance()? {
                (Token::Identifier(name), _) => name,
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "field name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            };

            self.consume(Token::Colon, "Expected ':' after field name")?;
            let field_type = self.parse_type()?;

            fields.push((field_name, field_type));

            // Fields are separated by commas
            if !self.check(&Token::RightBrace) {
                self.consume(Token::Comma, "Expected ',' after field")?;
            }
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after struct fields")?;

        Ok(StructDef {
            visibility: crate::ast::Visibility::Private, // TODO: parse pub keyword
            name,
            lifetime_params,
            type_params,
            const_params,
            fields,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse an enum definition
    fn parse_enum(&mut self) -> Result<EnumDef> {
        let start_span = self.consume(Token::Enum, "Expected 'enum'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "enum name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes, types, and consts) if present
        let (lifetime_params, type_params, const_params) = self.parse_generic_params()?;

        self.consume(Token::LeftBrace, "Expected '{' after enum name")?;

        let mut variants = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // Parse variant name
            let variant_name = match self.advance()? {
                (Token::Identifier(name), _) => name,
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "variant name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            };

            // Parse variant data
            let data = if self.check(&Token::LeftParen) {
                // Tuple variant
                self.advance()?; // consume '('
                let mut types = Vec::new();

                if !self.check(&Token::RightParen) {
                    loop {
                        types.push(self.parse_type()?);
                        if !self.check(&Token::Comma) {
                            break;
                        }
                        self.advance()?; // consume ','
                    }
                }

                self.consume(Token::RightParen, "Expected ')' after tuple variant types")?;
                EnumVariantData::Tuple(types)
            } else if self.check(&Token::LeftBrace) {
                // Struct variant
                self.advance()?; // consume '{'
                let mut fields = Vec::new();

                while !self.check(&Token::RightBrace) && !self.is_at_end() {
                    let field_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "field name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    self.consume(Token::Colon, "Expected ':' after field name")?;
                    let field_type = self.parse_type()?;

                    fields.push((field_name, field_type));

                    if !self.check(&Token::RightBrace) {
                        self.consume(Token::Comma, "Expected ',' after field")?;
                    }
                }

                self.consume(
                    Token::RightBrace,
                    "Expected '}' after struct variant fields",
                )?;
                EnumVariantData::Struct(fields)
            } else {
                // Unit variant
                EnumVariantData::Unit
            };

            variants.push(EnumVariant {
                name: variant_name,
                data,
            });

            // Variants are separated by commas
            if !self.check(&Token::RightBrace) {
                self.consume(Token::Comma, "Expected ',' after variant")?;
            }
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after enum variants")?;

        Ok(EnumDef {
            name,
            lifetime_params,
            type_params,
            const_params,
            variants,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a trait definition
    fn parse_trait(&mut self) -> Result<TraitDef> {
        let start_span = self.consume(Token::Trait, "Expected 'trait'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "trait name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes and types) if present
        let mut lifetime_params = Vec::new();
        let mut type_params = Vec::new();

        if self.check(&Token::Lt) {
            self.advance()?; // consume '<'

            loop {
                // Check if it's a lifetime parameter
                if self.check(&Token::SingleQuote) {
                    self.advance()?; // consume single quote
                    let lifetime_name = match self.advance()? {
                        (Token::Identifier(name), _) => format!("'{}", name),
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    lifetime_params.push(lifetime_name);
                } else {
                    // It's a type parameter
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "type parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    type_params.push(param_name);
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?; // consume ','
            }

            self.consume(Token::Gt, "Expected '>' after generic parameters")?;
        }

        self.consume(Token::LeftBrace, "Expected '{' after trait name")?;

        let mut methods = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // Parse method
            let method_start = self.consume(Token::Fn, "Expected 'fn' for trait method")?;

            let method_name = match self.advance()? {
                (Token::Identifier(name), _) => name,
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "method name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            };

            // Parse method generic parameters
            let mut method_lifetime_params = Vec::new();
            let mut method_type_params = Vec::new();

            if self.check(&Token::Lt) {
                self.advance()?; // consume '<'

                loop {
                    if self.check(&Token::SingleQuote) {
                        self.advance()?;
                        let lifetime_name = match self.advance()? {
                            (Token::Identifier(name), _) => format!("'{}", name),
                            (token, _) => {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "lifetime name".to_string(),
                                    found: token.to_string(),
                                    span: self.current_span(),
                                });
                            }
                        };
                        method_lifetime_params.push(lifetime_name);
                    } else {
                        let param_name = match self.advance()? {
                            (Token::Identifier(name), _) => name,
                            (token, _) => {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "type parameter name".to_string(),
                                    found: token.to_string(),
                                    span: self.current_span(),
                                });
                            }
                        };
                        method_type_params.push(param_name);
                    }

                    if !self.check(&Token::Comma) {
                        break;
                    }
                    self.advance()?;
                }

                self.consume(Token::Gt, "Expected '>' after generic parameters")?;
            }

            // Parse parameters
            self.consume(Token::LeftParen, "Expected '(' after method name")?;
            let mut params = Vec::new();

            if !self.check(&Token::RightParen) {
                loop {
                    let mutable = if self.check(&Token::Mut) {
                        self.advance()?;
                        true
                    } else {
                        false
                    };

                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    self.consume(Token::Colon, "Expected ':' after parameter name")?;
                    let param_type = self.parse_type()?;

                    params.push(Param {
                        name: param_name,
                        ty: param_type,
                        mutable,
                    });

                    if !self.check(&Token::Comma) {
                        break;
                    }
                    self.advance()?;
                }
            }

            self.consume(Token::RightParen, "Expected ')'")?;

            // Parse return type
            let return_type = if self.check(&Token::Arrow) {
                self.advance()?;
                Some(self.parse_type()?)
            } else {
                None
            };

            // Check if method has body
            let (has_body, body) = if self.check(&Token::LeftBrace) {
                self.advance()?; // consume '{'
                let mut stmts = Vec::new();
                while !self.check(&Token::RightBrace) && !self.is_at_end() {
                    stmts.push(self.parse_statement()?);
                }
                let _method_end = self.consume(Token::RightBrace, "Expected '}'")?;
                (true, Some(stmts))
            } else {
                self.consume(
                    Token::Semicolon,
                    "Expected ';' after trait method signature",
                )?;
                (false, None)
            };

            methods.push(TraitMethod {
                name: method_name,
                lifetime_params: method_lifetime_params,
                type_params: method_type_params,
                params,
                return_type,
                has_body,
                body,
                span: Span::new(
                    method_start.start,
                    self.current_span()
                        .map(|s| s.end)
                        .unwrap_or(method_start.end),
                    method_start.line,
                    method_start.column,
                ),
            });
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after trait methods")?;

        Ok(TraitDef {
            visibility: crate::ast::Visibility::Private, // Will be set by caller
            name,
            lifetime_params,
            type_params,
            methods,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse an impl block
    fn parse_impl(&mut self) -> Result<ImplBlock> {
        let start_span = self.consume(Token::Impl, "Expected 'impl'")?;

        // Parse generic parameters
        let mut lifetime_params = Vec::new();
        let mut type_params = Vec::new();

        if self.check(&Token::Lt) {
            self.advance()?; // consume '<'

            loop {
                if self.check(&Token::SingleQuote) {
                    self.advance()?;
                    let lifetime_name = match self.advance()? {
                        (Token::Identifier(name), _) => format!("'{}", name),
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    lifetime_params.push(lifetime_name);
                } else {
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "type parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    type_params.push(param_name);
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?;
            }

            self.consume(Token::Gt, "Expected '>' after generic parameters")?;
        }

        // First, try to parse a type
        let first_type = self.parse_type()?;

        // Check if this is a trait impl (has 'for' keyword)
        let (trait_type, for_type) = if self.check(&Token::For) {
            self.advance()?; // consume 'for'
            let impl_type = self.parse_type()?;
            (Some(first_type), impl_type)
        } else {
            // This is an inherent impl
            (None, first_type)
        };

        self.consume(Token::LeftBrace, "Expected '{' after impl type")?;

        let mut methods = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // For now, only support fn methods in impl blocks
            if !self.check(&Token::Fn) {
                return Err(CompileError::UnexpectedToken {
                    expected: "'fn' for method".to_string(),
                    found: self.peek()?.to_string(),
                    span: self.current_span(),
                });
            }
            let method = self.parse_function()?;
            methods.push(method);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after impl methods")?;

        Ok(ImplBlock {
            lifetime_params,
            type_params,
            trait_type,
            for_type,
            methods,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a type alias definition
    fn parse_type_alias(&mut self) -> Result<TypeAlias> {
        let start_span = self.consume(Token::Type, "Expected 'type'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "type alias name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes and types) if present
        let mut lifetime_params = Vec::new();
        let mut type_params = Vec::new();

        if self.check(&Token::Lt) {
            self.advance()?; // consume '<'

            loop {
                // Check if it's a lifetime parameter
                if self.check(&Token::SingleQuote) {
                    self.advance()?; // consume single quote
                    let lifetime_name = match self.advance()? {
                        (Token::Identifier(name), _) => format!("'{}", name),
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    lifetime_params.push(lifetime_name);
                } else {
                    // It's a type parameter
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "type parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    type_params.push(param_name);
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?; // consume ','
            }

            self.consume(Token::Gt, "Expected '>' after generic parameters")?;
        }

        self.consume(Token::Eq, "Expected '=' after type alias name")?;

        let ty = self.parse_type()?;

        let end_span = self.consume(Token::Semicolon, "Expected ';' after type alias")?;

        Ok(TypeAlias {
            visibility: Visibility::Private, // Will be set in parse_item
            name,
            lifetime_params,
            type_params,
            ty,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a macro definition
    fn parse_macro(&mut self) -> Result<MacroDef> {
        let start_span = self.consume(Token::Macro, "Expected 'macro'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "macro name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Expect '!' after macro name
        self.consume(Token::Not, "Expected '!' after macro name")?;

        // Parse parameter list in parentheses (optional for now)
        let params = if self.check(&Token::LeftParen) {
            self.advance()?; // consume '('
            let mut params = Vec::new();

            while !self.check(&Token::RightParen) && !self.is_at_end() {
                match self.advance()? {
                    (Token::Identifier(param), _) => {
                        params.push(param);

                        if self.check(&Token::Comma) {
                            self.advance()?; // consume ','
                        } else if !self.check(&Token::RightParen) {
                            return Err(CompileError::SyntaxError {
                                message: "Expected ',' or ')' in macro parameters".to_string(),
                                span: self.current_span(),
                            });
                        }
                    }
                    (token, _) => {
                        return Err(CompileError::UnexpectedToken {
                            expected: "parameter name".to_string(),
                            found: token.to_string(),
                            span: self.current_span(),
                        });
                    }
                }
            }

            self.consume(Token::RightParen, "Expected ')' after macro parameters")?;
            params
        } else {
            Vec::new()
        };

        // Parse macro body (for now, just collect tokens between braces)
        self.consume(Token::LeftBrace, "Expected '{' to start macro body")?;

        let mut body = Vec::new();
        let mut brace_depth = 1;

        while brace_depth > 0 && !self.is_at_end() {
            let (token, _) = self.advance()?;

            match &token {
                Token::LeftBrace => {
                    brace_depth += 1;
                    body.push(self.token_to_ast_token(token));
                }
                Token::RightBrace => {
                    brace_depth -= 1;
                    if brace_depth > 0 {
                        body.push(self.token_to_ast_token(token));
                    }
                }
                _ => {
                    body.push(self.token_to_ast_token(token));
                }
            }
        }

        let end_span = self.current_span().unwrap_or(start_span);

        Ok(MacroDef {
            name,
            params,
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Convert lexer token to AST token for macro body
    fn token_to_ast_token(&self, token: Token) -> crate::ast::Token {
        use crate::ast::Token as AstToken;

        match token {
            Token::Identifier(s) => AstToken::Ident(s),
            Token::String(s) => AstToken::Literal(format!("\"{}\"", s)),
            Token::Integer(n) => AstToken::Literal(n.to_string()),
            Token::True => AstToken::Literal("true".to_string()),
            Token::False => AstToken::Literal("false".to_string()),
            Token::LeftParen => AstToken::Punct('('),
            Token::RightParen => AstToken::Punct(')'),
            Token::LeftBrace => AstToken::Punct('{'),
            Token::RightBrace => AstToken::Punct('}'),
            Token::LeftBracket => AstToken::Punct('['),
            Token::RightBracket => AstToken::Punct(']'),
            Token::Semicolon => AstToken::Punct(';'),
            Token::Comma => AstToken::Punct(','),
            Token::Dot => AstToken::Punct('.'),
            Token::Plus => AstToken::Punct('+'),
            Token::Minus => AstToken::Punct('-'),
            Token::Star => AstToken::Punct('*'),
            Token::Slash => AstToken::Punct('/'),
            Token::Not => AstToken::Punct('!'),
            Token::Eq => AstToken::Punct('='),
            _ => AstToken::Ident(format!("{:?}", token)), // Fallback for other tokens
        }
    }

    /// Parse a statement
    pub fn parse_statement(&mut self) -> Result<Stmt> {
        match self.peek()? {
            Token::Let => self.parse_let(),
            Token::Return => self.parse_return(),
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Break => self.parse_break(),
            Token::Continue => self.parse_continue(),
            Token::Match => self.parse_match(),
            Token::Unsafe => self.parse_unsafe(),
            Token::Identifier(_) | Token::Star => {
                // Could be assignment or expression statement
                // Parse the left-hand side as an expression first
                let checkpoint = self.current;
                let expr = self.parse_expression()?; // Parse full expression including dereference

                // Check if this is an assignment
                if self.check(&Token::Eq) && !self.check_at(1, &Token::Eq) {
                    // This is an assignment
                    let start_span = expr.span();
                    self.advance()?; // consume '='
                    let value = self.parse_expression()?;
                    let end_span =
                        self.consume(Token::Semicolon, "Expected ';' after assignment")?;

                    // Convert expression to assignment target
                    let target = match expr {
                        Expr::Ident(name) => AssignTarget::Ident(name),
                        Expr::Index { array, index, .. } => AssignTarget::Index { array, index },
                        Expr::FieldAccess { object, field, .. } => {
                            AssignTarget::FieldAccess { object, field }
                        }
                        Expr::Deref { expr, .. } => AssignTarget::Deref { expr },
                        _ => {
                            return Err(CompileError::SyntaxError {
                                message: "Invalid assignment target".to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    return Ok(Stmt::Assign {
                        target,
                        value,
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    });
                }

                // Not an assignment, continue parsing as expression
                self.current = checkpoint;
                let expr = self.parse_expression()?;
                self.consume(Token::Semicolon, "Expected ';' after expression")?;
                Ok(Stmt::Expr(expr))
            }
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                self.consume(Token::Semicolon, "Expected ';' after expression")?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    /// Parse a return statement
    fn parse_return(&mut self) -> Result<Stmt> {
        self.consume(Token::Return, "Expected 'return'")?;

        if self.check(&Token::Semicolon) {
            self.advance()?;
            Ok(Stmt::Return(None))
        } else {
            let expr = self.parse_expression()?;
            self.consume(Token::Semicolon, "Expected ';' after return value")?;
            Ok(Stmt::Return(Some(expr)))
        }
    }

    /// Parse a let statement
    fn parse_let(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Let, "Expected 'let'")?;

        // Check for optional 'mut' keyword
        let mutable = if self.check(&Token::Mut) {
            self.advance()?; // consume 'mut'
            true
        } else {
            false
        };

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "variable name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Optional type annotation
        let ty = if self.check(&Token::Colon) {
            self.advance()?; // consume ':'
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(Token::Eq, "Expected '=' after variable name")?;
        let value = self.parse_expression()?;
        let end_span = self.consume(Token::Semicolon, "Expected ';' after let statement")?;

        Ok(Stmt::Let {
            name,
            ty,
            value,
            mutable,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse an if statement
    fn parse_if(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::If, "Expected 'if'")?;

        let condition = self.parse_expression()?;

        self.consume(Token::LeftBrace, "Expected '{' after if condition")?;

        let mut then_branch = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            then_branch.push(self.parse_statement()?);
        }

        self.consume(Token::RightBrace, "Expected '}' after if body")?;

        let else_branch = if self.check(&Token::Else) {
            self.advance()?; // consume 'else'
            self.consume(Token::LeftBrace, "Expected '{' after else")?;

            let mut else_stmts = Vec::new();
            while !self.check(&Token::RightBrace) && !self.is_at_end() {
                else_stmts.push(self.parse_statement()?);
            }

            let _end_span = self.consume(Token::RightBrace, "Expected '}' after else body")?;
            Some(else_stmts)
        } else {
            None
        };

        let end_span = self.tokens[self.current - 1].1;

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a while statement
    fn parse_while(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::While, "Expected 'while'")?;

        let condition = self.parse_expression()?;

        self.consume(Token::LeftBrace, "Expected '{' after while condition")?;

        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after while body")?;

        Ok(Stmt::While {
            condition,
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a for statement
    fn parse_for(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::For, "Expected 'for'")?;

        // Parse the loop variable
        let var = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "variable name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        self.consume(Token::In, "Expected 'in' after for variable")?;

        // Parse the iterator expression (array or range)
        let iter = self.parse_expression()?;

        self.consume(Token::LeftBrace, "Expected '{' after for header")?;

        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after for body")?;

        Ok(Stmt::For {
            var,
            iter,
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a break statement
    fn parse_break(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Break, "Expected 'break'")?;
        let end_span = self.consume(Token::Semicolon, "Expected ';' after break")?;

        Ok(Stmt::Break {
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a continue statement
    fn parse_continue(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Continue, "Expected 'continue'")?;
        let end_span = self.consume(Token::Semicolon, "Expected ';' after continue")?;

        Ok(Stmt::Continue {
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a match statement
    fn parse_match(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Match, "Expected 'match'")?;

        let expr = self.parse_expression()?;

        self.consume(Token::LeftBrace, "Expected '{' after match expression")?;

        let mut arms = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // Parse pattern
            let pattern = self.parse_pattern()?;

            self.consume(Token::FatArrow, "Expected '=>' after pattern")?;

            // Parse arm body
            let body = if self.check(&Token::LeftBrace) {
                // Block body
                self.advance()?; // consume '{'
                let mut stmts = Vec::new();
                while !self.check(&Token::RightBrace) && !self.is_at_end() {
                    stmts.push(self.parse_statement()?);
                }
                self.consume(Token::RightBrace, "Expected '}' after match arm body")?;
                stmts
            } else {
                // Single expression body
                let expr = self.parse_expression()?;
                self.consume(Token::Comma, "Expected ',' after match arm expression")?;
                vec![Stmt::Expr(expr)]
            };

            arms.push(MatchArm { pattern, body });
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after match arms")?;

        Ok(Stmt::Match {
            expr,
            arms,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse an unsafe block
    fn parse_unsafe(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Unsafe, "Expected 'unsafe'")?;

        self.consume(Token::LeftBrace, "Expected '{' after unsafe")?;

        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after unsafe block")?;

        Ok(Stmt::Unsafe {
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a pattern
    fn parse_pattern(&mut self) -> Result<Pattern> {
        // First, peek and clone the token to avoid borrowing issues
        let token = self.peek()?.clone();

        match token {
            Token::Underscore => {
                self.advance()?;
                Ok(Pattern::Wildcard)
            }
            Token::Identifier(name) => {
                self.advance()?;

                // Check if this is an enum pattern
                if self.check(&Token::DoubleColon) {
                    self.advance()?; // consume '::'

                    let variant = match self.advance()? {
                        (Token::Identifier(v), _) => v,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "variant name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    // Check for pattern data
                    let data = if self.check(&Token::LeftParen) {
                        // Tuple pattern
                        self.advance()?; // consume '('
                        let mut patterns = Vec::new();

                        if !self.check(&Token::RightParen) {
                            loop {
                                patterns.push(self.parse_pattern()?);
                                if !self.check(&Token::Comma) {
                                    break;
                                }
                                self.advance()?; // consume ','
                            }
                        }

                        self.consume(Token::RightParen, "Expected ')' after tuple pattern")?;
                        Some(PatternData::Tuple(patterns))
                    } else if self.check(&Token::LeftBrace) {
                        // Struct pattern
                        self.advance()?; // consume '{'
                        let mut fields = Vec::new();

                        while !self.check(&Token::RightBrace) && !self.is_at_end() {
                            let field_name = match self.advance()? {
                                (Token::Identifier(fname), _) => fname,
                                (token, _) => {
                                    return Err(CompileError::UnexpectedToken {
                                        expected: "field name".to_string(),
                                        found: token.to_string(),
                                        span: self.current_span(),
                                    });
                                }
                            };

                            self.consume(Token::Colon, "Expected ':' after field name in pattern")?;
                            let field_pattern = self.parse_pattern()?;

                            fields.push((field_name, field_pattern));

                            if !self.check(&Token::RightBrace) {
                                self.consume(Token::Comma, "Expected ',' after field pattern")?;
                            }
                        }

                        self.consume(Token::RightBrace, "Expected '}' after struct pattern")?;
                        Some(PatternData::Struct(fields))
                    } else {
                        None
                    };

                    Ok(Pattern::EnumPattern {
                        enum_name: name,
                        variant,
                        data,
                    })
                } else {
                    // Simple identifier pattern
                    Ok(Pattern::Ident(name))
                }
            }
            _ => Err(CompileError::UnexpectedToken {
                expected: "pattern".to_string(),
                found: token.to_string(),
                span: self.current_span(),
            }),
        }
    }

    /// Parse an expression
    pub fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_range()
    }

    /// Parse range operators (..)
    fn parse_range(&mut self) -> Result<Expr> {
        let mut left = self.parse_logical_or()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::DotDot => {
                    let left_span = Self::expr_span(&left);
                    self.advance()?; // consume '..'
                    let right = self.parse_logical_or()?;
                    let right_span = Self::expr_span(&right);
                    left = Expr::Range {
                        start: Box::new(left),
                        end: Box::new(right),
                        span: Span::new(
                            left_span.start,
                            right_span.end,
                            left_span.line,
                            left_span.column,
                        ),
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse logical OR (||)
    fn parse_logical_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_logical_and()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::OrOr => {
                    let left_span = Self::expr_span(&left);
                    let _ = self.advance()?; // consume '||'
                    let right = self.parse_logical_and()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op: BinOp::Or,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse logical AND (&&)
    fn parse_logical_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_equality()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::AndAnd => {
                    let left_span = Self::expr_span(&left);
                    let _ = self.advance()?; // consume '&&'
                    let right = self.parse_equality()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op: BinOp::And,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse equality operators (==, !=)
    fn parse_equality(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::EqEq | Token::Ne => {
                    let left_span = Self::expr_span(&left);
                    let op = match self.advance()?.0 {
                        Token::EqEq => BinOp::Eq,
                        Token::Ne => BinOp::Ne,
                        _ => unreachable!(),
                    };
                    let right = self.parse_comparison()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse comparison operators (<, >, <=, >=)
    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_addition()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::Lt | Token::Gt | Token::Le | Token::Ge => {
                    let left_span = Self::expr_span(&left);
                    let op = match self.advance()?.0 {
                        Token::Lt => BinOp::Lt,
                        Token::Gt => BinOp::Gt,
                        Token::Le => BinOp::Le,
                        Token::Ge => BinOp::Ge,
                        _ => unreachable!(),
                    };
                    let right = self.parse_addition()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse addition and subtraction
    fn parse_addition(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplication()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::Plus | Token::Minus => {
                    let left_span = Self::expr_span(&left);
                    let op = match self.advance()?.0 {
                        Token::Plus => BinOp::Add,
                        Token::Minus => BinOp::Sub,
                        _ => unreachable!(),
                    };
                    let right = self.parse_multiplication()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse multiplication and division
    fn parse_multiplication(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::Star | Token::Slash | Token::Percent => {
                    let left_span = Self::expr_span(&left);
                    let op = match self.advance()?.0 {
                        Token::Star => BinOp::Mul,
                        Token::Slash => BinOp::Div,
                        Token::Percent => BinOp::Mod,
                        _ => unreachable!(),
                    };
                    let right = self.parse_postfix()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse a type
    fn parse_type(&mut self) -> Result<Type> {
        match self.advance()? {
            (Token::SelfType, _) => {
                // Self type in trait or impl contexts
                Ok(Type::Custom("Self".to_string()))
            }
            (Token::Ampersand, _) => {
                // Parse reference type: &T or &mut T or &'a T or &'a mut T
                let mut lifetime = None;
                let mut mutable = false;

                // Check for lifetime annotation
                if matches!(self.peek()?, Token::SingleQuote) {
                    self.advance()?; // consume '
                    match self.advance()? {
                        (Token::Identifier(lt), _) => {
                            lifetime = Some(lt);
                        }
                        _ => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: self.peek()?.to_string(),
                                span: self.current_span(),
                            });
                        }
                    }
                }

                // Check for mut keyword
                if matches!(self.peek()?, Token::Mut) {
                    self.advance()?;
                    mutable = true;
                }

                // Parse the inner type
                let inner = self.parse_type()?;

                Ok(Type::Reference {
                    lifetime,
                    mutable,
                    inner: Box::new(inner),
                })
            }
            (Token::Identifier(name), _) => {
                // First check if it's a type parameter in scope
                if self.type_params_in_scope.contains(&name) {
                    return Ok(Type::TypeParam(name));
                }

                let base_type = match name.as_str() {
                    "i32" => Type::I32,
                    "i64" | "int" => Type::I64, // "int" is an alias for i64
                    "u32" => Type::U32,
                    "u64" => Type::U64,
                    "bool" => Type::Bool,
                    "String" => Type::String,
                    _ => Type::Custom(name.clone()),
                };

                // Check for generic arguments
                if self.check(&Token::Lt) {
                    // Only parse generics for custom types
                    match base_type {
                        Type::Custom(type_name) => {
                            self.advance()?; // consume '<'
                            let mut args = Vec::new();

                            loop {
                                // Try to parse as const value first (for literals)
                                if let Token::Integer(n) = self.peek()? {
                                    let n_val = *n;
                                    self.advance()?; // consume the integer
                                    args.push(GenericArg::Const(ConstValue::Integer(n_val)));
                                } else {
                                    // Otherwise parse as type
                                    let ty = self.parse_type()?;
                                    // If it's an identifier, it could be a const param
                                    match &ty {
                                        Type::Custom(name)
                                            if name
                                                .chars()
                                                .all(|c| c.is_uppercase() || c == '_') =>
                                        {
                                            // Assume uppercase identifiers are const params
                                            args.push(GenericArg::Const(ConstValue::ConstParam(
                                                name.clone(),
                                            )));
                                        }
                                        _ => {
                                            args.push(GenericArg::Type(ty));
                                        }
                                    }
                                }

                                if !self.check(&Token::Comma) {
                                    break;
                                }
                                self.advance()?; // consume ','
                            }

                            self.consume(Token::Gt, "Expected '>' after generic arguments")?;
                            Ok(Type::Generic {
                                name: type_name,
                                args,
                            })
                        }
                        _ => {
                            // Primitive types cannot have generic arguments
                            Err(CompileError::SyntaxError {
                                message: format!("Type '{}' cannot have generic arguments", name),
                                span: self.current_span(),
                            })
                        }
                    }
                } else {
                    Ok(base_type)
                }
            }
            (Token::LeftParen, _) => {
                self.consume(Token::RightParen, "Expected ')' for unit type")?;
                Ok(Type::Unit)
            }
            (Token::LeftBracket, _) => {
                // Parse array type: [T; N]
                let elem_type = self.parse_type()?;
                self.consume(Token::Semicolon, "Expected ';' in array type")?;

                // Parse the size (can be a literal or const parameter)
                let size = match self.peek()? {
                    Token::Integer(n) => {
                        let n_val = *n;
                        self.advance()?; // consume the integer
                        if n_val < 0 {
                            return Err(CompileError::Generic(
                                "Array size must be non-negative".to_string(),
                            ));
                        }
                        ArraySize::Literal(n_val as usize)
                    }
                    Token::Identifier(name) => {
                        let name_val = name.clone();
                        self.advance()?; // consume the identifier
                                         // Check if it's a const parameter in scope
                                         // For now, we'll assume any identifier could be a const param
                        ArraySize::ConstParam(name_val)
                    }
                    token => {
                        return Err(CompileError::UnexpectedToken {
                            expected: "array size (integer or const parameter)".to_string(),
                            found: token.to_string(),
                            span: self.current_span(),
                        });
                    }
                };

                self.consume(Token::RightBracket, "Expected ']' after array type")?;
                Ok(Type::Array(Box::new(elem_type), size))
            }
            (token, _) => Err(CompileError::UnexpectedToken {
                expected: "type".to_string(),
                found: token.to_string(),
                span: self.current_span(),
            }),
        }
    }

    /// Parse a primary expression
    fn parse_primary(&mut self) -> Result<Expr> {
        match self.advance()? {
            (Token::String(s), _) => Ok(Expr::String(s)),
            (Token::Integer(n), _) => Ok(Expr::Integer(n)),
            (Token::True, _) => Ok(Expr::Bool(true)),
            (Token::False, _) => Ok(Expr::Bool(false)),
            (Token::Identifier(name), span) => {
                // Check if this is a struct literal
                // We need to be careful here - only parse as struct literal if we see
                // identifier followed by field pattern (identifier + colon)
                if self.check(&Token::LeftBrace) && self.check_struct_literal_pattern() {
                    let start_span = span;
                    self.advance()?; // consume '{'

                    let mut fields = Vec::new();

                    while !self.check(&Token::RightBrace) && !self.is_at_end() {
                        // Parse field name
                        let field_name = match self.advance()? {
                            (Token::Identifier(fname), _) => fname,
                            (token, _) => {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "field name".to_string(),
                                    found: token.to_string(),
                                    span: self.current_span(),
                                });
                            }
                        };

                        self.consume(Token::Colon, "Expected ':' after field name")?;
                        let field_expr = self.parse_expression()?;

                        fields.push((field_name, field_expr));

                        if !self.check(&Token::RightBrace) {
                            self.consume(Token::Comma, "Expected ',' after field")?;
                        }
                    }

                    let end_span =
                        self.consume(Token::RightBrace, "Expected '}' after struct fields")?;

                    Ok(Expr::StructLiteral {
                        name,
                        fields,
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    })
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            (Token::LeftParen, _) => {
                // Parse parenthesized expression
                let expr = self.parse_expression()?;
                self.consume(Token::RightParen, "Expected ')' after expression")?;
                Ok(expr)
            }
            (Token::LeftBracket, span) => {
                // Parse array literal: [1, 2, 3] or array repeat: [0; 10]
                if self.check(&Token::RightBracket) {
                    // Empty array
                    let end_span = self.advance()?.1;
                    return Ok(Expr::ArrayLiteral {
                        elements: Vec::new(),
                        span: Span::new(span.start, end_span.end, span.line, span.column),
                    });
                }

                // Parse first element
                let first_elem = self.parse_expression()?;

                // Check if this is array repeat syntax
                if self.check(&Token::Semicolon) {
                    self.advance()?; // consume ';'
                    let count = self.parse_expression()?;
                    let end_span =
                        self.consume(Token::RightBracket, "Expected ']' after array repeat count")?;

                    Ok(Expr::ArrayRepeat {
                        value: Box::new(first_elem),
                        count: Box::new(count),
                        span: Span::new(span.start, end_span.end, span.line, span.column),
                    })
                } else {
                    // Regular array literal
                    let mut elements = vec![first_elem];

                    while self.check(&Token::Comma) {
                        self.advance()?; // consume ','
                        if self.check(&Token::RightBracket) {
                            // Trailing comma
                            break;
                        }
                        elements.push(self.parse_expression()?);
                    }

                    let end_span =
                        self.consume(Token::RightBracket, "Expected ']' after array elements")?;

                    Ok(Expr::ArrayLiteral {
                        elements,
                        span: Span::new(span.start, end_span.end, span.line, span.column),
                    })
                }
            }
            (token, _) => Err(CompileError::UnexpectedToken {
                expected: "expression".to_string(),
                found: token.to_string(),
                span: self.current_span(),
            }),
        }
    }

    /// Parse unary expressions (-, !, &, &mut, *)
    fn parse_unary(&mut self) -> Result<Expr> {
        match self.peek() {
            Ok(Token::Minus) => {
                let (_, start_span) = self.advance()?; // consume '-'
                let operand = self.parse_unary()?; // Right associative
                let end_span = operand.span();
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            Ok(Token::Not) => {
                let (_, start_span) = self.advance()?; // consume '!'
                let operand = self.parse_unary()?; // Right associative
                let end_span = operand.span();
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            Ok(Token::Ampersand) => {
                let (_, start_span) = self.advance()?; // consume '&'
                let mutable = if matches!(self.peek()?, Token::Mut) {
                    self.advance()?; // consume 'mut'
                    true
                } else {
                    false
                };
                let expr = self.parse_unary()?;
                let end_span = expr.span();
                Ok(Expr::Reference {
                    mutable,
                    expr: Box::new(expr),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            Ok(Token::Star) => {
                let (_, start_span) = self.advance()?; // consume '*'
                let expr = self.parse_unary()?;
                let end_span = expr.span();
                Ok(Expr::Deref {
                    expr: Box::new(expr),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            _ => self.parse_postfix(),
        }
    }

    /// Parse postfix expressions (array indexing, function calls)
    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek() {
                Ok(Token::LeftBracket) => {
                    let start_span = self.advance()?.1; // consume '['
                    let index = self.parse_expression()?;
                    let end_span =
                        self.consume(Token::RightBracket, "Expected ']' after array index")?;

                    expr = Expr::Index {
                        array: Box::new(expr),
                        index: Box::new(index),
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    };
                }
                Ok(Token::LeftParen) => {
                    let start_span = self.advance()?.1; // consume '('

                    let mut args = Vec::new();

                    if !self.check(&Token::RightParen) {
                        loop {
                            args.push(self.parse_expression()?);

                            if !self.check(&Token::Comma) {
                                break;
                            }
                            self.advance()?; // consume ','
                        }
                    }

                    let end_span = self.consume(Token::RightParen, "Expected ')'")?;

                    expr = Expr::Call {
                        func: Box::new(expr),
                        args,
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    };
                }
                Ok(Token::Dot) if self.check_at(1, &Token::Await) => {
                    let start_span = Self::expr_span(&expr);
                    self.advance()?; // consume '.'
                    let end_span = self.consume(Token::Await, "Expected 'await'")?;

                    expr = Expr::Await {
                        expr: Box::new(expr),
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    };
                }
                Ok(Token::Dot) => {
                    let start_span = Self::expr_span(&expr);

                    self.advance()?; // consume '.'

                    match self.advance()? {
                        (Token::Identifier(name), span) => {
                            let end_span = span;
                            expr = Expr::FieldAccess {
                                object: Box::new(expr),
                                field: name,
                                span: Span::new(
                                    start_span.start,
                                    end_span.end,
                                    start_span.line,
                                    start_span.column,
                                ),
                            };
                            continue;
                        }
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "field name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                }
                Ok(Token::DoubleColon) => {
                    // Handle enum constructor: EnumName::Variant
                    if let Expr::Ident(enum_name) = expr {
                        let start_span = self.tokens[self.current - 2].1; // Get span from before :: token
                        self.advance()?; // consume '::'

                        let variant = match self.advance()? {
                            (Token::Identifier(name), _) => name,
                            (token, _) => {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "variant name".to_string(),
                                    found: token.to_string(),
                                    span: self.current_span(),
                                });
                            }
                        };

                        // Check if this is a function call (has parentheses) or constructor
                        if self.check(&Token::LeftParen) {
                            // Parse tuple-style enum constructor
                            self.advance()?; // consume '('
                            let mut args = Vec::new();

                            if !self.check(&Token::RightParen) {
                                loop {
                                    args.push(self.parse_expression()?);
                                    if !self.check(&Token::Comma) {
                                        break;
                                    }
                                    self.advance()?; // consume ','
                                }
                            }

                            let end_span = self.consume(Token::RightParen, "Expected ')'")?;

                            // Create an enum constructor expression
                            expr = Expr::EnumConstructor {
                                enum_name,
                                variant,
                                data: Some(EnumConstructorData::Tuple(args)),
                                span: Span::new(
                                    start_span.start,
                                    end_span.end,
                                    start_span.line,
                                    start_span.column,
                                ),
                            };
                            continue;
                        }

                        // Check for struct-style constructor
                        let data = if self.check(&Token::LeftBrace) {
                            // Struct constructor
                            self.advance()?; // consume '{'
                            let mut fields = Vec::new();

                            while !self.check(&Token::RightBrace) && !self.is_at_end() {
                                let field_name = match self.advance()? {
                                    (Token::Identifier(fname), _) => fname,
                                    (token, _) => {
                                        return Err(CompileError::UnexpectedToken {
                                            expected: "field name".to_string(),
                                            found: token.to_string(),
                                            span: self.current_span(),
                                        });
                                    }
                                };

                                self.consume(Token::Colon, "Expected ':' after field name")?;
                                let field_expr = self.parse_expression()?;

                                fields.push((field_name, field_expr));

                                if !self.check(&Token::RightBrace) {
                                    self.consume(Token::Comma, "Expected ',' after field")?;
                                }
                            }

                            let _end_span = self.consume(Token::RightBrace, "Expected '}'")?;
                            Some(EnumConstructorData::Struct(fields))
                        } else {
                            None
                        };

                        let end_span = self.tokens[self.current - 1].1; // Get last consumed token span
                        expr = Expr::EnumConstructor {
                            enum_name,
                            variant,
                            data,
                            span: Span::new(
                                start_span.start,
                                end_span.end,
                                start_span.line,
                                start_span.column,
                            ),
                        };
                    } else {
                        return Err(CompileError::SyntaxError {
                            message: "Double colon can only be used after an identifier"
                                .to_string(),
                            span: self.current_span(),
                        });
                    }
                }
                Ok(Token::Question) => {
                    let start_span = Self::expr_span(&expr);
                    let (_, end_span) = self.advance()?; // consume '?'

                    expr = Expr::Question {
                        expr: Box::new(expr),
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    };
                }
                Ok(Token::Not) => {
                    // Macro invocation: name!(args)
                    if let Expr::Ident(name) = expr {
                        let start_span = self.tokens[self.current - 1].1; // Get span from identifier
                        self.advance()?; // consume '!'

                        // Parse macro arguments (simplified for now - just collect tokens in parens)
                        self.consume(Token::LeftParen, "Expected '(' after macro name!")?;

                        let mut args = Vec::new();
                        let mut paren_depth = 1;

                        while paren_depth > 0 && !self.is_at_end() {
                            let (token, _) = self.advance()?;

                            match &token {
                                Token::LeftParen => {
                                    paren_depth += 1;
                                    args.push(self.token_to_ast_token(token));
                                }
                                Token::RightParen => {
                                    paren_depth -= 1;
                                    if paren_depth > 0 {
                                        args.push(self.token_to_ast_token(token));
                                    }
                                }
                                _ => {
                                    args.push(self.token_to_ast_token(token));
                                }
                            }
                        }

                        let end_span = self.current_span().unwrap_or(start_span);

                        expr = Expr::MacroInvocation {
                            name,
                            args,
                            span: Span::new(
                                start_span.start,
                                end_span.end,
                                start_span.line,
                                start_span.column,
                            ),
                        };
                    } else {
                        return Err(CompileError::SyntaxError {
                            message: "Macro invocation '!' can only be used after an identifier"
                                .to_string(),
                            span: self.current_span(),
                        });
                    }
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    // Helper methods

    /// Check if the pattern ahead looks like a struct literal
    /// We look for: { identifier : ... or { }
    fn check_struct_literal_pattern(&self) -> bool {
        if self.current + 1 >= self.tokens.len() {
            return false;
        }

        // Check if next token after { is an identifier or }
        match &self.tokens[self.current + 1].0 {
            Token::Identifier(_) => {
                // Check if token after identifier is :
                if self.current + 2 < self.tokens.len() {
                    matches!(&self.tokens[self.current + 2].0, Token::Colon)
                } else {
                    false
                }
            }
            Token::RightBrace => true, // Empty struct literal
            _ => false,
        }
    }

    /// Check if we're at the end of tokens
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
    }

    /// Peek at the current token without consuming it
    fn peek(&self) -> Result<&Token> {
        if self.is_at_end() {
            Err(CompileError::SyntaxError {
                message: "Unexpected end of file".to_string(),
                span: self.current_span(),
            })
        } else {
            Ok(&self.tokens[self.current].0)
        }
    }

    /// Check if the current token matches the given token
    fn check(&self, token: &Token) -> bool {
        if self.is_at_end() {
            false
        } else {
            std::mem::discriminant(&self.tokens[self.current].0) == std::mem::discriminant(token)
        }
    }

    /// Check if a token at offset matches the given token
    fn check_at(&self, offset: usize, token: &Token) -> bool {
        let index = self.current + offset;
        if index >= self.tokens.len() {
            false
        } else {
            std::mem::discriminant(&self.tokens[index].0) == std::mem::discriminant(token)
        }
    }

    /// Advance to the next token
    fn advance(&mut self) -> Result<(Token, Span)> {
        if self.is_at_end() {
            Err(CompileError::SyntaxError {
                message: "Unexpected end of file".to_string(),
                span: self.current_span(),
            })
        } else {
            let token = self.tokens[self.current].clone();
            self.current += 1;
            self.update_cache(); // Update cache after advancing
            Ok(token)
        }
    }

    /// Consume a specific token or error
    fn consume(&mut self, expected: Token, message: &str) -> Result<Span> {
        let (token, span) = self.advance()?;

        if std::mem::discriminant(&token) == std::mem::discriminant(&expected) {
            Ok(span)
        } else {
            Err(CompileError::UnexpectedToken {
                expected: format!("{} ({})", expected, message),
                found: token.to_string(),
                span: self.current_span(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.params.len(), 0);
            assert_eq!(func.return_type, None);
            assert_eq!(func.body.len(), 1);

            if let Stmt::Expr(Expr::Call { func: _, args, .. }) = &func.body[0] {
                assert_eq!(args.len(), 1);
                if let Expr::String(s) = &args[0] {
                    assert_eq!(s, "Hello, World!");
                }
            }
        }
    }

    #[test]
    fn test_parse_function_with_return_type() {
        let source = r#"
        fn main() -> i32 {
            return 0;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.params.len(), 0);
            assert_eq!(func.return_type, Some(Type::I32));
            assert_eq!(func.body.len(), 1);

            if let Stmt::Return(Some(Expr::Integer(n))) = &func.body[0] {
                assert_eq!(*n, 0);
            } else {
                panic!("Expected return statement with integer");
            }
        }
    }

    #[test]
    fn test_parse_for_loop() {
        let source = r#"
        fn main() {
            for i in arr {
                print_int(i);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 1);

            if let Stmt::For {
                var, iter, body, ..
            } = &func.body[0]
            {
                assert_eq!(var, "i");
                if let Expr::Ident(name) = iter {
                    assert_eq!(name, "arr");
                }
                assert_eq!(body.len(), 1);
            } else {
                panic!("Expected for loop");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_break_continue() {
        let source = r#"
        fn main() {
            while true {
                if x > 10 {
                    break;
                }
                if x == 5 {
                    continue;
                }
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.body.len(), 1);

            if let Stmt::While { body, .. } = &func.body[0] {
                assert_eq!(body.len(), 2);

                if let Stmt::If { then_branch, .. } = &body[0] {
                    assert_eq!(then_branch.len(), 1);
                    assert!(matches!(&then_branch[0], Stmt::Break { .. }));
                }

                if let Stmt::If { then_branch, .. } = &body[1] {
                    assert_eq!(then_branch.len(), 1);
                    assert!(matches!(&then_branch[0], Stmt::Continue { .. }));
                }
            } else {
                panic!("Expected while loop");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_for_loop_with_break_continue() {
        let source = r#"
        fn main() {
            for i in arr {
                if i == 0 {
                    continue;
                }
                if i > 10 {
                    break;
                }
                print_int(i);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.body.len(), 1);

            if let Stmt::For { var, body, .. } = &func.body[0] {
                assert_eq!(var, "i");
                assert_eq!(body.len(), 3);

                // First statement: if with continue
                if let Stmt::If { then_branch, .. } = &body[0] {
                    assert_eq!(then_branch.len(), 1);
                    assert!(matches!(&then_branch[0], Stmt::Continue { .. }));
                }

                // Second statement: if with break
                if let Stmt::If { then_branch, .. } = &body[1] {
                    assert_eq!(then_branch.len(), 1);
                    assert!(matches!(&then_branch[0], Stmt::Break { .. }));
                }

                // Third statement: print_int call
                assert!(matches!(&body[2], Stmt::Expr(_)));
            } else {
                panic!("Expected for loop");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_struct() {
        let source = r#"
        struct Point {
            x: i64,
            y: i64,
        }
        
        fn main() {
            let p = Point { x: 10, y: 20 };
            print_int(p.x);
            p.y = 30;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 2);

        // Check struct definition
        if let Item::Struct(struct_def) = &ast.items[0] {
            assert_eq!(struct_def.name, "Point");
            assert_eq!(struct_def.fields.len(), 2);
            assert_eq!(struct_def.fields[0].0, "x");
            assert_eq!(struct_def.fields[0].1, Type::I64);
            assert_eq!(struct_def.fields[1].0, "y");
            assert_eq!(struct_def.fields[1].1, Type::I64);
        } else {
            panic!("Expected struct definition");
        }

        // Check function with struct usage
        if let Item::Function(func) = &ast.items[1] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 3);

            // First statement: struct literal
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "p");
                if let Expr::StructLiteral { name, fields, .. } = value {
                    assert_eq!(name, "Point");
                    assert_eq!(fields.len(), 2);
                    assert_eq!(fields[0].0, "x");
                    assert_eq!(fields[1].0, "y");
                } else {
                    panic!("Expected struct literal");
                }
            }

            // Second statement: field access
            if let Stmt::Expr(Expr::Call { args, .. }) = &func.body[1] {
                assert_eq!(args.len(), 1);
                if let Expr::FieldAccess { field, .. } = &args[0] {
                    assert_eq!(field, "x");
                } else {
                    panic!("Expected field access");
                }
            }

            // Third statement: field assignment
            if let Stmt::Assign { target, .. } = &func.body[2] {
                if let AssignTarget::FieldAccess { field, .. } = target {
                    assert_eq!(field, "y");
                } else {
                    panic!("Expected field assignment");
                }
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_range_syntax() {
        let source = r#"
        fn main() {
            for i in 0..10 {
                print_int(i);
            }
            
            let start = 5;
            let end = 15;
            for j in start..end {
                print_int(j);
            }
            
            for k in 0..n+1 {
                print_int(k);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 5); // 3 for loops + 2 let statements

            // First for loop: 0..10
            if let Stmt::For { var, iter, .. } = &func.body[0] {
                assert_eq!(var, "i");
                if let Expr::Range { start, end, .. } = iter {
                    assert!(matches!(start.as_ref(), Expr::Integer(0)));
                    assert!(matches!(end.as_ref(), Expr::Integer(10)));
                } else {
                    panic!("Expected range expression");
                }
            } else {
                panic!("Expected for loop");
            }

            // Check let statements
            assert!(matches!(&func.body[1], Stmt::Let { name, .. } if name == "start"));
            assert!(matches!(&func.body[2], Stmt::Let { name, .. } if name == "end"));

            // Second for loop: start..end (with variables)
            if let Stmt::For { var, iter, .. } = &func.body[3] {
                assert_eq!(var, "j");
                if let Expr::Range { start, end, .. } = iter {
                    assert!(matches!(start.as_ref(), Expr::Ident(s) if s == "start"));
                    assert!(matches!(end.as_ref(), Expr::Ident(e) if e == "end"));
                } else {
                    panic!("Expected range expression");
                }
            } else {
                panic!("Expected for loop");
            }

            // Third for loop: 0..n+1
            if let Stmt::For { var, iter, .. } = &func.body[4] {
                assert_eq!(var, "k");
                if let Expr::Range { start, end, .. } = iter {
                    assert!(matches!(start.as_ref(), Expr::Integer(0)));
                    // The end should be a binary expression (n+1)
                    assert!(matches!(end.as_ref(), Expr::Binary { .. }));
                } else {
                    panic!("Expected range expression");
                }
            } else {
                panic!("Expected for loop");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_enum() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        enum Option {
            Some(i64),
            None,
        }
        
        enum Shape {
            Circle { radius: i64 },
            Rectangle { width: i64, height: i64 },
            Point,
        }
        
        fn main() {
            let c = Color::Red;
            let opt = Option::Some(42);
            let shape = Shape::Circle { radius: 10 };
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 4);

        // Check first enum (simple)
        if let Item::Enum(enum_def) = &ast.items[0] {
            assert_eq!(enum_def.name, "Color");
            assert_eq!(enum_def.variants.len(), 3);
            assert_eq!(enum_def.variants[0].name, "Red");
            assert!(matches!(enum_def.variants[0].data, EnumVariantData::Unit));
            assert_eq!(enum_def.variants[1].name, "Green");
            assert!(matches!(enum_def.variants[1].data, EnumVariantData::Unit));
            assert_eq!(enum_def.variants[2].name, "Blue");
            assert!(matches!(enum_def.variants[2].data, EnumVariantData::Unit));
        } else {
            panic!("Expected enum definition");
        }

        // Check second enum (with tuple variant)
        if let Item::Enum(enum_def) = &ast.items[1] {
            assert_eq!(enum_def.name, "Option");
            assert_eq!(enum_def.variants.len(), 2);
            assert_eq!(enum_def.variants[0].name, "Some");
            if let EnumVariantData::Tuple(types) = &enum_def.variants[0].data {
                assert_eq!(types.len(), 1);
                assert_eq!(types[0], Type::I64);
            } else {
                panic!("Expected tuple variant");
            }
            assert_eq!(enum_def.variants[1].name, "None");
            assert!(matches!(enum_def.variants[1].data, EnumVariantData::Unit));
        } else {
            panic!("Expected enum definition");
        }

        // Check third enum (with struct variant)
        if let Item::Enum(enum_def) = &ast.items[2] {
            assert_eq!(enum_def.name, "Shape");
            assert_eq!(enum_def.variants.len(), 3);

            assert_eq!(enum_def.variants[0].name, "Circle");
            if let EnumVariantData::Struct(fields) = &enum_def.variants[0].data {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].0, "radius");
                assert_eq!(fields[0].1, Type::I64);
            } else {
                panic!("Expected struct variant");
            }

            assert_eq!(enum_def.variants[1].name, "Rectangle");
            if let EnumVariantData::Struct(fields) = &enum_def.variants[1].data {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "width");
                assert_eq!(fields[0].1, Type::I64);
                assert_eq!(fields[1].0, "height");
                assert_eq!(fields[1].1, Type::I64);
            } else {
                panic!("Expected struct variant");
            }

            assert_eq!(enum_def.variants[2].name, "Point");
            assert!(matches!(enum_def.variants[2].data, EnumVariantData::Unit));
        } else {
            panic!("Expected enum definition");
        }

        // Check function with enum usage
        if let Item::Function(func) = &ast.items[3] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 3);

            // First statement: unit enum constructor
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "c");
                if let Expr::EnumConstructor {
                    enum_name,
                    variant,
                    data,
                    ..
                } = value
                {
                    assert_eq!(enum_name, "Color");
                    assert_eq!(variant, "Red");
                    assert!(data.is_none());
                } else {
                    panic!("Expected enum constructor");
                }
            }

            // Second statement: tuple enum constructor
            if let Stmt::Let { name, value, .. } = &func.body[1] {
                assert_eq!(name, "opt");
                if let Expr::EnumConstructor {
                    enum_name,
                    variant,
                    data,
                    ..
                } = value
                {
                    assert_eq!(enum_name, "Option");
                    assert_eq!(variant, "Some");
                    if let Some(EnumConstructorData::Tuple(args)) = data {
                        assert_eq!(args.len(), 1);
                        if let Expr::Integer(n) = &args[0] {
                            assert_eq!(*n, 42);
                        } else {
                            panic!("Expected integer argument");
                        }
                    } else {
                        panic!("Expected tuple constructor data");
                    }
                } else {
                    panic!("Expected enum constructor");
                }
            }

            // Third statement: struct enum constructor
            if let Stmt::Let { name, value, .. } = &func.body[2] {
                assert_eq!(name, "shape");
                if let Expr::EnumConstructor {
                    enum_name,
                    variant,
                    data,
                    ..
                } = value
                {
                    assert_eq!(enum_name, "Shape");
                    assert_eq!(variant, "Circle");
                    if let Some(EnumConstructorData::Struct(fields)) = data {
                        assert_eq!(fields.len(), 1);
                        assert_eq!(fields[0].0, "radius");
                        if let Expr::Integer(n) = &fields[0].1 {
                            assert_eq!(*n, 10);
                        } else {
                            panic!("Expected integer field value");
                        }
                    } else {
                        panic!("Expected struct constructor data");
                    }
                } else {
                    panic!("Expected enum constructor");
                }
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_match_wildcard() {
        let source = r#"
        fn main() {
            let x = 42;
            match x {
                _ => print("wildcard"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.body.len(), 2);
            if let Stmt::Match { arms, .. } = &func.body[1] {
                assert_eq!(arms.len(), 1);
                match &arms[0].pattern {
                    Pattern::Wildcard => {}
                    _ => panic!("Expected wildcard pattern"),
                }
                assert_eq!(arms[0].body.len(), 1);
            } else {
                panic!("Expected match statement");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_match_identifier() {
        let source = r#"
        fn main() {
            match x {
                value => print("bound"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        if let Item::Function(func) = &ast.items[0] {
            if let Stmt::Match { arms, .. } = &func.body[0] {
                match &arms[0].pattern {
                    Pattern::Ident(name) => {
                        assert_eq!(name, "value");
                    }
                    _ => panic!("Expected identifier pattern"),
                }
            }
        }
    }

    #[test]
    fn test_parse_match_enum_patterns() {
        let source = r#"
        enum Option {
            Some(i64),
            None,
        }
        
        fn main() {
            match opt {
                Option::Some(n) => print_int(n),
                Option::None => print("none"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        if let Item::Function(func) = &ast.items[1] {
            if let Stmt::Match { arms, .. } = &func.body[0] {
                assert_eq!(arms.len(), 2);

                // First arm: Option::Some(n)
                match &arms[0].pattern {
                    Pattern::EnumPattern {
                        enum_name,
                        variant,
                        data,
                    } => {
                        assert_eq!(enum_name, "Option");
                        assert_eq!(variant, "Some");
                        if let Some(PatternData::Tuple(patterns)) = data {
                            assert_eq!(patterns.len(), 1);
                            match &patterns[0] {
                                Pattern::Ident(name) => assert_eq!(name, "n"),
                                _ => panic!("Expected identifier pattern in tuple"),
                            }
                        } else {
                            panic!("Expected tuple pattern data");
                        }
                    }
                    _ => panic!("Expected enum pattern"),
                }

                // Second arm: Option::None
                match &arms[1].pattern {
                    Pattern::EnumPattern {
                        enum_name,
                        variant,
                        data,
                    } => {
                        assert_eq!(enum_name, "Option");
                        assert_eq!(variant, "None");
                        assert!(data.is_none());
                    }
                    _ => panic!("Expected enum pattern"),
                }
            }
        }
    }

    #[test]
    fn test_parse_match_block_body() {
        let source = r#"
        fn main() {
            match x {
                _ => {
                    print("line 1");
                    print("line 2");
                }
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        if let Item::Function(func) = &ast.items[0] {
            if let Stmt::Match { arms, .. } = &func.body[0] {
                assert_eq!(arms[0].body.len(), 2);
            }
        }
    }

    #[test]
    fn test_parse_array_repeat() {
        let source = r#"
        fn main() {
            let arr = [0; 10];
            let arr2 = [42; 5];
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 2);

            // First statement: [0; 10]
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "arr");
                if let Expr::ArrayRepeat { value, count, .. } = value {
                    if let Expr::Integer(n) = value.as_ref() {
                        assert_eq!(*n, 0);
                    } else {
                        panic!("Expected integer value");
                    }
                    if let Expr::Integer(n) = count.as_ref() {
                        assert_eq!(*n, 10);
                    } else {
                        panic!("Expected integer count");
                    }
                } else {
                    panic!("Expected array repeat expression");
                }
            }

            // Second statement: [42; 5]
            if let Stmt::Let { name, value, .. } = &func.body[1] {
                assert_eq!(name, "arr2");
                if let Expr::ArrayRepeat { value, count, .. } = value {
                    if let Expr::Integer(n) = value.as_ref() {
                        assert_eq!(*n, 42);
                    } else {
                        panic!("Expected integer value");
                    }
                    if let Expr::Integer(n) = count.as_ref() {
                        assert_eq!(*n, 5);
                    } else {
                        panic!("Expected integer count");
                    }
                } else {
                    panic!("Expected array repeat expression");
                }
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_struct_returns() {
        let source = r#"
        struct Point {
            x: i64,
            y: i64,
        }
        
        fn make_point(x: i64, y: i64) -> Point {
            return Point { x: x, y: y };
        }
        
        fn get_origin() -> Point {
            return Point { x: 0, y: 0 };
        }
        
        fn main() {
            let p = make_point(10, 20);
            let origin = get_origin();
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 4);

        // Check struct definition
        if let Item::Struct(struct_def) = &ast.items[0] {
            assert_eq!(struct_def.name, "Point");
            assert_eq!(struct_def.fields.len(), 2);
        } else {
            panic!("Expected struct definition");
        }

        // Check make_point function
        if let Item::Function(func) = &ast.items[1] {
            assert_eq!(func.name, "make_point");
            assert_eq!(func.params.len(), 2);
            assert_eq!(func.params[0].name, "x");
            assert_eq!(func.params[0].ty, Type::I64);
            assert_eq!(func.params[1].name, "y");
            assert_eq!(func.params[1].ty, Type::I64);
            assert_eq!(func.return_type, Some(Type::Custom("Point".to_string())));

            // Check return statement
            assert_eq!(func.body.len(), 1);
            if let Stmt::Return(Some(Expr::StructLiteral { name, fields, .. })) = &func.body[0] {
                assert_eq!(name, "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "x");
                assert_eq!(fields[1].0, "y");
            } else {
                panic!("Expected return with struct literal");
            }
        } else {
            panic!("Expected function");
        }

        // Check get_origin function
        if let Item::Function(func) = &ast.items[2] {
            assert_eq!(func.name, "get_origin");
            assert_eq!(func.params.len(), 0);
            assert_eq!(func.return_type, Some(Type::Custom("Point".to_string())));

            // Check return statement
            assert_eq!(func.body.len(), 1);
            if let Stmt::Return(Some(Expr::StructLiteral { name, fields, .. })) = &func.body[0] {
                assert_eq!(name, "Point");
                assert_eq!(fields.len(), 2);
                if let Expr::Integer(n) = &fields[0].1 {
                    assert_eq!(*n, 0);
                }
                if let Expr::Integer(n) = &fields[1].1 {
                    assert_eq!(*n, 0);
                }
            } else {
                panic!("Expected return with struct literal");
            }
        } else {
            panic!("Expected function");
        }

        // Check main function
        if let Item::Function(func) = &ast.items[3] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 2);

            // First statement: let p = make_point(10, 20)
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "p");
                if let Expr::Call { func, args, .. } = value {
                    if let Expr::Ident(fname) = func.as_ref() {
                        assert_eq!(fname, "make_point");
                    }
                    assert_eq!(args.len(), 2);
                } else {
                    panic!("Expected function call");
                }
            }

            // Second statement: let origin = get_origin()
            if let Stmt::Let { name, value, .. } = &func.body[1] {
                assert_eq!(name, "origin");
                if let Expr::Call { func, args, .. } = value {
                    if let Expr::Ident(fname) = func.as_ref() {
                        assert_eq!(fname, "get_origin");
                    }
                    assert_eq!(args.len(), 0);
                } else {
                    panic!("Expected function call");
                }
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_logical_operators() {
        let source = r#"
        fn main() {
            let a = true && false;
            let b = true || false;
            let c = x < 5 && y > 10;
            let d = (a && b) || (c && d);
            
            if a && b || c {
                print("complex condition");
            }
            
            while i < 10 && running {
                i = i + 1;
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 6);

            // Check first statement: let a = true && false
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "a");
                if let Expr::Binary {
                    op, left, right, ..
                } = value
                {
                    assert_eq!(*op, BinOp::And);
                    assert!(matches!(left.as_ref(), Expr::Bool(true)));
                    assert!(matches!(right.as_ref(), Expr::Bool(false)));
                } else {
                    panic!("Expected && expression");
                }
            }

            // Check second statement: let b = true || false
            if let Stmt::Let { name, value, .. } = &func.body[1] {
                assert_eq!(name, "b");
                if let Expr::Binary {
                    op, left, right, ..
                } = value
                {
                    assert_eq!(*op, BinOp::Or);
                    assert!(matches!(left.as_ref(), Expr::Bool(true)));
                    assert!(matches!(right.as_ref(), Expr::Bool(false)));
                } else {
                    panic!("Expected || expression");
                }
            }

            // Check third statement: let c = x < 5 && y > 10
            if let Stmt::Let { name, value, .. } = &func.body[2] {
                assert_eq!(name, "c");
                if let Expr::Binary {
                    op, left, right, ..
                } = value
                {
                    assert_eq!(*op, BinOp::And);
                    // Left should be x < 5
                    if let Expr::Binary { op: left_op, .. } = left.as_ref() {
                        assert_eq!(*left_op, BinOp::Lt);
                    } else {
                        panic!("Expected comparison on left side of &&");
                    }
                    // Right should be y > 10
                    if let Expr::Binary { op: right_op, .. } = right.as_ref() {
                        assert_eq!(*right_op, BinOp::Gt);
                    } else {
                        panic!("Expected comparison on right side of &&");
                    }
                } else {
                    panic!("Expected && expression");
                }
            }

            // Check fourth statement: complex expression with parentheses
            if let Stmt::Let { name, value, .. } = &func.body[3] {
                assert_eq!(name, "d");
                if let Expr::Binary { op, .. } = value {
                    assert_eq!(*op, BinOp::Or);
                } else {
                    panic!("Expected || at top level");
                }
            }

            // Check if statement with logical operators
            if let Stmt::If { condition, .. } = &func.body[4] {
                if let Expr::Binary { op, .. } = condition {
                    assert_eq!(*op, BinOp::Or); // || has lower precedence than &&
                } else {
                    panic!("Expected logical expression in if condition");
                }
            }

            // Check while statement with logical operators
            if let Stmt::While { condition, .. } = &func.body[5] {
                if let Expr::Binary { op, .. } = condition {
                    assert_eq!(*op, BinOp::And);
                } else {
                    panic!("Expected && in while condition");
                }
            }
        } else {
            panic!("Expected function");
        }
    }
}
