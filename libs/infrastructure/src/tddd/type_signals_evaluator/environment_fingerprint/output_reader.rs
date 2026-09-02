use std::io::{Error, ErrorKind, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub(super) type NightlyOutputResult = Result<Vec<u8>, Error>;
pub(super) type NightlyOutputSlot = Arc<Mutex<Option<NightlyOutputResult>>>;
type NightlyReaderJob = Box<dyn FnOnce() + Send + 'static>;

const NIGHTLY_READER_QUEUE_CAPACITY: usize = 1;
const NIGHTLY_READER_QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_NIGHTLY_READER_WORKER_GENERATIONS: usize = 16;

// Each stream has one active worker and one queued read at most. A timeout retires the current
// generation and starts a replacement until the explicit generation budget is exhausted, which
// bounds retained pipes and workers even when a descendant keeps an inherited pipe open.

struct NightlyReaderWorker {
    generation: usize,
    sender: SyncSender<NightlyReaderJob>,
}

struct NightlyReaderExecutor {
    generations: usize,
    worker: Option<NightlyReaderWorker>,
}

struct NightlyReaderWorkerLease {
    generation: usize,
    sender: SyncSender<NightlyReaderJob>,
}

#[derive(Clone, Copy)]
pub(super) struct NightlyReaderConfig {
    pub(super) maximum_stream_bytes: usize,
    pub(super) maximum_total_bytes: u64,
    pub(super) execution_deadline: Instant,
}

static NIGHTLY_STDOUT_READER_WORKER: OnceLock<Mutex<NightlyReaderExecutor>> = OnceLock::new();
static NIGHTLY_STDERR_READER_WORKER: OnceLock<Mutex<NightlyReaderExecutor>> = OnceLock::new();

fn spawn_nightly_reader_worker(
    worker_name: &'static str,
    generation: usize,
) -> Result<SyncSender<NightlyReaderJob>, Error> {
    let (sender, receiver) = mpsc::sync_channel(NIGHTLY_READER_QUEUE_CAPACITY);
    thread::Builder::new()
        .name(format!("{worker_name}-{generation}"))
        .spawn(move || {
            while let Ok(job) = receiver.recv() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            }
        })
        .map(|_| sender)
        .map_err(|error| Error::other(format!("cannot spawn {worker_name}: {error}")))
}

fn nightly_reader_worker(
    worker: &'static OnceLock<Mutex<NightlyReaderExecutor>>,
    worker_name: &'static str,
) -> Result<NightlyReaderWorkerLease, Error> {
    let executor =
        worker.get_or_init(|| Mutex::new(NightlyReaderExecutor { generations: 0, worker: None }));
    let mut executor = match executor.lock() {
        Ok(executor) => executor,
        Err(poisoned) => poisoned.into_inner(),
    };
    if executor.worker.is_none() {
        if executor.generations >= MAX_NIGHTLY_READER_WORKER_GENERATIONS {
            return Err(Error::new(
                ErrorKind::TimedOut,
                format!("{worker_name} restart budget exhausted"),
            ));
        }
        let generation = executor.generations.saturating_add(1);
        let sender = spawn_nightly_reader_worker(worker_name, generation)?;
        executor.generations = generation;
        executor.worker = Some(NightlyReaderWorker { generation, sender });
    }
    let worker = match executor.worker.as_ref() {
        Some(worker) => worker,
        None => return Err(Error::other(format!("{worker_name} is unavailable"))),
    };
    Ok(NightlyReaderWorkerLease { generation: worker.generation, sender: worker.sender.clone() })
}

