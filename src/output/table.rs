use super::human;
use colored::Colorize;

pub struct TableResult {
    pub full_path: String,
    pub size: u64,
    pub mtime: i64,
    pub mime_type: String,
}

pub fn render_table(results: &[TableResult], icons: bool, use_color: bool) -> String {
    let term_width = term_width();

    if results.is_empty() {
        return String::new();
    }

    if term_width <= 50 {
        return render_narrow(results, icons, use_color);
    }

    let mut max_size = 4;
    let mut max_time = 8;
    for r in results {
        let s = human::format_size(r.size);
        let t = human::format_relative_time(r.mtime);
        max_size = max_size.max(s.len());
        max_time = max_time.max(t.len());
    }

    let gap = 2;
    let path_width = term_width
        .saturating_sub(max_size + max_time + gap * (3 - 1))
        .max(10);

    let sep = '─';
    let mut output = String::new();

    output.push_str(&header_line(path_width, max_size, max_time, gap, use_color));
    output.push('\n');
    output.push_str(&separator_line(path_width, max_size, max_time, gap, sep));
    output.push('\n');

    for r in results {
        output.push_str(&data_line(r, path_width, max_size, max_time, gap, icons, use_color));
        output.push('\n');
    }

    output
}

fn header_line(path_w: usize, size_w: usize, time_w: usize, gap: usize, use_color: bool) -> String {
    let gap_str = " ".repeat(gap);
    if use_color {
        format!(
            "{:path_w$}{gap}{:>size_w$}{gap}{:>time_w$}",
            "Path".white().bold(),
            "Size".white().bold(),
            "Modified".white().bold(),
            gap = gap_str,
        )
    } else {
        format!(
            "{:path_w$}{gap}{:>size_w$}{gap}{:>time_w$}",
            "Path", "Size", "Modified",
            gap = gap_str,
        )
    }
}

fn separator_line(path_w: usize, size_w: usize, time_w: usize, gap: usize, c: char) -> String {
    let gap_str = " ".repeat(gap);
    format!(
        "{path_sep}{gap}{size_sep}{gap}{time_sep}",
        path_sep = c.to_string().repeat(path_w),
        gap = gap_str,
        size_sep = c.to_string().repeat(size_w),
        time_sep = c.to_string().repeat(time_w),
    )
}

fn data_line(
    r: &TableResult,
    path_w: usize,
    size_w: usize,
    time_w: usize,
    gap: usize,
    icons: bool,
    use_color: bool,
) -> String {
    let icon_str = if icons {
        super::icons::get_icon(&r.full_path)
    } else {
        String::new()
    };

    let display_path = format_path(&r.full_path, path_w.saturating_sub(icon_str.len()));
    let filename = std::path::Path::new(&r.full_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&r.full_path);

    let path_str = if use_color {
        if display_path.contains("...") {
            let parts: Vec<&str> = display_path.rsplitn(2, '/').collect();
            let file = parts.first().unwrap_or(&"");
            let prefix = if parts.len() > 1 { parts[1] } else { "" };
            format!(
                "{}{}{}/{}",
                icon_str,
                prefix,
                "...",
                file.bold().bright_white()
            )
        } else if let Some(idx) = display_path.rfind(filename) {
            let prefix = &display_path[..idx];
            format!("{}{}{}", icon_str, prefix, filename.bold().bright_white())
        } else {
            format!("{}{}", icon_str, display_path)
        }
    } else {
        format!("{}{}", icon_str, display_path)
    };

    let size_str = human::format_size(r.size);
    let size_color = human::format_size_color(r.size);
    let size_text = if use_color {
        size_str.color(size_color).to_string()
    } else {
        size_str
    };

    let time_str = human::format_relative_time(r.mtime);
    let time_color = human::format_time_color(r.mtime);
    let time_text = if use_color {
        time_str.color(time_color).to_string()
    } else {
        time_str
    };

    let gap_str = " ".repeat(gap);

    let path_visible = visible_width(&path_str);
    let path_pad = if path_visible < path_w {
        " ".repeat(path_w - path_visible)
    } else {
        String::new()
    };

    let size_visible = visible_width(&size_text);
    let size_pad = if size_visible < size_w {
        " ".repeat(size_w - size_visible)
    } else {
        String::new()
    };

    let time_visible = visible_width(&time_text);
    let time_pad = if time_visible < time_w {
        " ".repeat(time_w - time_visible)
    } else {
        String::new()
    };

    format!(
        "{path}{path_pad}{gap}{size_pad}{size}{gap}{time_pad}{time}",
        path = path_str,
        path_pad = path_pad,
        gap = gap_str,
        size_pad = size_pad,
        size = size_text,
        time_pad = time_pad,
        time = time_text,
    )
}

fn visible_width(s: &str) -> usize {
    strip_ansi_escapes::strip_str(s).len()
}

fn format_path(path: &str, max_width: usize) -> String {
    if path.len() <= max_width {
        return path.to_string();
    }
    let (dir, filename) = path.rsplit_once('/').unwrap_or(("", path));
    if filename.len() >= max_width {
        return filename.to_string();
    }
    let available = max_width - filename.len() - 1;
    if available <= 4 {
        return filename.to_string();
    }
    let dir_trim = dir.len().saturating_sub(available - 3);
    format!("...{}/{}", &dir[dir_trim..], filename)
}

fn term_width() -> usize {
    if let Some((w, _)) = term_size::dimensions() {
        w
    } else {
        80
    }
}

fn render_narrow(results: &[TableResult], icons: bool, use_color: bool) -> String {
    let mut output = String::new();
    for r in results {
        let icon_str = if icons {
            super::icons::get_icon(&r.full_path)
        } else {
            String::new()
        };
        if use_color {
            let filename = std::path::Path::new(&r.full_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&r.full_path);
            output.push_str(&format!(
                "{}{}{}\n",
                icon_str,
                &r.full_path[..r.full_path.len() - filename.len()],
                filename.bold().bright_white()
            ));
        } else {
            output.push_str(&format!("{}{}\n", icon_str, r.full_path));
        }
    }
    output
}
