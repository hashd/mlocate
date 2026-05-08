use std::path::Path;
use std::sync::OnceLock;

static NERD_FONT_AVAILABLE: OnceLock<bool> = OnceLock::new();

pub fn detect_nerd_font() -> bool {
    *NERD_FONT_AVAILABLE.get_or_init(|| {
        if !is_terminal::is_terminal(&std::io::stdout()) {
            return false;
        }
        if std::env::var("TERM").unwrap_or_default() == "dumb" {
            return false;
        }
        true
    })
}

pub fn get_icon(path: &str, mime_type: &str) -> String {
    let p = Path::new(path);
    let has_nerd = detect_nerd_font();

    if p.is_dir() {
        return if has_nerd { "\u{fc6e} " } else { "[dir] " }.to_string();
    }

    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "rs" => if has_nerd { "\u{e7a8} " } else { "[rs] " },
        "py" => if has_nerd { "\u{e606} " } else { "[py] " },
        "js" => if has_nerd { "\u{e74e} " } else { "[js] " },
        "ts" => if has_nerd { "\u{e628} " } else { "[ts] " },
        "json" => if has_nerd { "\u{e60b} " } else { "[json] " },
        "yaml" | "yml" => if has_nerd { "\u{f16f} " } else { "[yaml] " },
        "md" => if has_nerd { "\u{f48a} " } else { "[md] " },
        "pdf" => if has_nerd { "\u{f724} " } else { "[pdf] " },
        "png" | "jpg" | "jpeg" | "gif" | "svg" => if has_nerd { "\u{f7e8} " } else { "[img] " },
        "zip" | "tar" | "gz" | "bz2" => if has_nerd { "\u{f1c6} " } else { "[zip] " },
        "sh" | "bash" | "zsh" => if has_nerd { "\u{e795} " } else { "[sh] " },
        "c" | "h" | "cpp" | "hpp" => if has_nerd { "\u{e61e} " } else { "[c] " },
        "go" => if has_nerd { "\u{e627} " } else { "[go] " },
        "rb" => if has_nerd { "\u{e791} " } else { "[rb] " },
        "css" => if has_nerd { "\u{e749} " } else { "[css] " },
        "html" => if has_nerd { "\u{f13b} " } else { "[html] " },
        "toml" => if has_nerd { "\u{e6b2} " } else { "[toml] " },
        "lock" => if has_nerd { "\u{f023} " } else { "[lock] " },
        _ => if has_nerd { "\u{f713} " } else { "[?] " },
    }.to_string()
}
