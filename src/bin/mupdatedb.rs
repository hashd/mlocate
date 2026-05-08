use clap::Parser;
use mlocate::cli::UpdateCli;
use mlocate::pipeline;
use mlocate::index::format::IndexConfig;
use mlocate::index::build::{build_index, build_index_incremental};
use mlocate::index::format::{IndexReader, DirTableEntry};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

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

fn acquire_lock(db_path: &str) -> anyhow::Result<()> {
    let lock_path = format!("{}.lock", db_path);
    if std::path::Path::new(&lock_path).exists() {
        anyhow::bail!("Index is locked at {}. If no other mupdatedb is running, delete this file.", lock_path);
    }
    std::fs::write(&lock_path, std::process::id().to_string())?;
    Ok(())
}

fn release_lock(db_path: &str) {
    let lock_path = format!("{}.lock", db_path);
    let _ = std::fs::remove_file(&lock_path);
}

fn cleanup_on_interrupt(db_path: &std::path::Path) {
    let tmp_path = db_path.with_extension("db.tmp");
    let _ = std::fs::remove_file(&tmp_path);
    let lock_path = format!("{}.lock", db_path.display());
    let _ = std::fs::remove_file(&lock_path);
}

fn main() -> anyhow::Result<()> {
    let cli = UpdateCli::parse();

    if cli.version {
        println!("mupdatedb {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if cli.args.install_cron {
        mlocate::cron::install()?;
        return Ok(());
    }

    ctrlc::set_handler(|| {
        INTERRUPTED.store(true, Ordering::SeqCst);
    }).expect("Failed to set SIGINT handler");

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
    let _ = mlocate::platform::cleanup_stale_tmp(&final_db);

    acquire_lock(&final_db)?;

    let parallel = cli.args.parallel.unwrap_or_else(mlocate::platform::default_parallel);

    let (crawl_tx, crawl_rx, extract_tx, extract_rx) =
        pipeline::create_channels(parallel, 1000);

    let crawl_stats = Arc::new(mlocate::crawl::WalkStats::default());

    let (ticker_running, ticker_handle) = start_progress_ticker(crawl_stats.clone(), cli.args.quiet);

    let mut extractor_handles = Vec::new();
    for _ in 0..parallel {
        let rx = crawl_rx.clone();
        let tx = extract_tx.clone();
        let skip_magic = cli.args.no_magic_mime;
        let handle = std::thread::spawn(move || {
            pipeline::run_extractor(rx, tx, skip_magic);
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
        let mut extracted_count = 0u64;
        for _ in extract_rx {
            extracted_count += 1;
        }
        crawl_handle.join().ok();
        for h in extractor_handles {
            h.join().ok();
        }
        stop_progress_ticker(ticker_running, ticker_handle);
        release_lock(&final_db);
        if !cli.args.quiet {
            let scanned = crawl_stats.files_scanned.load(Ordering::Relaxed);
            eprintln!(
                "Dry run completed: {} files scanned, {} entries extracted.\nPaths: {:?}\nPrune: {:?}",
                scanned, extracted_count, localpaths, prunepaths,
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

    let use_incremental = cli.args.incremental && !cli.args.force;

    let build_result = if use_incremental {
        build_incremental(
            extract_rx,
            &final_db,
            &final_db_path,
            config,
        )
    } else {
        let build_stats = Arc::new(mlocate::crawl::WalkStats::default());
        build_index(
            extract_rx,
            &final_db_path,
            config,
            build_stats.clone(),
        ).map_err(Into::into)
    };

    crawl_handle.join().ok();
    for h in extractor_handles {
        h.join().ok();
    }

    stop_progress_ticker(ticker_running, ticker_handle);

    if INTERRUPTED.load(Ordering::SeqCst) {
        cleanup_on_interrupt(&final_db_path);
        eprintln!("\nInterrupted. Temp files cleaned up.");
        std::process::exit(1);
    }

    build_result?;
    release_lock(&final_db);

    if !cli.args.quiet {
        let db_size = std::fs::metadata(&final_db)
            .map(|m| m.len())
            .unwrap_or(0);

        let file_count = match std::fs::File::open(&final_db) {
            Ok(file) => {
                // SAFETY: The mmap is backed by a file that may be truncated externally.
                // The old inode persists as long as the file handle is open.
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

fn build_incremental(
    rx: crossbeam_channel::Receiver<mlocate::pipeline::FileEntry>,
    old_db_path: &str,
    new_db_path: &std::path::Path,
    config: IndexConfig,
) -> anyhow::Result<()> {
    if !std::path::Path::new(old_db_path).exists() {
        eprintln!("Warning: No existing index for incremental update. Performing full rebuild.");
        let stats = Arc::new(mlocate::crawl::WalkStats::default());
        return build_index(rx, new_db_path, config, stats).map_err(Into::into);
    }

    let file = std::fs::File::open(old_db_path)?;
    // SAFETY: The mmap is backed by a file that may be truncated externally.
    // The old inode persists as long as the file handle is open.
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let reader = IndexReader::new(&mmap)
        .map_err(|e| anyhow::anyhow!("Failed to read old index: {}", e))?;

    if !reader.has_feature(mlocate::index::format::FEATURE_DIR_TABLE) {
        eprintln!("Warning: Existing index lacks directory table for incremental update. Performing full rebuild.");
        drop(mmap);
        let stats = Arc::new(mlocate::crawl::WalkStats::default());
        return build_index(rx, new_db_path, config, stats).map_err(Into::into);
    }

    let dirs: Vec<DirTableEntry> = reader.dir_entries_for_prefix("/");
    let stats = Arc::new(mlocate::crawl::WalkStats::default());
    build_index_incremental(rx, new_db_path, &reader, config, stats, &dirs).map_err(Into::into)
}
