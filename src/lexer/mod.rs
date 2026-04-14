pub mod span;
pub mod token;

pub use span::Span;
pub use token::Token;

use logos::Logos;
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Result type for lexing operations
pub type LexResult<T> = Result<T, LexerError>;

/// Lexer error with source location
#[derive(Error, Debug, Diagnostic)]
#[error("Lexical error: {message}")]
#[diagnostic(code(ved::lexer))]
pub struct LexerError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
    #[source_code]
    pub source_code: String,
}

/// A token with its source span and original text
#[derive(Debug, Clone)]
pub struct TokenWithContext {
    pub token: Token,
    pub span: Span,
    pub text: String,
    pub line: usize,
    pub column: usize,
}

/// Tokenize source code into a vector of tokens
/// Handles Python-style indentation tracking
pub fn tokenize(source: &str) -> LexResult<Vec<TokenWithContext>> {
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    let mut indent_stack: Vec<usize> = vec![0];
    let mut line_start = 0;
    let mut current_line = 1;

    // First pass: collect raw tokens (newlines included)
    let mut raw_tokens: Vec<(Token, logos::Span)> = Vec::new();

    loop {
        match lexer.next() {
            None => break,
            Some(Err(_)) => {
                let span = lexer.span();
                return Err(LexerError {
                    message: format!("Unexpected character: {}", &source[span.clone()]),
                    span: SourceSpan::from(span),
                    source_code: source.to_string(),
                });
            }
            Some(Ok(tok)) => {
                let span = lexer.span();
                raw_tokens.push((tok, span));
            }
        }
    }

    // Second pass: handle indentation
    for (tok, span) in raw_tokens {
        let span_start = span.start;
        let span_end = span.end;
        match &tok {
            Token::Newline => {
                // Check if next non-comment token is on a new line
                tokens.push(TokenWithContext {
                    token: tok,
                    span: Span::new(span_start, span_end),
                    text: source[span_start..span_end].to_string(),
                    line: current_line,
                    column: span_start - line_start,
                });
                current_line += 1;
                line_start = span_end;
            }
            Token::Comment => {
                // Skip comments but track position
            }
            _ => {
                // Calculate column position
                let column = span_start - line_start;

                // Check indentation after newlines
                if let Some(prev) = tokens.last() {
                    if matches!(prev.token, Token::Newline) {
                        let current_indent = *indent_stack.last().unwrap_or(&0);
                        // Handle dedent when column is 0 (back to top level)
                        if column == 0 && current_indent > 0 {
                            // Dedent back to top level
                            while indent_stack.len() > 1 {
                                indent_stack.pop();
                                tokens.push(TokenWithContext {
                                    token: Token::Dedent,
                                    span: Span::new(span_start, span_start),
                                    text: String::new(),
                                    line: current_line,
                                    column: 0,
                                });
                            }
                        } else if column > 0 {
                            if column > current_indent {
                                // Indent
                                indent_stack.push(column);
                                tokens.push(TokenWithContext {
                                    token: Token::Indent(column),
                                    span: Span::new(span_start - column, span_start),
                                    text: " ".repeat(column),
                                    line: current_line,
                                    column: 0,
                                });
                            } else if column < current_indent {
                                // Dedent - pop until we match
                                while *indent_stack.last().unwrap_or(&0) > column {
                                    indent_stack.pop();
                                    tokens.push(TokenWithContext {
                                        token: Token::Dedent,
                                        span: Span::new(span_start - column, span_start),
                                        text: String::new(),
                                        line: current_line,
                                        column: 0,
                                    });
                                }

                                // Check for inconsistent indentation
                                if *indent_stack.last().unwrap_or(&0) != column {
                                    return Err(LexerError {
                                        message: "Inconsistent indentation: does not match any outer block".to_string(),
                                        span: SourceSpan::from(span_start..span_end),
                                        source_code: source.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }

                tokens.push(TokenWithContext {
                    token: tok,
                    span: Span::new(span_start, span_end),
                    text: source[span_start..span_end].to_string(),
                    line: current_line,
                    column,
                });
            }
        }
    }

    // Emit any remaining dedents at end of file
    while indent_stack.len() > 1 {
        indent_stack.pop();
        tokens.push(TokenWithContext {
            token: Token::Dedent,
            span: Span::new(source.len(), source.len()),
            text: String::new(),
            line: current_line,
            column: 0,
        });
    }

    // Add Eof marker
    tokens.push(TokenWithContext {
        token: Token::Eof,
        span: Span::new(source.len(), source.len()),
        text: String::new(),
        line: current_line,
        column: 0,
    });

    Ok(tokens)
}

#[allow(dead_code)]
/// Filter tokens for parsing (remove comments, handle newlines)
pub fn filter_tokens(tokens: Vec<TokenWithContext>) -> Vec<TokenWithContext> {
    tokens
        .into_iter()
        .filter(|t| !matches!(t.token, Token::Comment))
        .collect()
}

#[allow(dead_code)]
/// Get line and column from byte offset
pub fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut last_line_start = 0;

    for (i, c) in source.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            last_line_start = i + 1;
        }
    }

    (line, offset - last_line_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokenize() {
        let source = "let x = 10";
        let tokens = tokenize(source).unwrap();
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwLet)));
        assert!(tokens
            .iter()
            .any(|t| matches!(&t.token, Token::Ident(s) if s == "x")));
    }

    #[test]
    fn test_indentation() {
        let source = "think main\n  let x = 10\n  give x";
        let tokens = tokenize(source).unwrap();
        let has_indent = tokens.iter().any(|t| matches!(t.token, Token::Indent(_)));
        assert!(has_indent);
    }

    #[test]
    fn test_string_literal() {
        let source = r#""hello world""#;
        let tokens = tokenize(source).unwrap();
        let str_tok = tokens.iter().find(|t| matches!(t.token, Token::LitStr(_)));
        assert!(str_tok.is_some());
    }

    #[test]
    fn test_stream_tokens() {
        let source = "stream lazy";
        let tokens = tokenize(source).unwrap();

        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwStream)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwLazy)));
    }

    #[test]
    fn test_flow_tokens() {
        let source = "animate spring physics stiffness damping snap pan";
        let tokens = tokenize(source).unwrap();

        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwAnimate)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwSpring)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwPhysics)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwStiffness)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwDamping)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwSnap)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwPan)));
    }

    #[test]
    fn test_live_tokens() {
        let source = "sync presence transform";
        let tokens = tokenize(source).unwrap();

        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwSync)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwPresence)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwTransform)));
    }

    #[test]
    fn test_ghost_tokens() {
        let source = "ghost on";
        let tokens = tokenize(source).unwrap();

        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwGhost)));
    }

    #[test]
    fn test_safe_tokens() {
        let source = "tap guard confirm";
        let tokens = tokenize(source).unwrap();

        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwTap)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwGuard)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::KwConfirm)));
    }
}