fn retire_nightly_reader_worker(
    worker: &'static OnceLock<Mutex<NightlyReaderExecutor>>,
    worker_name: &'static str,
    generation: usize,
) {
    let Some(executor) = worker.get() else {
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
    if executor.generations >= MAX_NIGHTLY_READER_WORKER_GENERATIONS {
        return;
    }
    let next_generation = executor.generations.saturating_add(1);
    executor.generations = next_generation;
    if let Ok(sender) = spawn_nightly_reader_worker(worker_name, next_generation) {
        executor.worker = Some(NightlyReaderWorker { generation: next_generation, sender });
    }
}

pub(super) fn retire_nightly_reader_workers(stdout_generation: usize, stderr_generation: usize) {
    retire_nightly_reader_worker(
        &NIGHTLY_STDOUT_READER_WORKER,
        "sotp-nightly-stdout-reader",
        stdout_generation,
    );
    retire_nightly_reader_worker(
        &NIGHTLY_STDERR_READER_WORKER,
        "sotp-nightly-stderr-reader",
        stderr_generation,
    );
}

pub(super) fn retire_nightly_stdout_reader(generation: usize) {
    retire_nightly_reader_worker(
        &NIGHTLY_STDOUT_READER_WORKER,
        "sotp-nightly-stdout-reader",
        generation,
    );
}

pub(super) fn spawn_nightly_output_reader(
    pipe: impl Read + Send + 'static,
    result_slot: NightlyOutputSlot,
    shared_total: Arc<AtomicU64>,
    config: NightlyReaderConfig,
    label: &str,
    stream: &'static str,
) -> Result<usize, Error> {
    let (worker_static, worker_name) = match stream {
        "stdout" => (&NIGHTLY_STDOUT_READER_WORKER, "sotp-nightly-stdout-reader"),
        "stderr" => (&NIGHTLY_STDERR_READER_WORKER, "sotp-nightly-stderr-reader"),
        _ => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("unknown nightly output stream: {stream}"),
            ));
        }
    };
    let worker = nightly_reader_worker(worker_static, worker_name)?;
    let reader_label = format!("{label} output reader");
    let output_label = label.to_owned();
    let job: NightlyReaderJob = Box::new(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            collect_nightly_output(
                pipe,
                shared_total,
                config.maximum_stream_bytes,
                config.maximum_total_bytes,
                &output_label,
            )
        }))
        .unwrap_or_else(|_| Err(Error::other(format!("{output_label} output reader panicked"))));
        match result_slot.lock() {
            Ok(mut slot) => *slot = Some(result),
            Err(poisoned) => *poisoned.into_inner() = Some(result),
        }
    });
    let mut job = Some(job);
    loop {
        let queued_job = match job.take() {
            Some(job) => job,
            None => {
                return Err(Error::other(format!("{reader_label} job was lost before scheduling")));
            }
        };
        match worker.sender.try_send(queued_job) {
            Ok(()) => break,
            Err(TrySendError::Full(queued_job)) => {
                job = Some(queued_job);
                let remaining = config.execution_deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    retire_nightly_reader_worker(worker_static, worker_name, worker.generation);
                    return Err(Error::new(
                        ErrorKind::TimedOut,
                        format!("{reader_label} could not be scheduled before execution timeout"),
                    ));
                }
                thread::sleep(NIGHTLY_READER_QUEUE_POLL_INTERVAL.min(remaining));
            }
            Err(TrySendError::Disconnected(_)) => {
                retire_nightly_reader_worker(worker_static, worker_name, worker.generation);
                return Err(Error::other(format!("{reader_label} worker disconnected")));
            }
        }
    }
    Ok(worker.generation)
}

pub(super) fn take_nightly_output_result(
    result_slot: &Mutex<Option<NightlyOutputResult>>,
) -> Option<NightlyOutputResult> {
    match result_slot.lock() {
        Ok(mut slot) => slot.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    }
}

fn collect_nightly_output(
    mut pipe: impl Read,
    shared_total: Arc<AtomicU64>,
    maximum_stream_bytes: usize,
    maximum_total_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, Error> {
    let mut buffer = [0_u8; 8192];
    let mut bytes = Vec::new();
    loop {
        let stream_remaining = maximum_stream_bytes.saturating_sub(bytes.len());
        let total_remaining =
            maximum_total_bytes.saturating_sub(shared_total.load(Ordering::Acquire));
        let total_remaining = usize::try_from(total_remaining).unwrap_or(usize::MAX);
        let available = stream_remaining.min(total_remaining);
        let read_limit = available.min(buffer.len().saturating_sub(1)).saturating_add(1);
        let read_buffer = buffer.get_mut(..read_limit).ok_or_else(|| {
            Error::new(ErrorKind::InvalidData, format!("{label} returned an invalid read limit"))
        })?;
        let read = pipe.read(read_buffer)?;
        if read == 0 {
            return Ok(bytes);
        }
        let next_len = bytes.len().checked_add(read).ok_or_else(|| {
            Error::new(ErrorKind::InvalidData, format!("{label} output length overflowed"))
        })?;
        if next_len > maximum_stream_bytes {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("subprocess output exceeds {maximum_stream_bytes} bytes per stream"),
            ));
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            Error::new(ErrorKind::InvalidData, format!("{label} returned an invalid byte count"))
        })?;
        if !reserve_shared_output_bytes(&shared_total, read, maximum_total_bytes) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("{label} output exceeds {maximum_total_bytes} aggregate bytes"),
            ));
        }
        bytes.extend_from_slice(chunk);
    }
}

fn reserve_shared_output_bytes(total: &AtomicU64, bytes: usize, maximum: u64) -> bool {
    let bytes = bytes as u64;
    total
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
            used.checked_add(bytes).filter(|next| *next <= maximum)
        })
        .is_ok()
}
