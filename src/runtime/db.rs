#![allow(dead_code)]
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseEngine {
    Sqlite,
    Postgres,
    Mysql,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub engine: DatabaseEngine,
    pub url: String,
    pub max_connections: u32,
}

impl DatabaseConfig {
    pub fn validate(&self) -> DbResult<()> {
        if self.url.trim().is_empty() {
            return Err(DbError {
                message: "Database URL cannot be empty".to_string(),
                span: (0..0).into(),
            });
        }

        let ok_scheme = match self.engine {
            DatabaseEngine::Sqlite => self.url.starts_with("sqlite:"),
            DatabaseEngine::Postgres => {
                self.url.starts_with("postgres:") || self.url.starts_with("postgresql:")
            }
            DatabaseEngine::Mysql => self.url.starts_with("mysql:"),
        };

        if !ok_scheme {
            return Err(DbError {
                message: format!(
                    "Database URL '{}' does not match configured engine {:?}",
                    self.url, self.engine
                ),
                span: (0..0).into(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseRuntime {
    pub config: DatabaseConfig,
    pub connected: bool,
}

impl DatabaseRuntime {
    pub async fn connect(config: DatabaseConfig) -> DbResult<Self> {
        config.validate()?;

        Ok(Self {
            config,
            connected: true,
        })
    }

    pub fn ping(&self) -> DbResult<()> {
        if self.connected {
            Ok(())
        } else {
            Err(DbError {
                message: "Database runtime is not connected".to_string(),
                span: (0..0).into(),
            })
        }
    }
}

/// Database runtime error
#[derive(Error, Debug, Diagnostic)]
#[error("Database error: {message}")]
#[diagnostic(code(ved::db))]
pub struct DbError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

pub type DbResult<T> = Result<T, DbError>;
