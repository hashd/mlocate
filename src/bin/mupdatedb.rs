use clap::Parser;
use mlocate::cli::UpdateCli;
use mlocate::pipeline;
use mlocate::progress::Progress;
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    let cli = UpdateCli::parse();

    if cli.args.install_cron {
        eprintln!("--install-cron not yet implemented");
        return Ok(());
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
    let tmp_db = mlocate::platform::tmp_db_path(db_path);

    mlocate::platform::ensure_cache_dir(&final_db)?;
    mlocate::db::cleanup_stale_tmp(
        std::path::Path::new(&final_db)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("."),
    )?;

    let parallel = cli.args.parallel.unwrap_or_else(mlocate::platform::default_parallel);
    let _progress = Arc::new(Progress::new(cli.args.quiet));

    let (crawl_tx, crawl_rx, extract_tx, extract_rx) =
        pipeline::create_channels(parallel, 1000);

    // Spawn extractor workers
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

    // Start crawling
    let paths = localpaths.clone();
    let prunepaths_clone = prunepaths.clone();
    let crawl_handle = std::thread::spawn(move || {
        let stats = Arc::new(Default::default());
        mlocate::crawl::walk_paths(paths, prunepaths_clone, crawl_tx, stats, true);
    });

    let is_dry_run = cli.args.dry_run;

    if is_dry_run {
        // Drain channel and discard — no DB writes
        for _ in extract_rx {}
        crawl_handle.join().ok();
        for h in extractor_handles {
            h.join().ok();
        }
        if !cli.args.quiet {
            eprintln!(
                "Dry run completed.\nPaths: {:?}\nPrune: {:?}",
                localpaths, prunepaths,
            );
        }
        return Ok(());
    }

    // Run batcher
    let stats = Arc::new(mlocate::crawl::WalkStats::default());
    pipeline::run_batcher(
        extract_rx,
        &tmp_db,
        cli.args.incremental,
        cli.args.force,
        stats.clone(),
    )?;

    // Wait for crawler and extractors
    crawl_handle.join().ok();
    for h in extractor_handles {
        h.join().ok();
    }

    // Atomic swap
    mlocate::db::atomic_swap(&tmp_db, &final_db)?;

    // Print summary
    if !cli.args.quiet {
        eprintln!("Database created at {}", final_db);
    }

    Ok(())
}
