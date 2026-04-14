#![allow(dead_code)]

use crate::ast::*;
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Web runtime error
#[derive(Error, Debug, Diagnostic)]
#[error("Web runtime error: {message}")]
#[diagnostic(code(ved::web))]
pub struct WebError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

pub type WebResult<T> = Result<T, WebError>;

/// Start the web runtime (dev preview mode)
pub fn start(program: Program, verbose: bool) -> WebResult<()> {
    // Collect screens
    let screens: Vec<&ScreenDef> = program
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Screen(s) = item {
                Some(s)
            } else {
                None
            }
        })
        .collect();

    if screens.is_empty() {
        return Err(WebError {
            message: "No 'screen' blocks found for web target".to_string(),
            span: (0..0).into(),
        });
    }

    if verbose {
        eprintln!(
            "[vedc] Screens: {}",
            screens
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        for screen in &screens {
            if !screen.state.is_empty() {
                eprintln!("[vedc] Screen '{}':", screen.name);
                for state in &screen.state {
                    eprintln!("[vedc]   - {} = {:?}", state.name, state.default);
                }
            }
        }
    }

    println!("Web preview: use 'vedc build --target web' to build for the browser.");

    Ok(())
}
