#![allow(dead_code)]
use miette::Diagnostic;
use thiserror::Error;

/// General VED error type that wraps all sub-errors
#[derive(Error, Debug, Diagnostic)]
pub enum VedError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Lex(#[from] crate::lexer::LexerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] crate::parser::ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Type(#[from] crate::typeck::TypeError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Runtime(#[from] crate::interpreter::RuntimeError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Compile(#[from] crate::compiler::CompileError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Server(#[from] crate::runtime::server::ServerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Web(#[from] crate::runtime::web::WebError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Db(#[from] crate::runtime::db::DbError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Auth(#[from] crate::runtime::auth::AuthError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for VED operations
pub type VedResult<T> = Result<T, VedError>;
