use clap::Parser;
use mlocate::cli::UpdateCli;
use mlocate::pipeline;
use mlocate::index::format::IndexConfig;
use mlocate::index::build::build_index;
use mlocate::index::format::IndexReader;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

fn start_progress_ticker(stats: Arc<mlocate::crawl::WalkStats>, quiet: bool) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let handle = std::thread::spawn(move || {
        let start = Instant::now();
        while r.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if quiet {
                continue;
            }
            let scanned = stats.files_scanned.load(Ordering::Relaxed);
            let added = stats.files_added.load(Ordering::Relaxed);
            let skipped = stats.dirs_skipped.load(Ordering::Relaxed);
            let elapsed = start.elapsed().as_secs_f64();
            let rate = if elapsed > 0.0 { scanned as f64 / elapsed } else { 0.0 };
            if is_terminal::is_terminal(std::io::stderr()) {
                eprint!(
                    "\r\x1b[KScanned: {} | Added: {} | Skipped dirs: {} | {:.0} files/s",
                    scanned, added, skipped, rate
                );
            } else if scanned > 0 && scanned.is_multiple_of(10000) {
                eprintln!(
                    "Scanned: {} | Added: {} | Skipped dirs: {} | {:.0} files/s",
                    scanned, added, skipped, rate
                );
            }
        }
        if !quiet && is_terminal::is_terminal(std::io::stderr()) {
            eprintln!();
        }
    });
    (running, handle)
}

fn stop_progress_ticker(running: Arc<AtomicBool>, handle: std::thread::JoinHandle<()>) {
    running.store(false, Ordering::Relaxed);
    handle.join().ok();
}

fn main() -> anyhow::Result<()> {
    let cli = UpdateCli::parse();

    if cli.args.install_cron {
        mlocate::cron::install()?;
        return Ok(());
    }

    if cli.args.incremental {
        anyhow::bail!("--incremental is not supported in this version. Full rebuilds are used because they are now fast enough (the DuckDB bottleneck is gone).");
    }

    let localpaths = if cli.args.localpaths.is_empty() {
        mlocate::platform::path::default_localpaths()
    } else {
        cli.args.localpaths.clone()
    };

    let prunepaths = if cli.args.prunepaths.is_empty() {
        mlocate::platform::path::default_prunepaths()
    } else {
        cli.args.prunepaths.clone()
    };

    let db_path = cli.args.database.as_deref();
    let final_db = mlocate::platform::db_path(db_path);
    let final_db_path = std::path::PathBuf::from(&final_db);

    mlocate::platform::ensure_cache_dir(&final_db)?;

    let db_dir = final_db_path.parent().and_then(|p| p.to_str()).unwrap_or(".");
    let _ = mlocate::platform::cleanup_stale_tmp(db_dir);

    let parallel = cli.args.parallel.unwrap_or_else(mlocate::platform::default_parallel);

    let (crawl_tx, crawl_rx, extract_tx, extract_rx) =
        pipeline::create_channels(parallel, 1000);

    let crawl_stats = Arc::new(mlocate::crawl::WalkStats::default());

    let (ticker_running, ticker_handle) = start_progress_ticker(crawl_stats.clone(), cli.args.quiet);

    let mut extractor_handles = Vec::new();
    for _ in 0..parallel {
        let rx = crawl_rx.clone();
        let tx = extract_tx.clone();
        let handle = std::thread::spawn(move || {
            pipeline::run_extractor(rx, tx, Arc::new(Default::default()));
        });
        extractor_handles.push(handle);
    }
    drop(crawl_rx);
    drop(extract_tx);

    let paths = localpaths.clone();
    let prunepaths_clone = prunepaths.clone();
    let cs = crawl_stats.clone();
    let q = cli.args.quiet;
    let crawl_handle = std::thread::spawn(move || {
        mlocate::crawl::walk_paths(paths, prunepaths_clone, crawl_tx, cs, q);
    });

    let is_dry_run = cli.args.dry_run;

    if is_dry_run {
        for _ in extract_rx {}
        crawl_handle.join().ok();
        for h in extractor_handles {
            h.join().ok();
        }
        stop_progress_ticker(ticker_running, ticker_handle);
        if !cli.args.quiet {
            let scanned = crawl_stats.files_scanned.load(Ordering::Relaxed);
            eprintln!(
                "Dry run completed: {} files would be indexed.\nPaths: {:?}\nPrune: {:?}",
                scanned, localpaths, prunepaths,
            );
        }
        return Ok(());
    }

    let config = IndexConfig {
        indexed_paths: localpaths.clone(),
        pruned_paths: prunepaths.clone(),
        timestamp: chrono::Utc::now().timestamp(),
        hostname: hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        total_bytes_indexed: 0,
        mlocate_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let build_stats = Arc::new(mlocate::crawl::WalkStats::default());
    build_index(
        extract_rx,
        &final_db_path,
        config,
        build_stats.clone(),
    )?;

    crawl_handle.join().ok();
    for h in extractor_handles {
        h.join().ok();
    }

    stop_progress_ticker(ticker_running, ticker_handle);

    if !cli.args.quiet {
        let db_size = std::fs::metadata(&final_db)
            .map(|m| m.len())
            .unwrap_or(0);

        let file_count = match std::fs::File::open(&final_db) {
            Ok(file) => {
                match unsafe { memmap2::Mmap::map(&file) } {
                    Ok(mmap) => {
                        match IndexReader::new(&mmap) {
                            Ok(reader) => reader.num_files(),
                            Err(_) => 0,
                        }
                    }
                    Err(_) => 0,
                }
            }
            Err(_) => 0,
        };

        let skipped = crawl_stats.dirs_skipped.load(Ordering::Relaxed);
        let denied = crawl_stats.permission_denied.load(Ordering::Relaxed);

        eprintln!(
            "Indexed {} files.\nDatabase: {} ({:.1} MB)\nSkipped: {} dirs, {} permission denied",
            file_count,
            final_db,
            db_size as f64 / 1_000_000.0,
            skipped,
            denied,
        );
    }

    Ok(())
}
