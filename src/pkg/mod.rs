#![allow(dead_code)]
use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

pub fn write_manifest(path: &Path, manifest: &PackageManifest) -> Result<()> {
    let content = serde_json::to_string_pretty(manifest).into_diagnostic()?;
    fs::write(path, content).into_diagnostic()?;
    Ok(())
}
