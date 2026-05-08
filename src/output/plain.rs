pub fn render_plain(results: &[String]) -> String {
    results.join("\n")
}

pub fn render_null(results: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for r in results {
        out.extend_from_slice(r.as_bytes());
        out.push(0u8);
    }
    out
}
