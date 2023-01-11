use backoff::ExponentialBackoff;
use std::time::Duration;
use backoff::Error;
use log::debug;

pub fn backoffset() -> ExponentialBackoff {
    ExponentialBackoff {
        max_interval: Duration::from_secs(15),
        max_elapsed_time: Some(Duration::from_secs(60)),
        ..Default::default()
    }
}

pub fn from_reqwest_err(err: reqwest::Error) -> backoff::Error<reqwest::Error> {
    debug!("access aleo api: {err}; retrying...");
    Error::Transient { err: err.into(), retry_after: None }
}