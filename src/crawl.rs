use dashmap::DashSet;
use ignore::WalkBuilder;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub fn walk_paths(
    localpaths: Vec<String>,
    prunepaths: Vec<String>,
    crawl_tx: crossbeam_channel::Sender<String>,
    quiet: bool,
) -> (usize, usize) {
    let seen_dirs: Arc<DashSet<(u64, u64)>> = Arc::new(DashSet::new());
    let files_found = 0usize;
    let dirs_skipped = 0usize;
    let start = Instant::now();

    for root_path in &localpaths {
        let root = Path::new(root_path);
        if !root.exists() {
            if !quiet {
                eprintln!("Warning: Root path '{}' does not exist, skipping.", root_path);
            }
            continue;
        }

        let canonical_root = match root.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                if !quiet {
                    eprintln!("Warning: Cannot resolve '{}': {}", root_path, e);
                }
                continue;
            }
        };

        let mut builder = WalkBuilder::new(&canonical_root);
        builder
            .hidden(false)
            .git_ignore(true)
            .ignore(true)
            .follow_links(false)
            .threads(std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));

        let _prunes = prunepaths.clone();
        let seen = seen_dirs.clone();

        let walker = builder.build_parallel();

        walker.run(|| {
            let crawl_tx = crawl_tx.clone();
            let seen = seen.clone();
            Box::new(move |result| {
                use ignore::WalkState;

                match result {
                    Ok(entry) => {
                        let path = entry.path();
                        let file_type = match entry.file_type() {
                            Some(ft) => ft,
                            None => match std::fs::symlink_metadata(path) {
                                Ok(m) => m.file_type(),
                                Err(_) => return WalkState::Continue,
                            },
                        };

                        if file_type.is_dir() {
                            if let Ok(meta) = std::fs::metadata(path) {
                                use std::os::unix::fs::MetadataExt;
                                let dev = meta.dev();
                                let ino = meta.ino();
                                if seen.contains(&(dev, ino)) {
                                    return WalkState::Skip;
                                }
                                seen.insert((dev, ino));
                            }
                        }

                        if file_type.is_symlink() {
                            match std::fs::metadata(path) {
                                Ok(meta) => {
                                    if meta.is_dir() {
                                        return WalkState::Continue;
                                    }
                                }
                                Err(_) => return WalkState::Continue,
                            }
                        }

                        if !file_type.is_dir() {
                            if let Some(path_str) = path.to_str() {
                                let _ = crawl_tx.send(path_str.to_string());
                            }
                        }

                        WalkState::Continue
                    }
                    Err(err) => {
                        eprintln!("Warning: {}", err);
                        WalkState::Continue
                    }
                }
            })
        });
    }

    let elapsed = start.elapsed();
    if !quiet {
        eprintln!(
            "Walk completed: {} files in {:.1}s",
            files_found,
            elapsed.as_secs_f64()
        );
    }

    (files_found, dirs_skipped)
}
