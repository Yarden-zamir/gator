//! XDG base-directory helpers plus atomic JSON persistence and a small
//! subprocess worker pool, shared by every gator app's caches and state.

use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::VecDeque,
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Directory for durable user state (`$XDG_STATE_HOME/<app>` or
/// `~/.local/state/<app>`). Errors when neither env var is available.
pub fn state_dir(app: &str) -> io::Result<PathBuf> {
    let root = env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".local").join("state"))
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Cannot locate {app} state: XDG_STATE_HOME and HOME are unset"),
            )
        })?;
    Ok(root.join(app))
}

/// File inside [`state_dir`].
pub fn state_file(app: &str, file_name: &str) -> io::Result<PathBuf> {
    Ok(state_dir(app)?.join(file_name))
}

/// Directory for regenerable caches (`$XDG_CACHE_HOME/<app>`, `~/.cache/<app>`,
/// or a temp dir fallback).
pub fn cache_dir(app: &str) -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(env::temp_dir)
        .join(app)
}

/// File inside [`cache_dir`].
pub fn cache_file(app: &str, file_name: &str) -> PathBuf {
    cache_dir(app).join(file_name)
}

/// Read and deserialize JSON, returning `None` on any error (missing file,
/// unreadable, or malformed). Suited to regenerable caches.
pub fn read_json_opt<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Serialize `value` to JSON and replace `path` atomically: the parent
/// directory is created, bytes are written to a sibling `.tmp` file, flushed to
/// disk, then renamed over the target.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_vec(value).map_err(io::Error::other)?;
    let tmp = suffixed(path, ".tmp");
    let mut file = File::create(&tmp)?;
    file.write_all(&contents)?;
    file.sync_all()?;
    fs::rename(tmp, path)
}

/// Open and exclusively lock a `<path>.lock` sibling file, creating it and its
/// parent directory if needed. The lock is released when the returned handle is
/// dropped. Use to guard read-modify-write of a state file across processes.
pub fn lock_sibling(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lock_path = suffixed(path, ".lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    file.lock()?;
    Ok(file)
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    PathBuf::from(name)
}

/// Current time as whole seconds since the Unix epoch (0 if the clock predates
/// it).
pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Number of workers for `job_count` jobs: available parallelism, capped to the
/// job count and clamped to `[1, 8]`.
pub fn worker_count(job_count: usize) -> usize {
    let parallel = thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(4);
    job_count.min(parallel).clamp(1, 8)
}

/// Run `run_job` over `jobs` on a bounded worker pool, streaming outputs to
/// `batch_tx` in batches. A batch is flushed when it reaches `batch_size` or
/// when `batch_delay` elapses with no new output; a final partial batch is sent
/// before the sender closes.
pub fn spawn_batched_jobs<Job, Output, Run>(
    jobs: Vec<Job>,
    batch_size: usize,
    batch_delay: Duration,
    batch_tx: mpsc::Sender<Vec<Output>>,
    run_job: Run,
) where
    Job: Send + 'static,
    Output: Send + 'static,
    Run: Fn(Job) -> Vec<Output> + Send + Sync + 'static,
{
    let worker_count = worker_count(jobs.len());
    let queue = Arc::new(Mutex::new(VecDeque::from(jobs)));
    let run_job = Arc::new(run_job);
    let (item_tx, item_rx) = mpsc::channel::<Output>();

    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let run_job = Arc::clone(&run_job);
        let item_tx = item_tx.clone();
        thread::spawn(move || loop {
            let job = {
                let mut queue = queue.lock().expect("job queue lock should not be poisoned");
                queue.pop_front()
            };
            let Some(job) = job else {
                break;
            };
            for item in run_job(job) {
                let _ = item_tx.send(item);
            }
        });
    }
    drop(item_tx);

    thread::spawn(move || {
        let mut batch = Vec::new();
        loop {
            match item_rx.recv_timeout(batch_delay) {
                Ok(item) => {
                    batch.push(item);
                    if batch.len() >= batch_size {
                        let _ = batch_tx.send(std::mem::take(&mut batch));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !batch.is_empty() {
                        let _ = batch_tx.send(std::mem::take(&mut batch));
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if !batch.is_empty() {
                        let _ = batch_tx.send(batch);
                    }
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_json_round_trips_and_creates_parents() {
        let dir = env::temp_dir().join(format!("gator-xdg-{}", std::process::id()));
        let path = dir.join("nested").join("value.json");
        let _ = fs::remove_dir_all(&dir);
        write_json_atomic(&path, &vec![1_u32, 2, 3]).expect("write");
        let back: Vec<u32> = read_json_opt(&path).expect("read");
        assert_eq!(back, vec![1, 2, 3]);
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn read_json_opt_is_none_for_missing_or_malformed() {
        let missing = env::temp_dir().join("gator-xdg-does-not-exist.json");
        let _ = fs::remove_file(&missing);
        assert!(read_json_opt::<Vec<u32>>(&missing).is_none());
    }
}
