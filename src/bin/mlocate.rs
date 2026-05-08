use clap::CommandFactory;
use clap::Parser;
use mlocate::cli::{SearchCli, Shell};
use mlocate::index::format::IndexReader;
use mlocate::index::search::{search, SearchFilters, SearchOptions};
use mlocate::output;

fn main() -> anyhow::Result<()> {
    install_sigbus_handler();

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

    if !std::path::Path::new(&db_path).exists() {
        eprintln!(
            "mlocate: no index found at {}. Run 'mupdatedb' to create one.",
            db_path
        );
        std::process::exit(2);
    }

    if cli.verbose {
        let lock_path = format!("{}.lock", db_path);
        if std::path::Path::new(&lock_path).exists() {
            eprintln!("mlocate: index {} is currently being updated (lock file present). Results may be stale.", db_path);
        }
    }

    let file = match std::fs::File::open(&db_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("mlocate: cannot open index at {}: {}", db_path, e);
            std::process::exit(2);
        }
    };

    // SAFETY: The mmap is backed by a file that may be truncated externally.
    // A SIGBUS handler is installed (install_sigbus_handler) to catch this.
    // The old inode persists as long as the file handle is open.
    let mmap = match unsafe { memmap2::Mmap::map(&file) } {
        Ok(m) => m,
        Err(e) => {
            eprintln!("mlocate: cannot memory-map index at {}: {}", db_path, e);
            std::process::exit(2);
        }
    };

    let reader = match IndexReader::new(&mmap) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("mlocate: the index at {} appears corrupt or from an incompatible version. Run 'mupdatedb' to rebuild. Details: {}", db_path, e);
            std::process::exit(2);
        }
    };

    if cli.stats {
        if !cli.patterns.is_empty() && cli.verbose {
            eprintln!("mlocate: --statistics ignores search patterns.");
        }
        let file_count = reader.num_files() as i64;
        let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
        let last_idx = match reader.config() {
            Ok(config) => chrono::DateTime::from_timestamp(config.timestamp, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string()),
            Err(_) => "unknown".to_string(),
        };
        let trigram_stats = reader.trigram_stats();
        let json = output::json::render_json_schema(
            &db_path,
            db_size,
            file_count,
            &last_idx,
            &trigram_stats,
        );
        println!("{}", json);
        return Ok(());
    }

    if cli.patterns.is_empty()
        && cli.size.is_none()
        && cli.modified.is_none()
        && cli.mime_type.is_none()
    {
        eprintln!("mlocate: a search pattern is required. Usage: mlocate [OPTIONS] <pattern>");
        std::process::exit(2);
    }

    let size_filter = cli
        .size
        .as_deref()
        .map(mlocate::filter::parse_size)
        .transpose()?;
    let modified_filter = cli
        .modified
        .as_deref()
        .map(mlocate::filter::parse_modified)
        .transpose()?;
    let mime_filter = cli
        .mime_type
        .as_deref()
        .map(mlocate::filter::parse_mime_type)
        .transpose()?;

    let options = SearchOptions {
        ignore_case: cli.ignore_case,
        basename: cli.basename,
        regex: cli.regex,
        existing: cli.existing,
        limit: cli.limit,
    };

    let filters = SearchFilters {
        size: size_filter,
        modified: modified_filter,
        mime: mime_filter,
    };

    let search_results = match search(&reader, &cli.patterns, &options, &filters) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("mlocate: {}", e);
            std::process::exit(2);
        }
    };

    if cli.count {
        let mut count: i64 = 0;
        for result in search_results {
            match result {
                Ok(_) => count += 1,
                Err(e) => {
                    eprintln!("mlocate: {}", e);
                }
            }
        }
        if cli.json {
            println!("{}", output::json::render_json_count(count));
        } else {
            println!("{}", count);
        }
        std::process::exit(if count == 0 { 1 } else { 0 });
    }

    let use_color = match cli.color {
        mlocate::cli::ColorMode::Always => {
            colored::control::set_override(true);
            true
        }
        mlocate::cli::ColorMode::Never => {
            colored::control::set_override(false);
            false
        }
        mlocate::cli::ColorMode::Auto => is_terminal::is_terminal(std::io::stdout()),
    };

    if cli.json {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let mut first = true;
        let mut has_results = false;
        write!(handle, "[").unwrap();
        for result in search_results {
            match result {
                Ok(entry) => {
                    let je = output::json::JsonFileEntry {
                        path: entry.full_path,
                        size: entry.size,
                        mtime: entry.mtime,
                        mode: entry.mode,
                        mime_type: entry.mime_type,
                    };
                    if !first {
                        write!(handle, ",").unwrap();
                    }
                    write!(
                        handle,
                        "\n {}",
                        serde_json::to_string(&je).unwrap_or_default()
                    )
                    .unwrap();
                    first = false;
                    has_results = true;
                }
                Err(e) => eprintln!("mlocate: {}", e),
            }
        }
        if has_results {
            writeln!(handle, "\n]").unwrap();
        } else {
            writeln!(handle, "]").unwrap();
        }
        std::process::exit(if has_results { 0 } else { 1 });
    } else if cli.null {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let mut has_results = false;
        for result in search_results {
            match result {
                Ok(entry) => {
                    handle.write_all(entry.full_path.as_bytes()).unwrap();
                    handle.write_all(&[0u8]).unwrap();
                    has_results = true;
                }
                Err(e) => eprintln!("mlocate: {}", e),
            }
        }
        std::process::exit(if has_results { 0 } else { 1 });
    } else if cli.plain {
        let mut has_results = false;
        for result in search_results {
            match result {
                Ok(entry) => {
                    println!("{}", entry.full_path);
                    has_results = true;
                }
                Err(e) => eprintln!("mlocate: {}", e),
            }
        }
        std::process::exit(if has_results { 0 } else { 1 });
    } else if cli.table || is_terminal::is_terminal(std::io::stdout()) {
        let mut table_results: Vec<output::table::TableResult> = Vec::new();
        for result in search_results {
            match result {
                Ok(entry) => {
                    table_results.push(output::table::TableResult {
                        full_path: entry.full_path,
                        size: entry.size,
                        mtime: entry.mtime,
                        mime_type: entry.mime_type,
                    });
                }
                Err(e) => eprintln!("mlocate: {}", e),
            }
        }
        let exit_code = if table_results.is_empty() { 1 } else { 0 };
        let table_output = output::table::render_table(&table_results, cli.icons, use_color);
        output::pager::page_output(&table_output)?;
        std::process::exit(exit_code);
    } else {
        let mut has_results = false;
        for result in search_results {
            match result {
                Ok(entry) => {
                    println!("{}", entry.full_path);
                    has_results = true;
                }
                Err(e) => eprintln!("mlocate: {}", e),
            }
        }
        std::process::exit(if has_results { 0 } else { 1 });
    }
}

fn install_sigbus_handler() {
    unsafe {
        let _ = signal_hook::low_level::register(signal_hook::consts::SIGBUS, || {
            eprintln!(
                "mlocate: the index file was modified during search. Run 'mupdatedb' and retry."
            );
            std::process::exit(2);
        });
    }
}
