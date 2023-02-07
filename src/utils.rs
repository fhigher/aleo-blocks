use backoff::{future::retry, ExponentialBackoff, Error};
use log::{info, error};
use colored::Colorize;
use core::ops::Range;
use futures::Future;
use std::time::{Duration, Instant};
use anyhow::anyhow;
use tokio::runtime::{self, Runtime};
use std::fs::{OpenOptions, File};
use std::path::PathBuf;

pub fn backoffset() -> ExponentialBackoff {
    ExponentialBackoff {
        max_interval: Duration::from_secs(15),
        max_elapsed_time: Some(Duration::from_secs(60)),
        ..Default::default()
    }
}

pub fn from_reqwest_err(err: reqwest::Error) -> backoff::Error<reqwest::Error> {
    error!("access aleo api: {err}; retrying...");
    Error::Transient { err: err.into(), retry_after: None }
}

/// Logs the progress of the sync.
pub fn log_progress(
    timer: Instant,
    current_index: u32,
    cdn_range: &Range<u32>,
    object_name: &str,
    batch_request: u32,
) {
    // Prepare the CDN start and end heights.
    let cdn_start = cdn_range.start;
    let cdn_end = cdn_range.end;
    // Compute the percentage completed.
    let percentage = current_index * 100 / cdn_end;
    // Compute the number of files processed so far.
    let num_files_done = 1 + (current_index - cdn_start) / batch_request;
    // Compute the number of files remaining.
    let num_files_remaining = 1 + (cdn_end - current_index) / batch_request;
    // Compute the milliseconds per file.
    let millis_per_file = timer.elapsed().as_millis() / num_files_done as u128;
    // Compute the heuristic slowdown factor (in millis).
    let slowdown = 100 * num_files_remaining as u128;
    // Compute the time remaining (in millis).
    let time_remaining = num_files_remaining as u128 * millis_per_file + slowdown;
    // Prepare the estimate message (in secs).
    let estimate = format!("(est. {} minutes remaining)", time_remaining / (60 * 1000));
    // Log the progress.
    info!("synced up to {object_name} {current_index} of {cdn_end} - {percentage}% complete {}", estimate.dimmed());
}

/// Executes the given closure, with a backoff policy, and returns the result.
pub(crate) async fn handle_dispatch_error<'a, T, F>(func: impl Fn() -> F + 'a) -> anyhow::Result<T>
where
    F: Future<Output = Result<T, anyhow::Error>>,
{
    
    fn from_anyhow_err(err: anyhow::Error) -> backoff::Error<anyhow::Error> {
        if let Ok(err) = err.downcast::<reqwest::Error>() {
            error!("server error: {err}; retrying...");
            Error::Transient { err: err.into(), retry_after: None }
        } else {
            Error::Transient { err: anyhow!("block parse error"), retry_after: None }
        }
    }

    retry(backoffset(), || async { func().await.map_err(from_anyhow_err) }).await
}

 /// Returns a runtime for the node.
 pub fn runtime() -> Runtime {
    let (num_tokio_worker_threads, max_tokio_blocking_threads) = (num_cpus::get(), 512);

    // Initialize the runtime configuration.
    runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .worker_threads(num_tokio_worker_threads)
        .max_blocking_threads(max_tokio_blocking_threads)
        .build()
        .expect("Failed to initialize a runtime for the router")
}

pub fn open_file(filepath: String) -> File {
    let path: PathBuf = PathBuf::from(filepath);
    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path)
                        .unwrap();
    file.set_len(4).unwrap();
    file
}