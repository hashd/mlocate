use dashmap::DashSet;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Default)]
pub struct WalkStats {
    pub files_scanned: AtomicUsize,
    pub files_added: AtomicUsize,
    pub dirs_skipped: AtomicUsize,
    pub permission_denied: AtomicUsize,
    pub ino_set_size: AtomicUsize,
}

pub fn walk_paths(
    localpaths: Vec<String>,
    prunepaths: Vec<String>,
    crawl_tx: crossbeam_channel::Sender<PathBuf>,
    stats: Arc<WalkStats>,
    quiet: bool,
) {
    let seen_dirs: Arc<DashSet<(u64, u64)>> = Arc::new(DashSet::new());
    let start = Instant::now();

    for root_path in &localpaths {
        let root = Path::new(root_path);
        if !root.exists() {
            if !quiet {
                eprintln!(
                    "Warning: Root path '{}' does not exist, skipping.",
                    root_path
                );
            }
            continue;
        }

        let canonical_root = match std::fs::canonicalize(root) {
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
            .threads(
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4),
            );

        let prunes: Vec<PathBuf> = prunepaths
            .iter()
            .filter_map(|p| std::fs::canonicalize(p).ok())
            .collect();
        let seen = seen_dirs.clone();
        let stats = stats.clone();

        let walker = builder.build_parallel();

        walker.run(|| {
            let crawl_tx = crawl_tx.clone();
            let seen = seen.clone();
            let prunes = prunes.clone();
            let stats = stats.clone();

            Box::new(move |result| {
                use ignore::WalkState;

                match result {
                    Ok(entry) => {
                        let path = entry.path().to_path_buf();
                        stats.files_scanned.fetch_add(1, Ordering::Relaxed);

                        let file_type = match entry.file_type() {
                            Some(ft) => ft,
                            None => return WalkState::Continue,
                        };

                        if file_type.is_dir() {
                            if prunes.iter().any(|p| path.starts_with(p)) {
                                return WalkState::Skip;
                            }

                            match std::fs::metadata(&path) {
                                Ok(meta) => {
                                    use std::os::unix::fs::MetadataExt;
                                    let key = (meta.dev(), meta.ino());
                                    if seen.contains(&key) {
                                        return WalkState::Skip;
                                    }
                                    seen.insert(key);
                                    stats.ino_set_size.store(seen.len(), Ordering::Relaxed);
                                }
                                Err(_) => {
                                    stats.permission_denied.fetch_add(1, Ordering::Relaxed);
                                    return WalkState::Continue;
                                }
                            }
                        }

                        if file_type.is_symlink() {
                            match std::fs::metadata(&path) {
                                Ok(meta) => {
                                    if meta.is_dir() {
                                        return WalkState::Continue;
                                    }
                                }
                                Err(_) => return WalkState::Continue,
                            }
                        }

                        if file_type.is_file() || file_type.is_symlink() {
                            let resolved = if file_type.is_symlink() {
                                match path.canonicalize() {
                                    Ok(c) => c,
                                    Err(_) => return WalkState::Continue,
                                }
                            } else {
                                path
                            };
                            if prunes.iter().any(|p| resolved.starts_with(p)) {
                                return WalkState::Continue;
                            }
                            if let Some(path_str) = resolved.to_str() {
                                let _ = crawl_tx.send(PathBuf::from(path_str));
                                stats.files_added.fetch_add(1, Ordering::Relaxed);
                            }
                        }

                        WalkState::Continue
                    }
                    Err(err) => {
                        if !quiet {
                            eprintln!("Warning: {}", err);
                        }
                        WalkState::Continue
                    }
                }
            })
        });
    }

    let elapsed = start.elapsed();
    if !quiet {
        eprintln!(
            "Walk completed: {} files scanned, {} sent in {:.1}s",
            stats.files_scanned.load(Ordering::Relaxed),
            stats.files_added.load(Ordering::Relaxed),
            elapsed.as_secs_f64()
        );
    }

    drop(crawl_tx);
}
