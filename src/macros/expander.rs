// Macro expander implementation
// "Expanding code like expanding minds"

use super::parser::{CaptureKind, PatternElement, RepetitionKind};
use crate::ast::{Expr, Stmt};
use crate::errors::{CompileError, Result};
use crate::lexer::{Lexer, Token};
use crate::parser::Parser;
use std::collections::HashMap;

/// Captured values from macro matching
#[derive(Debug, Clone)]
pub enum CaptureValue {
    /// Single captured value
    Single(Vec<Token>),
    /// List of captured values (from repetition)
    List(Vec<Vec<Token>>),
}

/// Macro match context
pub struct MatchContext {
    /// Captured variables
    captures: HashMap<String, CaptureValue>,
}

impl MatchContext {
    pub fn new() -> Self {
        Self {
            captures: HashMap::new(),
        }
    }

    /// Add a capture
    pub fn add_capture(&mut self, name: String, value: Vec<Token>) {
        self.captures.insert(name, CaptureValue::Single(value));
    }

    /// Add a list capture
    pub fn add_list_capture(&mut self, name: String, values: Vec<Vec<Token>>) {
        self.captures.insert(name, CaptureValue::List(values));
    }

    /// Get a capture
    pub fn get_capture(&self, name: &str) -> Option<&CaptureValue> {
        self.captures.get(name)
    }
}

/// Match a token stream against a pattern
pub fn match_pattern(pattern: &[PatternElement], tokens: &[Token]) -> Result<Option<MatchContext>> {
    let mut context = MatchContext::new();
    let mut token_pos = 0;
    let mut pattern_pos = 0;

    while pattern_pos < pattern.len() && token_pos < tokens.len() {
        match &pattern[pattern_pos] {
            PatternElement::Literal(expected) => {
                // Match literal token
                if std::mem::discriminant(&tokens[token_pos]) != std::mem::discriminant(expected) {
                    return Ok(None); // No match
                }
                token_pos += 1;
                pattern_pos += 1;
            }

            PatternElement::Variable { name, kind } => {
                // Capture tokens based on kind
                let captured = capture_tokens(kind, &tokens[token_pos..])?;
                if captured.is_empty() {
                    return Ok(None); // No match
                }

                context.add_capture(name.clone(), captured.clone());
                token_pos += captured.len();
                pattern_pos += 1;
            }

            PatternElement::Repetition {
                pattern: rep_pattern,
                separator,
                kind,
            } => {
                // Match repetition
                let (captured_lists, consumed) =
                    match_repetition(rep_pattern, separator.as_ref(), kind, &tokens[token_pos..])?;

                // Add captures from repetition
                for (name, values) in captured_lists {
                    context.add_list_capture(name, values);
                }

                token_pos += consumed;
                pattern_pos += 1;
            }
        }
    }

    // Check if we consumed all tokens and pattern
    if pattern_pos == pattern.len() && token_pos == tokens.len() {
        Ok(Some(context))
    } else {
        Ok(None)
    }
}

/// Capture tokens based on capture kind
fn capture_tokens(kind: &CaptureKind, tokens: &[Token]) -> Result<Vec<Token>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    match kind {
        CaptureKind::Ident => {
            // Capture single identifier
            if let Token::Identifier(_) = &tokens[0] {
                Ok(vec![tokens[0].clone()])
            } else {
                Ok(Vec::new())
            }
        }

        CaptureKind::Lit => {
            // Capture literal (int, string, bool)
            match &tokens[0] {
                Token::Integer(_) | Token::String(_) | Token::True | Token::False => {
                    Ok(vec![tokens[0].clone()])
                }
                _ => Ok(Vec::new()),
            }
        }

        CaptureKind::Expr => {
            // Capture expression tokens
            // This is simplified - real implementation would parse balanced expressions
            capture_balanced_expr(tokens)
        }

        CaptureKind::Stmt => {
            // Capture statement tokens
            capture_until_semicolon(tokens)
        }

        CaptureKind::Type => {
            // Capture type tokens
            capture_type_tokens(tokens)
        }

        CaptureKind::Pat => {
            // Capture pattern tokens
            capture_pattern_tokens(tokens)
        }

        CaptureKind::Tt => {
            // Token tree - capture any single token or balanced group
            capture_token_tree(tokens)
        }
    }
}

