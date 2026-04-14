#![allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OverlayDiagnostic {
    pub title: String,
    pub message: String,
    pub location: Option<String>,
}

pub fn render_error_overlay(diagnostics: &[OverlayDiagnostic]) -> String {
    let mut body = String::new();

    for d in diagnostics {
        let location = d.location.as_deref().unwrap_or("unknown location");
        body.push_str(&format!(
            "<div class=\"ved-overlay-item\"><h3>{}</h3><pre>{}</pre><small>{}</small></div>",
            html_escape(&d.title),
            html_escape(&d.message),
            html_escape(location),
        ));
    }

    format!(
        "<div id=\"ved-error-overlay\"><style>{}</style><div class=\"ved-overlay-panel\">{}</div></div>",
        overlay_styles(),
        body
    )
}

fn overlay_styles() -> &'static str {
    "#ved-error-overlay{position:fixed;inset:0;background:rgba(10,10,10,.7);z-index:99999;color:#ffe8e8;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}.ved-overlay-panel{margin:32px auto;max-width:960px;background:#1b1116;border:1px solid #5a2f3f;border-radius:12px;padding:16px}.ved-overlay-item{padding:12px;border-bottom:1px solid #3f2a32}.ved-overlay-item:last-child{border-bottom:0}.ved-overlay-item h3{margin:0 0 8px;color:#ffb4c4}.ved-overlay-item pre{white-space:pre-wrap;margin:0 0 8px}.ved-overlay-item small{opacity:.8}"
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
