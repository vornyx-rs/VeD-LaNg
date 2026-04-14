#![allow(dead_code)]
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DevServerConfig {
    pub root: PathBuf,
    pub host: String,
    pub port: u16,
    pub hot_reload: bool,
}

impl Default for DevServerConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("dist"),
            host: "127.0.0.1".to_string(),
            port: 5173,
            hot_reload: true,
        }
    }
}

pub fn render_hot_reload_client() -> &'static str {
    r#"(() => {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const ws = new WebSocket(`${proto}://${location.host}/__ved_hmr`);
  ws.onmessage = (ev) => {
    if (ev.data === 'reload') location.reload();
  };
})();"#
}

pub fn startup_banner(cfg: &DevServerConfig) -> String {
    format!(
        "VED dev server (hot reload: {}) on http://{}:{} serving {}",
        cfg.hot_reload,
        cfg.host,
        cfg.port,
        cfg.root.display()
    )
}
