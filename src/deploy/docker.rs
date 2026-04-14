#![allow(dead_code)]
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DockerSpec {
    pub app_name: String,
    pub binary_name: String,
    pub port: u16,
}

impl Default for DockerSpec {
    fn default() -> Self {
        Self {
            app_name: "ved-app".to_string(),
            binary_name: "vedc".to_string(),
            port: 8080,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedDockerArtifacts {
    pub dockerfile_path: PathBuf,
    pub compose_path: PathBuf,
}

pub fn generate(out_dir: &Path, spec: &DockerSpec) -> io::Result<GeneratedDockerArtifacts> {
    fs::create_dir_all(out_dir)?;

    let dockerfile_path = out_dir.join("Dockerfile");
    let compose_path = out_dir.join("docker-compose.yml");

    let dockerfile = format!(
        "FROM rust:1.75 as builder\n\
         WORKDIR /app\n\
         COPY . .\n\
         RUN cargo build --release --bin {binary_name}\n\n\
         FROM debian:bookworm-slim\n\
         WORKDIR /app\n\
         COPY --from=builder /app/target/release/{binary_name} /usr/local/bin/{binary_name}\n\
         EXPOSE {port}\n\
         CMD [\"{binary_name}\", \"run\", \"/app/Main.ved\", \"--target\", \"server\"]\n",
        binary_name = spec.binary_name,
        port = spec.port
    );

    let compose = format!(
        "version: '3.9'\nservices:\n  {app_name}:\n    build: .\n    ports:\n      - \"{port}:{port}\"\n    restart: unless-stopped\n",
        app_name = spec.app_name,
        port = spec.port
    );

    fs::write(&dockerfile_path, dockerfile)?;
    fs::write(&compose_path, compose)?;

    Ok(GeneratedDockerArtifacts {
        dockerfile_path,
        compose_path,
    })
}
