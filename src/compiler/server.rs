use crate::ast::*;
use crate::compiler::{emit_expr, emit_stmt, rust_ident, CompileResult};
use std::path::Path;

/// Emit server target (Axum-based)
pub fn emit(program: Program, out: &Path, release: bool) -> CompileResult<()> {
    // Generate Rust server code
    let rust_code = generate_server_code(&program)?;

    let src_dir = out.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| crate::compiler::CompileError {
        message: format!("Failed to create src directory: {}", e),
        span: (0..0).into(),
    })?;

    std::fs::write(src_dir.join("main.rs"), rust_code).map_err(|e| {
        crate::compiler::CompileError {
            message: format!("Failed to write main.rs: {}", e),
            span: (0..0).into(),
        }
    })?;

    // Generate Cargo.toml for the server
    let cargo_toml = r#"[package]
name = "ved-server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1.38", features = ["full"] }
tower-http = { version = "0.5", features = ["cors"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite", "postgres", "mysql"] }
jsonwebtoken = "9.3"
tokio-tungstenite = "0.23"
"#;

    std::fs::write(out.join("Cargo.toml"), cargo_toml).map_err(|e| {
        crate::compiler::CompileError {
            message: format!("Failed to write Cargo.toml: {}", e),
            span: (0..0).into(),
        }
    })?;

    println!("  server  ->  {}", out.display());
    println!(
        "  To build: cd {} && cargo build{}",
        out.display(),
        if release { " --release" } else { "" }
    );

    Ok(())
}

fn generate_server_code(program: &Program) -> CompileResult<String> {
    let mut code = String::new();

    // Detect if any database is declared
    let has_db = program.items.iter().any(|i| matches!(i, Item::Database(_)));
    let db_field = if has_db {
        "    db: sqlx::SqlitePool,\n"
    } else {
        ""
    };
    let db_init = if has_db {
        r#"    let db = sqlx::SqlitePool::connect(
        &std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:app.db".to_string()),
    )
    .await
    .expect("failed to connect to database");
"#
    } else {
        ""
    };
    let db_state = if has_db { "db, " } else { "" };

    // Collect all route registrations from serve blocks
    let mut route_lines: Vec<String> = Vec::new();
    for item in &program.items {
        if let Item::Serve(serve) = item {
            let prefix = serve.prefix.as_deref().unwrap_or("");
            for route in &serve.routes {
                let method_fn = match route.method {
                    HttpMethod::Get => "get",
                    HttpMethod::Post => "post",
                    HttpMethod::Put => "put",
                    HttpMethod::Patch => "patch",
                    HttpMethod::Del => "delete",
                };
                let full_path = format!("{}{}", prefix, route.pattern);
                route_lines.push(format!(
                    "        .route(\"{}\", {}(handler_{}))",
                    full_path,
                    method_fn,
                    rust_ident(&route.handler)
                ));
            }
        }
    }

    let port = program
        .items
        .iter()
        .find_map(|i| if let Item::Serve(s) = i { s.port } else { None })
        .unwrap_or(8080);

    code.push_str(&format!(
        r#"// Auto-generated VED server — do not edit manually
#![allow(unused_variables, dead_code, unused_imports)]

use axum::{{
    extract::{{Json, Path, Query, State}},
    http::StatusCode,
    response::IntoResponse,
    routing::{{delete, get, patch, post, put}},
    Router,
}};
use serde::{{Deserialize, Serialize}};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{{error, info}};

#[derive(Clone)]
struct AppState {{
{db_field}}}

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {{
    data: Option<T>,
    error: Option<String>,
}}

impl<T: Serialize> ApiResponse<T> {{
    fn ok(data: T) -> Json<Self> {{
        Json(Self {{ data: Some(data), error: None }})
    }}
    fn err(msg: impl Into<String>) -> Json<ApiResponse<()>> {{
        Json(ApiResponse {{ data: None, error: Some(msg.into()) }})
    }}
}}

#[tokio::main]
async fn main() {{
    tracing_subscriber::fmt::init();
{db_init}
    let state = Arc::new(AppState {{ {db_state}}});

    let app = Router::new()
{routes}
        .route("/health", get(handler_health))
        .with_state(state);

    let addr = "0.0.0.0:{port}";
    let listener = TcpListener::bind(addr).await.expect("failed to bind");
    info!("VED server listening on http://{{}}", addr);
    axum::serve(listener, app).await.expect("server error");
}}

async fn handler_health() -> impl IntoResponse {{
    Json(serde_json::json!({{"status": "ok"}}))
}}

"#,
        db_field = db_field,
        db_init = db_init,
        db_state = db_state,
        routes = route_lines.join("\n"),
        port = port,
    ));

    // Generate shape structs for serialization
    for item in &program.items {
        if let Item::Shape(s) = item {
            code.push_str(&generate_shape_struct(s));
        }
    }

    // Generate handler functions from think definitions
    for item in &program.items {
        if let Item::Think(think) = item {
            code.push_str(&generate_handler(think)?);
        }
    }

    // Stub handlers for routes that have no matching think
    for item in &program.items {
        if let Item::Serve(serve) = item {
            for route in &serve.routes {
                let handler_name = format!("handler_{}", rust_ident(&route.handler));
                // Only generate stub if no think with this name exists
                let has_think = program
                    .items
                    .iter()
                    .any(|i| matches!(i, Item::Think(t) if t.name == route.handler));
                if !has_think {
                    code.push_str(&format!(
                        "async fn {}(State(_state): State<Arc<AppState>>) -> impl IntoResponse {{\n",
                        handler_name
                    ));
                    code.push_str(
                        "    ApiResponse::ok(serde_json::json!({\"message\": \"ok\"}))\n",
                    );
                    code.push_str("}\n\n");
                }
            }
        }
    }

    Ok(code)
}

