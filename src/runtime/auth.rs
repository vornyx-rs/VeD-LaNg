#![allow(dead_code)]
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use miette::{Diagnostic, SourceSpan};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub issuer: String,
    pub audience: String,
    pub secret: String,
    pub expiration_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Clone)]
pub struct AuthService {
    config: AuthConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
}

impl AuthService {
    pub fn new(config: AuthConfig) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&config.issuer]);
        validation.set_audience(&[&config.audience]);

        Self {
            encoding_key: EncodingKey::from_secret(config.secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(config.secret.as_bytes()),
            validation,
            config,
        }
    }

    pub fn issue_token(&self, subject: impl Into<String>) -> AuthResult<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AuthError {
                message: format!("Failed to get system time: {e}"),
                span: (0..0).into(),
            })?
            .as_secs();

        let claims = Claims {
            sub: subject.into(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            iat: now as usize,
            exp: (now + self.config.expiration_seconds) as usize,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| AuthError {
            message: format!("Failed to issue JWT: {e}"),
            span: (0..0).into(),
        })
    }

    pub fn validate_token(&self, token: &str) -> AuthResult<TokenData<Claims>> {
        decode::<Claims>(token, &self.decoding_key, &self.validation).map_err(|e| AuthError {
            message: format!("Invalid JWT: {e}"),
            span: (0..0).into(),
        })
    }
}

/// Authentication runtime error
#[derive(Error, Debug, Diagnostic)]
#[error("Auth error: {message}")]
#[diagnostic(code(ved::auth))]
pub struct AuthError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

pub type AuthResult<T> = Result<T, AuthError>;
