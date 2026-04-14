#![allow(dead_code)]
pub fn minify_js(input: &str) -> String {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("")
}

pub fn minify_css(input: &str) -> String {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("/*"))
        .collect::<Vec<_>>()
        .join("")
}
