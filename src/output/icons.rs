use std::path::Path;

pub fn get_icon(path: &str) -> String {
    let p = Path::new(path);

    if p.is_dir() {
        return "\u{fc6e} ".to_string();
    }

    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "rs" => "\u{e7a8} ",
        "py" => "\u{e606} ",
        "js" => "\u{e74e} ",
        "ts" => "\u{e628} ",
        "json" => "\u{e60b} ",
        "yaml" | "yml" => "\u{f16f} ",
        "md" => "\u{f48a} ",
        "pdf" => "\u{f724} ",
        "png" | "jpg" | "jpeg" | "gif" | "svg" => "\u{f7e8} ",
        "zip" | "tar" | "gz" | "bz2" => "\u{f1c6} ",
        "sh" | "bash" | "zsh" => "\u{e795} ",
        "c" | "h" | "cpp" | "hpp" => "\u{e61e} ",
        "go" => "\u{e627} ",
        "rb" => "\u{e791} ",
        "css" => "\u{e749} ",
        "html" => "\u{f13b} ",
        "toml" => "\u{e6b2} ",
        "lock" => "\u{f023} ",
        _ => "\u{f713} ",
    }.to_string()
}
