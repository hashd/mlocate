use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct Progress {
    pub files_scanned: Arc<AtomicUsize>,
    pub files_added: Arc<AtomicUsize>,
    pub dirs_skipped: Arc<AtomicUsize>,
    pub running: Arc<AtomicBool>,
    pub quiet: bool,
    start: Instant,
}

impl Progress {
    pub fn new(quiet: bool) -> Self {
        Progress {
            files_scanned: Arc::new(AtomicUsize::new(0)),
            files_added: Arc::new(AtomicUsize::new(0)),
            dirs_skipped: Arc::new(AtomicUsize::new(0)),
            running: Arc::new(AtomicBool::new(true)),
            quiet,
            start: Instant::now(),
        }
    }

    pub fn tick(&self) {
        if self.quiet {
            return;
        }
        let scanned = self.files_scanned.load(Ordering::Relaxed);
        let added = self.files_added.load(Ordering::Relaxed);
        let skipped = self.dirs_skipped.load(Ordering::Relaxed);
        let elapsed = self.start.elapsed().as_secs_f64();
        let rate = if elapsed > 0.0 { scanned as f64 / elapsed } else { 0.0 };

        if is_terminal::is_terminal(&std::io::stderr()) {
            eprint!(
                "\r\x1b[KScanned: {} | Added: {} | Skipped dirs: {} | {:.0} files/s",
                scanned, added, skipped, rate
            );
        } else {
            if scanned % 10000 == 0 {
                eprintln!(
                    "Scanned: {} | Added: {} | Skipped dirs: {} | {:.0} files/s",
                    scanned, added, skipped, rate
                );
            }
        }
    }

    pub fn finish(&self) {
        self.running.store(false, Ordering::Relaxed);
        if !self.quiet && is_terminal::is_terminal(&std::io::stderr()) {
            eprintln!();
        }
    }
}
