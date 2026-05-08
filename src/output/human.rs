pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn format_relative_time(mtime: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let diff = now - mtime;

    if diff < 0 {
        return "in the future".to_string();
    }
    if diff < 60 {
        return format!("{} seconds ago", diff);
    }
    if diff < 3600 {
        return format!("{} minutes ago", diff / 60);
    }
    if diff < 86400 {
        return format!("{} hours ago", diff / 3600);
    }
    if diff < 604800 {
        return format!("{} days ago", diff / 86400);
    }
    if diff < 2592000 {
        return format!("{} weeks ago", diff / 604800);
    }
    format!("{} months ago", diff / 2592000)
}

pub fn format_size_color(bytes: u64) -> colored::Color {
    use colored::Color;
    if bytes < 1024 {
        Color::TrueColor {
            r: 128,
            g: 128,
            b: 128,
        }
    } else if bytes < 1024 * 1024 {
        Color::Green
    } else if bytes < 100 * 1024 * 1024 {
        Color::Yellow
    } else if bytes < 1024 * 1024 * 1024 {
        Color::Red
    } else {
        Color::BrightRed
    }
}

pub fn format_time_color(mtime: i64) -> colored::Color {
    use colored::Color;
    let now = chrono::Utc::now().timestamp();
    let diff = now - mtime;
    if diff < 0 {
        Color::Magenta
    } else if diff < 3600 {
        Color::BrightGreen
    } else if diff < 86400 {
        Color::Green
    } else if diff < 604800 {
        Color::Yellow
    } else if diff < 2592000 {
        Color::TrueColor {
            r: 255,
            g: 255,
            b: 255,
        }
    } else {
        Color::TrueColor {
            r: 128,
            g: 128,
            b: 128,
        }
    }
}
