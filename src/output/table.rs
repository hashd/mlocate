use super::human;
use colored::Colorize;
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;

pub struct TableResult {
    pub full_path: String,
    pub size: i64,
    pub mtime: i64,
    pub mime_type: String,
}

pub fn render_table(
    results: &[TableResult],
    icons: bool,
    use_color: bool,
) -> String {
    let term_width = term_width();

    if results.is_empty() {
        return String::new();
    }

    if term_width <= 50 {
        return render_narrow(results, icons, use_color);
    }

    let path_width = ((term_width as f64 * 0.6) as usize).max(20).min(120);
    let _size_width = 12usize;
    let _time_width = 14usize;

    let mut table = comfy_table::Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(term_width as u16)
        .set_header(vec![
            Cell::new("Path").fg(Color::White),
            Cell::new("Size").fg(Color::White),
            Cell::new("Modified").fg(Color::White),
        ]);

    for r in results {
        let icon_str = if icons {
            super::icons::get_icon(&r.full_path, &r.mime_type)
        } else {
            String::new()
        };

        let display_path = format_path(&r.full_path, path_width);
        let filename = std::path::Path::new(&r.full_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&r.full_path);

        let path_cell = if use_color {
            let cell_text = if display_path.contains("...") {
                let parts: Vec<&str> = display_path.rsplitn(2, '/').collect();
                let file = parts.first().unwrap_or(&"");
                let prefix = if parts.len() > 1 { parts[1] } else { "" };
                format!("{}{}{}/{}", icon_str, prefix, "...", file.bold().bright_white())
            } else if let Some(idx) = display_path.rfind(filename) {
                let prefix = &display_path[..idx];
                format!("{}{}{}", icon_str, prefix, filename.bold().bright_white())
            } else {
                format!("{}{}", icon_str, display_path)
            };
            Cell::new(cell_text)
        } else {
            Cell::new(format!("{}{}", icon_str, display_path))
        };

        let size_str = human::format_size(r.size);
        let size_cell = if use_color {
            Cell::new(size_str).fg(to_comfy_color(human::format_size_color(r.size)))
        } else {
            Cell::new(size_str)
        };

        let time_str = human::format_relative_time(r.mtime);
        let time_cell = if use_color {
            Cell::new(time_str).fg(to_comfy_color(human::format_time_color(r.mtime)))
        } else {
            Cell::new(time_str)
        };

        table.add_row(vec![path_cell, size_cell, time_cell]);
    }

    table.to_string()
}

fn render_narrow(results: &[TableResult], icons: bool, use_color: bool) -> String {
    let mut output = String::new();
    for r in results {
        let icon_str = if icons {
            super::icons::get_icon(&r.full_path, &r.mime_type)
        } else {
            String::new()
        };
        if use_color {
            let filename = std::path::Path::new(&r.full_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&r.full_path);
            output.push_str(&format!("{}{}{}\n", icon_str, &r.full_path[..r.full_path.len()-filename.len()], filename.bold().bright_white()));
        } else {
            output.push_str(&format!("{}{}\n", icon_str, r.full_path));
        }
    }
    output
}

fn format_path(path: &str, max_width: usize) -> String {
    if path.len() <= max_width {
        return path.to_string();
    }
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    if filename.len() >= max_width - 3 {
        return filename.to_string();
    }
    let prefix_len = max_width - 3 - filename.len() - 1;
    format!("...{}/{}", &path[path.len() - prefix_len - filename.len() - 1..path.len() - filename.len() - 1], filename)
}

fn to_comfy_color(c: colored::Color) -> comfy_table::Color {
    use colored::Color as C;
    match c {
        C::White => comfy_table::Color::White,
        C::Green => comfy_table::Color::Green,
        C::Yellow => comfy_table::Color::Yellow,
        C::Red => comfy_table::Color::Red,
        C::BrightRed => comfy_table::Color::DarkRed,
        C::BrightGreen => comfy_table::Color::DarkGreen,
        C::Magenta => comfy_table::Color::Magenta,
        C::TrueColor { r, g, b } => comfy_table::Color::Rgb { r, g, b },
        _ => comfy_table::Color::Reset,
    }
}

fn term_width() -> usize {
    if let Some((w, _)) = term_size::dimensions() {
        w
    } else {
        80
    }
}
