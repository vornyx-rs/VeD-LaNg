#![allow(dead_code)]
use crate::lexer::{Span, Token, TokenWithContext};

/// Parser cursor for recursive descent parsing
/// Tracks position and provides lookahead/consume operations
pub struct Cursor {
    tokens: Vec<TokenWithContext>,
    pos: usize,
}

impl Cursor {
    pub fn new(tokens: Vec<TokenWithContext>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Peek at current token without consuming
    pub fn peek(&self) -> Option<&TokenWithContext> {
        self.tokens.get(self.pos)
    }

    /// Peek at token n positions ahead
    pub fn peek_n(&self, n: usize) -> Option<&TokenWithContext> {
        self.tokens.get(self.pos + n)
    }

    /// Get current token and advance
    pub fn advance(&mut self) -> Option<&TokenWithContext> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    /// Check if current token matches expected
    pub fn check(&self, expected: &Token) -> bool {
        matches!(self.peek(), Some(t) if t.token == *expected)
    }

    /// Check if current token is a specific identifier
    pub fn check_ident(&self, name: &str) -> bool {
        matches!(self.peek(), Some(t) if t.token.ident_name() == Some(name))
    }

    /// Check if current token is an identifier (any)
    pub fn is_ident(&self) -> bool {
        matches!(self.peek(), Some(t) if t.token.is_ident())
    }

    /// Check if current token can start an expression
    pub fn is_expr_start(&self) -> bool {
        matches!(self.peek(), Some(t) if t.token.is_expr_start())
    }

    /// Check if current token is a statement start
    pub fn is_stmt_start(&self) -> bool {
        matches!(self.peek(), Some(t) if t.token.is_stmt_start())
    }

    /// Expect a specific token, error if not found
    pub fn expect(&mut self, expected: Token) -> Result<&TokenWithContext, String> {
        match self.peek() {
            Some(t) if t.token == expected => {
                self.advance();
                Ok(&self.tokens[self.pos - 1])
            }
            Some(t) => Err(format!("Expected {}, found {}", expected, t.token)),
            None => Err(format!("Expected {}, found end of file", expected)),
        }
    }

    /// Check if we're at end of file
    pub fn is_eof(&self) -> bool {
        matches!(self.peek(), Some(t) if matches!(t.token, Token::Eof)) || self.peek().is_none()
    }

    /// Get span of current token
    pub fn span(&self) -> Span {
        self.peek().map(|t| t.span).unwrap_or_else(|| {
            // Return last token's end or empty
            self.tokens
                .last()
                .map(|t| Span::new(t.span.end, t.span.end))
                .unwrap_or_else(Span::empty)
        })
    }

    /// Get line and column of current token
    pub fn position(&self) -> (usize, usize) {
        self.peek().map(|t| (t.line, t.column)).unwrap_or((0, 0))
    }

    /// Skip newlines and return count
    pub fn skip_newlines(&mut self) -> usize {
        let mut count = 0;
        while matches!(self.peek(), Some(t) if matches!(t.token, Token::Newline)) {
            self.advance();
            count += 1;
        }
        count
    }

    /// Skip any whitespace-like tokens (newlines, comments)
    pub fn skip_whitespace(&mut self) {
        loop {
            match self.peek() {
                Some(t) if matches!(t.token, Token::Newline | Token::Comment) => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    /// Check for indentation token
    pub fn check_indent(&self) -> Option<usize> {
        match self.peek() {
            Some(t) => match &t.token {
                Token::Indent(level) => Some(*level),
                _ => None,
            },
            None => None,
        }
    }

    /// Expect and consume an indent token
    pub fn expect_indent(&mut self) -> Result<usize, String> {
        match self.peek() {
            Some(t) => match &t.token {
                Token::Indent(level) => {
                    let level = *level;
                    self.advance();
                    Ok(level)
                }
                _ => Err(format!("Expected indentation, found {}", t.token)),
            },
            None => Err("Expected indentation, found end of file".to_string()),
        }
    }

    /// Check for dedent token
    pub fn check_dedent(&self) -> bool {
        matches!(self.peek(), Some(t) if matches!(t.token, Token::Dedent))
    }

    /// Check for end of block
    pub fn check_end_of_block(&self) -> bool {
        matches!(self.peek(), Some(t) if matches!(t.token, Token::EndOfBlock | Token::Dedent | Token::Eof))
    }

    /// Consume if token matches
    pub fn consume_if(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Get the source text of current token
    pub fn current_text(&self) -> Option<&str> {
        self.peek().map(|t| t.text.as_str())
    }

    /// Create a span from previous position to current
    pub fn span_since(&self, start: Span) -> Span {
        let end = self.span();
        Span::new(start.start, end.end)
    }

    /// Backtrack by n positions (use with caution)
    pub fn backtrack(&mut self, n: usize) {
        self.pos = self.pos.saturating_sub(n);
    }

    /// Get remaining token count
    pub fn remaining(&self) -> usize {
        self.tokens.len().saturating_sub(self.pos)
    }

    /// Check if current token can start an expression
    pub fn check_expr_start(&self) -> bool {
        matches!(self.peek(), Some(t) if t.token.is_expr_start())
    }
}

/// Helper to check if token is a keyword that could be an identifier
pub fn is_reserved_keyword(token: &Token) -> bool {
    matches!(
        token,
        Token::KwShape
            | Token::KwThink
            | Token::KwLet
            | Token::KwGive
            | Token::KwFail
            | Token::KwWhen
            | Token::KwEach
            | Token::KwScreen
            | Token::KwPiece
            | Token::KwServe
            | Token::KwDatabase
            | Token::KwTask
            | Token::KwQueue
            | Token::KwLive
            | Token::KwAuth
            | Token::KwRoutes
            | Token::KwRemember
            | Token::KwBox
            | Token::KwWords
            | Token::KwTap
            | Token::KwField
            | Token::KwImage
            | Token::KwLoad
            | Token::KwConnect
            | Token::KwGo
            | Token::KwBack
            | Token::KwNeeds
            | Token::KwParam
            | Token::KwShow
            | Token::KwHide
            | Token::KwEnter
            | Token::KwLeave
            | Token::KwHover
            | Token::KwPress
            | Token::KwLoop
            | Token::KwMove
            | Token::KwEase
            | Token::KwDuration
            | Token::KwFill
            | Token::KwTall
            | Token::KwFlow
            | Token::KwGap
            | Token::KwPadding
            | Token::KwCenter
            | Token::KwPush
            | Token::KwColor
            | Token::KwRadius
            | Token::KwBorder
            | Token::KwShadow
            | Token::KwBlur
            | Token::KwOpacity
            | Token::KwClip
            | Token::KwScroll
            | Token::KwCursor
            | Token::KwSize
            | Token::KwWeight
            | Token::KwAlign
            | Token::KwSpacing
            | Token::KwLines
            | Token::KwCut
            | Token::KwPort
            | Token::KwPrefix
            | Token::KwPath
            | Token::KwSecret
            | Token::KwExpires
            | Token::KwKind
            | Token::KwUrl
            | Token::KwPool
            | Token::KwAs
            | Token::KwBroadcast
            | Token::KwTransaction
            | Token::KwEvery
            | Token::KwWorkers
            | Token::KwRetry
            | Token::KwOtherwise
            | Token::KwAnd
            | Token::KwOr
            | Token::KwNot
            | Token::KwIs
            | Token::KwIn
            | Token::KwWith
            | Token::KwNothing
            | Token::KwAsync
            | Token::KwOn
            | Token::KwOk
            | Token::KwMaybe
            | Token::KwKeep
            | Token::KwFold
            | Token::KwTake
            | Token::KwSort
            | Token::KwGuard
            | Token::KwReply
            | Token::KwMut
            | Token::KwConfirm
            | Token::KwFetch
            | Token::KwCache
            | Token::KwLoading
            | Token::KwError
            | Token::KwGhost
            | Token::KwStream
            | Token::KwLazy
            | Token::KwAnimate
            | Token::KwSpring
            | Token::KwPhysics
            | Token::KwStiffness
            | Token::KwDamping
            | Token::KwMass
            | Token::KwSnap
            | Token::KwPan
            | Token::KwFrom
            | Token::KwTo
            | Token::KwScale
            | Token::KwRotate
            | Token::KwVelocity
            | Token::KwOffset
            | Token::KwRelease
            | Token::KwSync
            | Token::KwPresence
            | Token::KwTransform
            | Token::KwAutomatic
            | Token::KwManual
            | Token::KwLWW
            | Token::KwPNCounter
            | Token::KwGCounter
            | Token::KwMVRegister
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn test_cursor_navigation() {
        let source = "let x = 10";
        let tokens = tokenize(source).unwrap();
        let mut cursor = Cursor::new(tokens);

        assert!(cursor.check(&Token::KwLet));
        cursor.advance();
        assert!(cursor.is_ident());
    }
}
