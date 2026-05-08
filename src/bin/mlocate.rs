use clap::CommandFactory;
use clap::Parser;
use mlocate::cli::{SearchCli, Shell};
use mlocate::index::search::{search, SearchOptions, SearchFilters};
use mlocate::index::format::IndexReader;
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
        eprintln!("mlocate: no index found at {}. Run 'mupdatedb' to create one.", db_path);
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
            &db_path, db_size, file_count, &last_idx, &trigram_stats,
        );
        println!("{}", json);
        return Ok(());
    }

    if cli.patterns.is_empty() {
        eprintln!("mlocate: a search pattern is required. Usage: mlocate [OPTIONS] <pattern>");
        std::process::exit(2);
    }

    let size_filter = cli.size.as_deref().map(mlocate::filter::parse_size).transpose()?;
    let modified_filter = cli.modified.as_deref().map(mlocate::filter::parse_modified).transpose()?;
    let mime_filter = cli.mime_type.as_deref().map(mlocate::filter::parse_mime_type).transpose()?;

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

    let mut results: Vec<mlocate::index::format::DiskFileEntry> = Vec::new();
    for result in search_results {
        match result {
            Ok(entry) => {
                results.push(entry);
            }
            Err(e) => {
                eprintln!("mlocate: {}", e);
            }
        }
    }

    if cli.count {
        let count = results.len() as i64;
        if cli.json {
            println!("{}", output::json::render_json_count(count));
        } else {
            println!("{}", count);
        }
        std::process::exit(if count == 0 { 1 } else { 0 });
    }

    let exit_code = if results.is_empty() { 1 } else { 0 };

    let use_color = match cli.color {
        mlocate::cli::ColorMode::Always => {
            colored::control::set_override(true);
            true
        }
        mlocate::cli::ColorMode::Never => {
            colored::control::set_override(false);
            false
        }
        mlocate::cli::ColorMode::Auto => {
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
    } else if cli.plain {
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
    } else if cli.gnu {
        let paths: Vec<String> = results.iter().map(|r| r.full_path.clone()).collect();
        println!("{}", output::plain::render_plain(&paths));
    } else {
        let paths: Vec<String> = results.iter().map(|r| r.full_path.clone()).collect();
        println!("{}", output::plain::render_plain(&paths));
    }

    std::process::exit(exit_code);
}

fn install_sigbus_handler() {
    unsafe {
        let _ = signal_hook::low_level::register(signal_hook::consts::SIGBUS, || {
            eprintln!("mlocate: the index file was modified during search. Run 'mupdatedb' and retry.");
            std::process::exit(2);
        });
    }
}