/// Capture a balanced expression
fn capture_balanced_expr(tokens: &[Token]) -> Result<Vec<Token>> {
    let mut captured = Vec::new();
    let mut depth = 0;
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            Token::LeftParen | Token::LeftBrace | Token::LeftBracket => {
                depth += 1;
                captured.push(tokens[i].clone());
            }
            Token::RightParen | Token::RightBrace | Token::RightBracket => {
                captured.push(tokens[i].clone());
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Token::Semicolon | Token::Comma if depth == 0 => {
                break;
            }
            _ => {
                captured.push(tokens[i].clone());
            }
        }
        i += 1;
    }

    Ok(captured)
}

/// Capture tokens until semicolon
fn capture_until_semicolon(tokens: &[Token]) -> Result<Vec<Token>> {
    let mut captured = Vec::new();

    for token in tokens {
        if let Token::Semicolon = token {
            captured.push(token.clone());
            break;
        }
        captured.push(token.clone());
    }

    Ok(captured)
}

/// Capture type tokens
fn capture_type_tokens(tokens: &[Token]) -> Result<Vec<Token>> {
    // Simplified - just capture identifier or basic type
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    match &tokens[0] {
        Token::Identifier(_) => Ok(vec![tokens[0].clone()]),
        _ => Ok(Vec::new()),
    }
}

/// Capture pattern tokens
fn capture_pattern_tokens(tokens: &[Token]) -> Result<Vec<Token>> {
    // Simplified - just capture identifier patterns for now
    capture_type_tokens(tokens)
}

/// Capture a token tree
fn capture_token_tree(tokens: &[Token]) -> Result<Vec<Token>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    match &tokens[0] {
        Token::LeftParen | Token::LeftBrace | Token::LeftBracket => {
            // Capture balanced group
            capture_balanced_group(tokens)
        }
        _ => {
            // Single token
            Ok(vec![tokens[0].clone()])
        }
    }
}

/// Capture a balanced group (parens, braces, brackets)
fn capture_balanced_group(tokens: &[Token]) -> Result<Vec<Token>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let (open, close) = match &tokens[0] {
        Token::LeftParen => (Token::LeftParen, Token::RightParen),
        Token::LeftBrace => (Token::LeftBrace, Token::RightBrace),
        Token::LeftBracket => (Token::LeftBracket, Token::RightBracket),
        _ => return Ok(vec![tokens[0].clone()]),
    };

    let mut captured = vec![tokens[0].clone()];
    let mut depth = 1;
    let mut i = 1;

    while i < tokens.len() && depth > 0 {
        captured.push(tokens[i].clone());

        if std::mem::discriminant(&tokens[i]) == std::mem::discriminant(&open) {
            depth += 1;
        } else if std::mem::discriminant(&tokens[i]) == std::mem::discriminant(&close) {
            depth -= 1;
        }

        i += 1;
    }

    Ok(captured)
}

/// Match a repetition pattern
fn match_repetition(
    pattern: &[PatternElement],
    separator: Option<&Token>,
    kind: &RepetitionKind,
    tokens: &[Token],
) -> Result<(HashMap<String, Vec<Vec<Token>>>, usize)> {
    let mut captures: HashMap<String, Vec<Vec<Token>>> = HashMap::new();
    let mut consumed = 0;
    let mut match_count = 0;

    loop {
        // Try to match pattern
        let remaining = &tokens[consumed..];
        if let Some(ctx) = match_pattern(pattern, remaining)? {
            // Extract captures
            for (name, value) in ctx.captures {
                match value {
                    CaptureValue::Single(tokens) => {
                        captures.entry(name).or_insert_with(Vec::new).push(tokens);
                    }
                    CaptureValue::List(_) => {
                        // Nested repetitions not supported yet
                        return Err(CompileError::Generic(
                            "Nested repetitions not supported".to_string(),
                        ));
                    }
                }
            }

            // Count tokens consumed by this match
            let pattern_consumed = count_pattern_tokens(pattern, remaining)?;
            consumed += pattern_consumed;
            match_count += 1;

            // Check for separator
            if let Some(sep) = separator {
                if consumed < tokens.len()
                    && std::mem::discriminant(&tokens[consumed]) == std::mem::discriminant(sep)
                {
                    consumed += 1; // Consume separator
                } else {
                    break; // No separator, end of repetition
                }
            }
        } else {
            break; // No match
        }
    }

    // Validate match count
    match kind {
        RepetitionKind::ZeroOrMore => {} // Any count OK
        RepetitionKind::OneOrMore => {
            if match_count == 0 {
                return Err(CompileError::Generic(
                    "Expected at least one match".to_string(),
                ));
            }
        }
        RepetitionKind::ZeroOrOne => {
            if match_count > 1 {
                return Err(CompileError::Generic(
                    "Expected at most one match".to_string(),
                ));
            }
        }
    }

    Ok((captures, consumed))
}

