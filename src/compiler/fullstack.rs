use crate::ast::Program;
use crate::compiler::{self, CompileError, CompileResult};
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct FullstackLayout {
    web_dir: PathBuf,
    server_dir: PathBuf,
}

impl FullstackLayout {
    fn from_root(root: &Path) -> Self {
        Self {
            web_dir: root.join("web"),
            server_dir: root.join("server"),
        }
    }
}

/// Emit fullstack target (web + server) as a coordinated bundle.
pub fn emit(program: Program, out: &Path, release: bool) -> CompileResult<()> {
    let layout = FullstackLayout::from_root(out);

    std::fs::create_dir_all(&layout.web_dir).map_err(io_to_compile_error)?;
    std::fs::create_dir_all(&layout.server_dir).map_err(io_to_compile_error)?;

    compiler::web::emit(program.clone(), &layout.web_dir, release)?;
    compiler::server::emit(program, &layout.server_dir, release)?;

    let manifest = json!({
        "target": "fullstack",
        "mode": if release { "release" } else { "debug" },
        "outputs": {
            "web": layout.web_dir,
            "server": layout.server_dir
        }
    });

    std::fs::write(
        out.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).map_err(|e| CompileError {
            message: format!("Failed to serialize fullstack manifest: {e}"),
            span: (0..0).into(),
        })?,
    )
    .map_err(io_to_compile_error)?;

    println!("  fullstack  ->  {}", out.display());
    Ok(())
}

fn io_to_compile_error(err: std::io::Error) -> CompileError {
    CompileError {
        message: format!("I/O error during fullstack emit: {err}"),
        span: (0..0).into(),
    }
}
