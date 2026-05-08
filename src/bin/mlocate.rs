use clap::CommandFactory;
use clap::Parser;
use mlocate::cli::{SearchCli, Shell};
use mlocate::db::{self, query::build_query};
use mlocate::output;

struct SearchResult {
    full_path: String,
    size: i64,
    mtime: i64,
    mode: i32,
    mime_type: String,
}

fn main() -> anyhow::Result<()> {
    let cli = SearchCli::parse();

    mlocate::compat::warn_gnu_stubs(&cli);

    if cli.version {
        println!("mlocate {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if cli.help {
        SearchCli::command().print_help()?;
        println!();
        return Ok(());
    }

    if let Some(shell) = &cli.generate_completions {
        let name = "mlocate";
        match shell {
            Shell::Bash => clap_complete::generate(
                clap_complete::Shell::Bash,
                &mut <SearchCli as clap::CommandFactory>::command(),
                name,
                &mut std::io::stdout(),
            ),
            Shell::Zsh => clap_complete::generate(
                clap_complete::Shell::Zsh,
                &mut <SearchCli as clap::CommandFactory>::command(),
                name,
                &mut std::io::stdout(),
            ),
            Shell::Fish => clap_complete::generate(
                clap_complete::Shell::Fish,
                &mut <SearchCli as clap::CommandFactory>::command(),
                name,
                &mut std::io::stdout(),
            ),
        }
        return Ok(());
    }

    let db_path = mlocate::platform::db_path(cli.database.as_deref());

    if cli.schema {
        if !std::path::Path::new(&db_path).exists() {
            eprintln!("Error: No database found at {}. Run 'mupdatedb' to create one.", db_path);
            std::process::exit(2);
        }
        let conn = match db::open_or_create(&db_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(2);
            }
        };
        let file_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap_or(0);
        let db_size = std::fs::metadata(&db_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let last_idx = chrono::Utc::now().to_rfc3339();
        let json = output::json::render_json_schema(
            &db_path, db_size, file_count, &last_idx,
        );
        println!("{}", json);
        return Ok(());
    }

    if cli.patterns.is_empty() {
        eprintln!("Error: A search pattern is required. Usage: mlocate [OPTIONS] <pattern>");
        std::process::exit(2);
    }

    if !std::path::Path::new(&db_path).exists() {
        eprintln!("Error: No database found at {}. Run 'mupdatedb' to create one.", db_path);
        std::process::exit(2);
    }

    let conn = match db::open_or_create(&db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(2);
        }
    };

    let size_filter = cli.size.as_deref().map(mlocate::filter::parse_size).transpose()?;
    let modified_filter = cli.modified.as_deref().map(mlocate::filter::parse_modified).transpose()?;
    let mime_filter = cli.mime_type.as_deref().map(mlocate::filter::parse_mime_type).transpose()?;

    let q = build_query(
        &cli.patterns,
        cli.ignore_case,
        cli.basename,
        cli.regex,
        cli.existing,
        cli.limit,
        cli.count,
        size_filter.as_ref(),
        modified_filter.as_ref(),
        mime_filter.as_ref(),
    );

    let mut results: Vec<SearchResult> = Vec::new();

    if cli.count {
        let param_values: Vec<&str> = q.params.iter().map(|s| s.as_str()).collect();
        let count: i64 = conn
            .query_row(&q.sql, duckdb::params_from_iter(param_values), |row| row.get(0))
            .unwrap_or_else(|e| {
                eprintln!("Error: Database query failed: {}. The index may be corrupt. Try running 'mupdatedb --force' to rebuild.", e);
                std::process::exit(2)
            });

        if cli.json {
            println!("{}", output::json::render_json_count(count));
        } else {
            println!("{}", count);
        }
        std::process::exit(if count == 0 { 1 } else { 0 });
    }

    {
        let param_values: Vec<&str> = q.params.iter().map(|s| s.as_str()).collect();
        let mut stmt = conn.prepare(&q.sql).unwrap_or_else(|e| {
            eprintln!("Error: Database query failed: {}. The index may be corrupt. Try running 'mupdatedb --force' to rebuild.", e);
            std::process::exit(2)
        });

        let rows = stmt.query_map(duckdb::params_from_iter(param_values), |row| {
            Ok(SearchResult {
                full_path: row.get(1)?,
                size: row.get(2)?,
                mtime: row.get(3)?,
                mode: row.get(4)?,
                mime_type: row.get(5)?,
            })
        }).unwrap_or_else(|e| {
            eprintln!("Error: Database query failed: {}. The index may be corrupt. Try running 'mupdatedb --force' to rebuild.", e);
            std::process::exit(2)
        });

        for row_result in rows {
            match row_result {
                Ok(r) => {
                    if cli.existing && !std::path::Path::new(&r.full_path).exists() {
                        continue;
                    }
                    results.push(r);
                }
                Err(e) => {
                    eprintln!("Warning: failed to read row: {}", e);
                }
            }
        }
    }

    let exit_code = if results.is_empty() { 1 } else { 0 };

    let use_color = match cli.color {
        mlocate::cli::ColorMode::Always => true,
        mlocate::cli::ColorMode::Never => false,
        mlocate::cli::ColorMode::Auto => {
            colored::control::set_override(true);
            is_terminal::is_terminal(std::io::stdout())
        }
    };

    if cli.json {
        let entries: Vec<output::json::JsonFileEntry> = results
            .iter()
            .map(|r| output::json::JsonFileEntry {
                path: r.full_path.clone(),
                size: r.size,
                mtime: r.mtime,
                mode: r.mode,
                mime_type: r.mime_type.clone(),
            })
            .collect();
        let json_output = output::json::render_json(&entries);
        println!("{}", json_output);
    } else if cli.null {
        let paths: Vec<String> = results.iter().map(|r| r.full_path.clone()).collect();
        let bytes = output::plain::render_null(&paths);
        std::io::Write::write_all(&mut std::io::stdout(), &bytes)?;
    } else if cli.plain || cli.gnu {
        let paths: Vec<String> = results.iter().map(|r| r.full_path.clone()).collect();
        println!("{}", output::plain::render_plain(&paths));
    } else if cli.table || is_terminal::is_terminal(std::io::stdout()) {
        let table_results: Vec<output::table::TableResult> = results
            .iter()
            .map(|r| output::table::TableResult {
                full_path: r.full_path.clone(),
                size: r.size,
                mtime: r.mtime,
                mime_type: r.mime_type.clone(),
            })
            .collect();
        let table_output = output::table::render_table(&table_results, cli.icons, use_color);
        output::pager::page_output(&table_output)?;
    } else {
        let paths: Vec<String> = results.iter().map(|r| r.full_path.clone()).collect();
        println!("{}", output::plain::render_plain(&paths));
    }

    std::process::exit(exit_code);
}
