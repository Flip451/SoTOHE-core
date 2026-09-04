use std::io::{Error, ErrorKind};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use super::{RustdocInputFingerprintError, io_error};

pub(super) struct FingerprintDeadline {
    started: Instant,
    maximum: Duration,
}

type FingerprintIoJob = Box<dyn FnOnce() + Send + 'static>;

const FINGERPRINT_IO_QUEUE_CAPACITY: usize = 1;
const FINGERPRINT_IO_QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_FINGERPRINT_IO_WORKER_GENERATIONS: usize = 16;

struct FingerprintIoWorker {
    generation: usize,
    sender: SyncSender<FingerprintIoJob>,
}

struct FingerprintIoExecutor {
    generations: usize,
    worker: Option<FingerprintIoWorker>,
}

struct FingerprintIoWorkerLease {
    generation: usize,
    sender: SyncSender<FingerprintIoJob>,
}

static FINGERPRINT_IO_EXECUTOR: OnceLock<Mutex<FingerprintIoExecutor>> = OnceLock::new();

fn spawn_fingerprint_io_worker(generation: usize) -> Result<SyncSender<FingerprintIoJob>, Error> {
    let (sender, receiver) = mpsc::sync_channel(FINGERPRINT_IO_QUEUE_CAPACITY);
    thread::Builder::new()
        .name(format!("sotp-fingerprint-io-worker-{generation}"))
        .spawn(move || {
            while let Ok(job) = receiver.recv() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            }
        })
        .map(|_| sender)
        .map_err(|error| Error::other(format!("cannot spawn fingerprint I/O worker: {error}")))
}

fn fingerprint_io_worker() -> Result<FingerprintIoWorkerLease, Error> {
    let executor = FINGERPRINT_IO_EXECUTOR
        .get_or_init(|| Mutex::new(FingerprintIoExecutor { generations: 0, worker: None }));
    let mut executor = match executor.lock() {
        Ok(executor) => executor,
        Err(poisoned) => poisoned.into_inner(),
    };
    if executor.worker.is_none() {
        if executor.generations >= MAX_FINGERPRINT_IO_WORKER_GENERATIONS {
            return Err(Error::new(
                ErrorKind::TimedOut,
                "fingerprint I/O worker restart budget exhausted",
            ));
        }
        let generation = executor.generations.saturating_add(1);
        let sender = spawn_fingerprint_io_worker(generation)?;
        executor.generations = generation;
        executor.worker = Some(FingerprintIoWorker { generation, sender });
    }
    let worker = match executor.worker.as_ref() {
        Some(worker) => worker,
        None => return Err(Error::other("fingerprint I/O worker is unavailable")),
    };
    Ok(FingerprintIoWorkerLease { generation: worker.generation, sender: worker.sender.clone() })
}

fn retire_fingerprint_io_worker(generation: usize) {
    let Some(executor) = FINGERPRINT_IO_EXECUTOR.get() else {
        return;
    };
    let mut executor = match executor.lock() {
        Ok(executor) => executor,
        Err(poisoned) => poisoned.into_inner(),
    };
    let is_current = executor.worker.as_ref().is_some_and(|worker| worker.generation == generation);
    if !is_current {
        return;
    }
    executor.worker = None;
    if executor.generations >= MAX_FINGERPRINT_IO_WORKER_GENERATIONS {
        return;
    }
    let next_generation = executor.generations.saturating_add(1);
    executor.generations = next_generation;
    if let Ok(sender) = spawn_fingerprint_io_worker(next_generation) {
        executor.worker = Some(FingerprintIoWorker { generation: next_generation, sender });
    }
}

impl FingerprintDeadline {
    pub(super) fn new(maximum: Duration) -> Self {
        Self { started: Instant::now(), maximum }
    }

    pub(super) fn check(
        &self,
        operation: &'static str,
    ) -> Result<(), RustdocInputFingerprintError> {
        if self.started.elapsed() >= self.maximum {
            Err(RustdocInputFingerprintError::TimedOut { operation, maximum: self.maximum })
        } else {
            Ok(())
        }
    }

    pub(super) fn remaining(
        &self,
        operation: &'static str,
    ) -> Result<Duration, RustdocInputFingerprintError> {
        let elapsed = self.started.elapsed();
        self.maximum
            .checked_sub(elapsed)
            .filter(|remaining| *remaining > Duration::ZERO)
            .ok_or(RustdocInputFingerprintError::TimedOut { operation, maximum: self.maximum })
    }

    /// Runs one potentially blocking filesystem operation behind the fingerprint deadline.
    ///
    /// The standard filesystem API has no cancellation primitive. Reusable worker generations
    /// and a one-slot queue keep blocked operations bounded; a timed-out generation is retired
    /// and replaced until the explicit restart budget is exhausted, after which capture fails
    /// closed rather than retaining unbounded workers.
    pub(super) fn run_io<T, F>(
        &self,
        operation: &'static str,
        path: std::path::PathBuf,
        operation_fn: F,
    ) -> Result<T, RustdocInputFingerprintError>
    where
        T: Send + 'static,
        F: FnOnce() -> std::io::Result<T> + Send + 'static,
    {
        self.check(operation)?;
        let worker = fingerprint_io_worker().map_err(|error| {
            if error.kind() == ErrorKind::TimedOut {
                RustdocInputFingerprintError::TimedOut { operation, maximum: self.maximum }
            } else {
                io_error(&path, error)
            }
        })?;
        let (result_sender, receiver) = mpsc::sync_channel(1);
        let job: FingerprintIoJob = Box::new(move || {
            let _ = result_sender.send(operation_fn());
        });
        let mut job = Some(job);
        loop {
            let queued_job = match job.take() {
                Some(job) => job,
                None => {
                    return Err(io_error(
                        &path,
                        Error::other("fingerprint I/O job was lost before scheduling"),
                    ));
                }
            };
            match worker.sender.try_send(queued_job) {
                Ok(()) => break,
                Err(TrySendError::Full(queued_job)) => {
                    job = Some(queued_job);
                    let remaining = match self.remaining(operation) {
                        Ok(remaining) => remaining,
                        Err(error) => {
                            retire_fingerprint_io_worker(worker.generation);
                            return Err(error);
                        }
                    };
                    thread::sleep(FINGERPRINT_IO_QUEUE_POLL_INTERVAL.min(remaining));
                }
                Err(TrySendError::Disconnected(_)) => {
                    retire_fingerprint_io_worker(worker.generation);
                    return Err(io_error(
                        &path,
                        Error::other("fingerprint I/O worker disconnected"),
                    ));
                }
            }
        }

        let remaining = match self.remaining(operation) {
            Ok(remaining) => remaining,
            Err(error) => {
                retire_fingerprint_io_worker(worker.generation);
                return Err(error);
            }
        };
        match receiver.recv_timeout(remaining) {
            Ok(Ok(value)) => {
                self.check(operation)?;
                Ok(value)
            }
            Ok(Err(error)) => Err(io_error(&path, error)),
            Err(RecvTimeoutError::Timeout) => {
                retire_fingerprint_io_worker(worker.generation);
                Err(RustdocInputFingerprintError::TimedOut { operation, maximum: self.maximum })
            }
            Err(RecvTimeoutError::Disconnected) => {
                retire_fingerprint_io_worker(worker.generation);
                Err(io_error(
                    &path,
                    Error::other("bounded filesystem operation worker disconnected"),
                ))
            }
        }
    }
}