fn generate_shape_struct(shape: &ShapeDef) -> String {
    let fields: Vec<String> = shape
        .fields
        .iter()
        .map(|f| format!("    pub {}: {},", rust_ident(&f.name), type_to_rust(&f.ty)))
        .collect();
    format!(
        "#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct {} {{\n{}\n}}\n\n",
        rust_ident(&shape.name),
        fields.join("\n")
    )
}

fn generate_handler(think: &ThinkDef) -> CompileResult<String> {
    let mut code = String::new();

    let handler_name = format!("handler_{}", rust_ident(&think.name));

    // Build parameter list: state + path params
    let mut sig_params = vec!["State(_state): State<Arc<AppState>>".to_string()];
    for param in &think.params {
        sig_params.push(format!(
            "Path({}): Path<{}>",
            rust_ident(&param.name),
            type_to_rust(&param.ty)
        ));
    }

    code.push_str(&format!(
        "async fn {}({}) -> impl IntoResponse {{\n",
        handler_name,
        sig_params.join(", ")
    ));

    if think.body.is_empty() {
        code.push_str("    ApiResponse::ok(serde_json::json!({\"message\": \"ok\"}))\n");
    } else {
        // Emit actual body statements
        for stmt in &think.body {
            // Intercept top-level `give` to wrap in ApiResponse
            match stmt {
                Stmt::Give { value, .. } => {
                    code.push_str(&format!("    ApiResponse::ok({})\n", emit_expr(value)));
                }
                Stmt::Fail { value, .. } => {
                    code.push_str(&format!(
                        "    return ApiResponse::err({}).into_response();\n",
                        emit_expr(value)
                    ));
                }
                other => {
                    code.push_str(&emit_stmt(other, 1));
                    code.push('\n');
                }
            }
        }
        // If no give statement, emit a default ok
        let has_give = think.body.iter().any(|s| matches!(s, Stmt::Give { .. }));
        if !has_give {
            code.push_str("    ApiResponse::ok(serde_json::json!({\"message\": \"ok\"}))\n");
        }
    }

    code.push_str("}\n\n");
    Ok(code)
}

fn type_to_rust(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Num => "i64".to_string(),
        TypeExpr::Dec => "f64".to_string(),
        TypeExpr::Text => "String".to_string(),
        TypeExpr::Bool => "bool".to_string(),
        TypeExpr::Nothing => "()".to_string(),
        TypeExpr::List(t) => format!("Vec<{}>", type_to_rust(t)),
        TypeExpr::Map(k, v) => {
            format!(
                "std::collections::HashMap<{}, {}>",
                type_to_rust(k),
                type_to_rust(v)
            )
        }
        TypeExpr::Maybe(t) => format!("Option<{}>", type_to_rust(t)),
        TypeExpr::Ok(t, e) => format!("Result<{}, {}>", type_to_rust(t), type_to_rust(e)),
        TypeExpr::Named(n) => rust_ident(n),
        TypeExpr::Any => "serde_json::Value".to_string(),
        TypeExpr::Function(params, ret) => {
            let param_types: Vec<String> = params.iter().map(type_to_rust).collect();
            format!("fn({}) -> {}", param_types.join(", "), type_to_rust(ret))
        }
    }
}
