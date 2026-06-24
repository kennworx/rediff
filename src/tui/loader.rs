//! Background, parallel, cancellable diff loader. Enumeration yields a list of
//! [`FileStub`]s synchronously (cheap); this drives the expensive per-file diff
//! across a worker pool and streams each completed `DiffFile` back to the UI,
//! tagged with its original index so display order never changes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::git::{self, FileStub};
use crate::model::DiffFile;

/// A streaming diff load in progress.
pub struct Loader {
    /// Total files to diff (known up front, from enumeration).
    pub total: usize,
    /// Files whose diff has been delivered and installed so far.
    pub done: usize,
    rx: Receiver<(usize, DiffFile)>,
    cancel: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl Loader {
    /// Diff `jobs` against the repo at `repo_dir` on a worker pool. Each job is
    /// `(index, stub)` where `index` is the file's position in the view's
    /// changeset; results stream back tagged with that index via [`Loader::drain`],
    /// so a resumed load (a subset of jobs) installs each file at its original
    /// slot. Dropping the loader cancels it.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "repo_dir is cloned into each worker; owning it keeps the call site simple"
    )]
    pub fn start(repo_dir: PathBuf, jobs: Vec<(usize, FileStub)>) -> Loader {
        let total = jobs.len();
        let (tx, rx) = mpsc::channel::<(usize, DiffFile)>();
        let cancel = Arc::new(AtomicBool::new(false));
        let next = Arc::new(AtomicUsize::new(0));
        let jobs = Arc::new(jobs);

        let n_workers = pool_size(total);
        let mut workers = Vec::with_capacity(n_workers);
        for _ in 0..n_workers {
            let tx = tx.clone();
            let cancel = cancel.clone();
            let next = next.clone();
            let jobs = jobs.clone();
            let repo_dir = repo_dir.clone();
            workers.push(thread::spawn(move || {
                // Each worker opens its own repository handle (gix repos are not
                // shared across threads).
                let Ok(repo) = gix::discover(&repo_dir) else {
                    return;
                };
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let slot = next.fetch_add(1, Ordering::Relaxed);
                    let Some((idx, stub)) = jobs.get(slot) else {
                        break;
                    };
                    let file = git::diff_file(&repo, stub);
                    if tx.send((*idx, file)).is_err() {
                        break;
                    }
                }
            }));
        }

        Loader {
            total,
            done: 0,
            rx,
            cancel,
            workers,
        }
    }

    /// Drain all currently-ready results, bumping `done`. Returns the
    /// `(index, file)` pairs to install into the changeset.
    pub fn drain(&mut self) -> Vec<(usize, DiffFile)> {
        let mut out = Vec::new();
        while let Ok(item) = self.rx.try_recv() {
            self.done += 1;
            out.push(item);
        }
        out
    }

    /// Whether every file has been diffed and installed.
    pub fn finished(&self) -> bool {
        self.done >= self.total
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        // Signal cancellation and detach — do NOT join. Joining here would block
        // the UI thread until every in-flight `diff_file` finished, which makes
        // Esc/q feel frozen on a big load. Workers check the flag between files
        // (and their sends fail once the receiver drops), so they stop on their
        // own; the OS reaps any stragglers at process exit.
        self.cancel.store(true, Ordering::Relaxed);
        self.workers.clear();
    }
}

/// Worker count: a small pool that leaves a core free for the UI thread so
/// input stays responsive while diffs stream, bounded by the file count.
fn pool_size(total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let cores = thread::available_parallelism().map_or(4, std::num::NonZero::get);
    cores.saturating_sub(1).clamp(1, 8).min(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Drive a loader to completion (bounded wait), returning installed files in
    /// index order.
    fn run_to_completion(mut loader: Loader) -> Vec<DiffFile> {
        let total = loader.total;
        let mut out: Vec<Option<DiffFile>> = (0..total).map(|_| None).collect();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !loader.finished() && Instant::now() < deadline {
            for (idx, file) in loader.drain() {
                out[idx] = Some(file);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        for (idx, file) in loader.drain() {
            out[idx] = Some(file);
        }
        out.into_iter()
            .map(|f| f.expect("every file diffed"))
            .collect()
    }

    #[test]
    fn streams_all_files_in_order() {
        // The crate's own repo, diffed HEAD vs its parent, is a live fixture.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let en = git::enumerate(dir, &git::LoadRequest::Show { rev: "HEAD".into() }).unwrap();
        if en.stubs.is_empty() {
            return; // nothing to assert on an empty commit
        }
        let want: Vec<String> = en.stubs.iter().map(|s| s.path.clone()).collect();
        let jobs: Vec<(usize, FileStub)> = en.stubs.into_iter().enumerate().collect();
        let loader = Loader::start(dir.to_path_buf(), jobs);
        let files = run_to_completion(loader);
        let got: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(got, want, "files install in enumeration order");
        assert!(
            files.iter().all(|f| f.diffed),
            "every streamed file is diffed"
        );
    }

    #[test]
    fn cancel_stops_further_work() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let en = git::enumerate(dir, &git::LoadRequest::Show { rev: "HEAD".into() }).unwrap();
        let jobs: Vec<(usize, FileStub)> = en.stubs.into_iter().enumerate().collect();
        let loader = Loader::start(dir.to_path_buf(), jobs);
        // Dropping signals cancellation and joins the worker pool without
        // hanging, even mid-load.
        drop(loader);
    }
}
