use crate::ast::*;
use axum::{response::Json, routing::get, Router};
use miette::{Diagnostic, SourceSpan};
use serde_json::json;
use thiserror::Error;

/// Server runtime error
#[derive(Error, Debug, Diagnostic)]
#[error("Server error: {message}")]
#[diagnostic(code(ved::server))]
pub struct ServerError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

pub type ServerResult<T> = Result<T, ServerError>;

/// Start the server runtime
pub async fn start(program: Program) -> ServerResult<()> {
    // Find serve definition
    let serve = program.items.iter().find_map(|item| {
        if let Item::Serve(s) = item {
            Some(s)
        } else {
            None
        }
    });

    let serve = serve.ok_or_else(|| ServerError {
        message: "No 'serve' block found".to_string(),
        span: (0..0).into(),
    })?;

    let port = serve.port.unwrap_or(8080);

    println!("VED Server starting on http://localhost:{}", port);

    // Build router
    let mut router = Router::new().route("/health", get(health_check));

    // Add routes from serve definition
    for route in &serve.routes {
        match route.method {
            crate::ast::HttpMethod::Get => {
                let handler_name = route.handler.clone();
                router = router.route(
                    &route.pattern,
                    get(move || async move {
                        Json(json!({
                            "handler": handler_name,
                            "status": "not yet implemented"
                        }))
                    }),
                );
            }
            _ => {
                // Handle other methods
            }
        }
    }

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| ServerError {
            message: format!("Failed to bind to {}: {}", addr, e),
            span: (0..0).into(),
        })?;

    axum::serve(listener, router)
        .await
        .map_err(|e| ServerError {
            message: format!("Server error: {}", e),
            span: (0..0).into(),
        })?;

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "VED Server",
        "version": "0.1.0"
    }))
}
