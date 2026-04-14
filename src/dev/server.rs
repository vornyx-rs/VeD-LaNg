use axum::Router;
use std::path::PathBuf;
use tower_http::services::ServeDir;

/// Start a static file server serving `root` on `port`.
/// Blocks until Ctrl-C.
pub async fn serve(root: PathBuf, port: u16) -> Result<(), String> {
    let addr = format!("127.0.0.1:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Cannot bind to port {}: {}", port, e))?;

    let app = Router::new().nest_service("/", ServeDir::new(&root));

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {}", e))
}

/// Try to find a free port starting from `start`.
pub fn find_port(start: u16) -> u16 {
    for port in start..start + 20 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    start
}

/// Open a URL in the default system browser.
pub fn open_browser(url: &str) {
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();

    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn();
}

// Keep stub types so nothing else breaks
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DevServerConfig {
    pub root: PathBuf,
    pub host: String,
    pub port: u16,
    pub hot_reload: bool,
}

#[allow(dead_code)]
impl Default for DevServerConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("dist"),
            host: "127.0.0.1".to_string(),
            port: 5173,
            hot_reload: false,
        }
    }
}

#[allow(dead_code)]
pub fn startup_banner(cfg: &DevServerConfig) -> String {
    format!(
        "VED dev server on http://{}:{} serving {}",
        cfg.host,
        cfg.port,
        cfg.root.display()
    )
}