/// Count tokens consumed by a pattern match
fn count_pattern_tokens(pattern: &[PatternElement], tokens: &[Token]) -> Result<usize> {
    // This is a simplified implementation
    // Real implementation would track exact token consumption
    let mut count = 0;

    for element in pattern {
        match element {
            PatternElement::Literal(_) => count += 1,
            PatternElement::Variable { kind, .. } => {
                let captured = capture_tokens(kind, &tokens[count..])?;
                count += captured.len();
            }
            PatternElement::Repetition { .. } => {
                // This is handled separately
                break;
            }
        }
    }

    Ok(count)
}

/// Substitute captured values into template
pub fn substitute_template(template: &[Token], context: &MatchContext) -> Result<Vec<Token>> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < template.len() {
        if let Token::Dollar = &template[i] {
            if i + 1 < template.len() {
                if let Token::Identifier(name) = &template[i + 1] {
                    // Substitute capture
                    if let Some(capture) = context.get_capture(name) {
                        match capture {
                            CaptureValue::Single(tokens) => {
                                result.extend(tokens.clone());
                            }
                            CaptureValue::List(token_lists) => {
                                // For now, just concatenate all
                                for tokens in token_lists {
                                    result.extend(tokens.clone());
                                }
                            }
                        }
                        i += 2; // Skip $ and name
                        continue;
                    }
                }
            }
        }

        result.push(template[i].clone());
        i += 1;
    }

    Ok(result)
}

/// Expand a macro invocation to an expression
pub fn expand_to_expr(tokens: Vec<Token>) -> Result<Expr> {
    // Convert tokens back to string and parse
    let source = tokens_to_string(&tokens);
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.collect_tokens()?;
    let mut parser = Parser::new(tokens);
    parser.parse_expression()
}

/// Expand a macro invocation to statements
pub fn expand_to_stmts(tokens: Vec<Token>) -> Result<Vec<Stmt>> {
    // Convert tokens back to string and parse
    let source = tokens_to_string(&tokens);
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.collect_tokens()?;
    let mut parser = Parser::new(tokens);

    let mut stmts = Vec::new();
    while parser.current_token() != &Token::Eof {
        stmts.push(parser.parse_statement()?);
    }

    Ok(stmts)
}

/// Convert tokens to string (for re-parsing)
fn tokens_to_string(tokens: &[Token]) -> String {
    let mut result = String::new();

    for (i, token) in tokens.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }

        match token {
            Token::Identifier(s) => result.push_str(s),
            Token::Integer(n) => result.push_str(&n.to_string()),
            Token::String(s) => {
                result.push('"');
                result.push_str(s);
                result.push('"');
            }
            Token::True => result.push_str("true"),
            Token::False => result.push_str("false"),
            Token::LeftParen => result.push('('),
            Token::RightParen => result.push(')'),
            Token::LeftBrace => result.push('{'),
            Token::RightBrace => result.push('}'),
            Token::LeftBracket => result.push('['),
            Token::RightBracket => result.push(']'),
            Token::Comma => result.push(','),
            Token::Semicolon => result.push(';'),
            Token::Colon => result.push(':'),
            Token::Arrow => result.push_str("->"),
            Token::Plus => result.push('+'),
            Token::Minus => result.push('-'),
            Token::Star => result.push('*'),
            Token::Slash => result.push('/'),
            Token::Percent => result.push('%'),
            Token::Eq => result.push('='),
            Token::EqEq => result.push_str("=="),
            Token::Ne => result.push_str("!="),
            Token::Lt => result.push('<'),
            Token::Gt => result.push('>'),
            Token::Le => result.push_str("<="),
            Token::Ge => result.push_str(">="),
            Token::AndAnd => result.push_str("&&"),
            Token::OrOr => result.push_str("||"),
            Token::Not => result.push('!'),
            Token::Dot => result.push('.'),
            Token::DotDot => result.push_str(".."),
            Token::Ampersand => result.push('&'),
            Token::Pipe => result.push('|'),
            Token::Question => result.push('?'),
            Token::Dollar => result.push('$'),
            _ => result.push_str(&format!("{:?}", token)),
        }
    }

    result
}
