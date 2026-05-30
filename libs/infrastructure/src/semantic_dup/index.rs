//! Infrastructure adapter implementing [`usecase::semantic_dup::SemanticIndexPort`]
//! via LanceDB (local file DB, Apache 2.0, official Rust SDK).

use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::types::Float32Type;
use arrow_array::{
    FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, RecordBatchReader,
    StringArray,
};
use arrow_schema::{ArrowError, DataType, Field, Schema};
use futures::TryStreamExt as _;
use lancedb::query::{ExecutableQuery as _, QueryBase as _};
use lancedb::{Connection, DistanceType};

use domain::semantic_dup::{CodeFragment, SimilarFragment, SimilarityScore};
use usecase::semantic_dup::{SemanticIndexError, SemanticIndexPort};

// Column name constants to keep insert/search in sync.
const COL_PATH: &str = "source_path";
const COL_CONTENT: &str = "content";
const COL_VECTOR: &str = "vector";
const TABLE_NAME: &str = "fragments";

/// Infrastructure adapter implementing [`SemanticIndexPort`] using LanceDB
/// (local file DB, Apache 2.0, official Rust SDK).
/// CN-04: lancedb dependency is confined to infrastructure.
///
/// The adapter holds a single-worker multi-thread Tokio runtime to bridge
/// LanceDB's async API to the synchronous [`SemanticIndexPort`] interface.
/// The runtime is created once at construction and reused for every `insert`
/// / `search` call.
///
/// Each synchronous call spawns a dedicated OS thread and drives the async
/// future via `Handle::block_on`.  Using a multi-thread runtime (instead of
/// `current_thread`) ensures that `Handle::block_on` drives the scheduler
/// correctly from any OS thread — a `current_thread` runtime only drives
/// futures on the thread calling `Runtime::block_on` and would hang when
/// driven via `Handle::block_on` from a bridge thread.
///
/// The runtime is wrapped in `Option` so that `Drop` can take ownership
/// of it and shut it down on a dedicated OS thread, avoiding the Tokio
/// panic that occurs when a `Runtime` is dropped from inside an async
/// executor context (e.g. when the adapter itself is constructed or used
/// inside a Tokio task and then dropped).
///
/// The Arrow table schema is:
/// - `source_path`: `Utf8`
/// - `content`:     `Utf8`
/// - `vector`:      `FixedSizeList<Float32, DIM>` where `DIM` is inferred from
///   the first inserted embedding.
pub struct LanceDbSemanticIndexAdapter {
    /// Wrapped in `Option` so `Drop` can take ownership for off-thread shutdown.
    runtime: Option<tokio::runtime::Runtime>,
    /// Wrapped in `Arc` so the connection handle can be cloned cheaply into
    /// the OS threads that drive each async operation.
    connection: Arc<Connection>,
}

impl std::fmt::Debug for LanceDbSemanticIndexAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanceDbSemanticIndexAdapter").finish_non_exhaustive()
    }
}

impl LanceDbSemanticIndexAdapter {
    /// Open or create a LanceDB database at `db_path`.
    ///
    /// The database directory is created by LanceDB if it does not exist.
    /// No network access is performed.
    ///
    /// The async `lancedb::connect` call is driven on a dedicated OS thread so
    /// that construction is safe even when called from within an existing Tokio
    /// executor context (calling `Runtime::block_on` from inside a Tokio
    /// executor panics; using a separate OS thread avoids that).
    ///
    /// # Errors
    ///
    /// Returns [`SemanticIndexError::OpenFailed`] if building the Tokio runtime,
    /// spawning the connection thread, or opening the database fails.
    pub fn new(db_path: PathBuf) -> Result<Self, SemanticIndexError> {
        // Use a single-worker multi-thread runtime so that `Handle::block_on`
        // works correctly from any OS thread (including freshly-spawned bridge
        // threads).  A `current_thread` runtime only drives futures on the
        // thread that calls `Runtime::block_on`; calling `Handle::block_on`
        // from a different OS thread does not drive the scheduler and can hang.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| SemanticIndexError::OpenFailed { source: e.to_string() })?;

        // Reject non-UTF-8 paths: lancedb accepts a `&str` URI, so a path that
        // cannot be represented as valid UTF-8 would need lossy conversion and
        // risk opening the wrong directory.
        let db_path_str = db_path
            .to_str()
            .ok_or_else(|| SemanticIndexError::OpenFailed {
                source: format!("database path is not valid UTF-8: {}", db_path.display()),
            })?
            .to_owned();

        // Drive the async connect on a dedicated OS thread so that `block_on`
        // is never nested inside an ambient Tokio runtime (which would panic).
        let handle = runtime.handle().clone();
        let connection = std::thread::Builder::new()
            .name("lancedb-connect".to_owned())
            .spawn(move || handle.block_on(lancedb::connect(&db_path_str).execute()))
            .map_err(|e| SemanticIndexError::OpenFailed {
                source: format!("failed to spawn async bridge thread: {e}"),
            })?
            .join()
            .map_err(|_| SemanticIndexError::OpenFailed {
                source: "async bridge thread panicked during connect".to_owned(),
            })?
            .map_err(|e| SemanticIndexError::OpenFailed { source: e.to_string() })?;

        Ok(Self { runtime: Some(runtime), connection: Arc::new(connection) })
    }

    /// Build the Arrow schema for the fragments table.
    fn arrow_schema(dim: i32) -> Schema {
        Schema::new(vec![
            Field::new(COL_PATH, DataType::Utf8, false),
            Field::new(COL_CONTENT, DataType::Utf8, false),
            Field::new(
                COL_VECTOR,
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                false,
            ),
        ])
    }

    /// Normalize raw cosine similarity from `[-1, 1]` to `[0, 1]` by clamping
    /// negative values to `0.0` (per `SimilarityScore`'s doc: Jina v2 produces
    /// near-zero negatives in practice, so clamping is the correct approach).
    fn normalize_cosine(raw: f32) -> f32 {
        raw.clamp(0.0, 1.0)
    }

    /// Drive an async future on the held runtime from a freshly spawned OS
    /// thread, returning its `Result`.
    ///
    /// Using a dedicated OS thread (rather than calling `Runtime::block_on`
    /// directly) avoids a panic when the caller is itself executing inside a
    /// Tokio executor: `block_on` panics on nested runtime entry; an OS thread
    /// has no ambient runtime and is always safe to drive.
    ///
    /// [`std::thread::Builder`] is used (not `std::thread::spawn`) so that
    /// OS-level thread-creation failures are returned as an `Err` rather than
    /// causing a panic.
    ///
    /// # Errors
    ///
    /// Returns `thread_err` if OS thread creation fails or if the spawned
    /// thread panics (which indicates a bug in the async future, not a
    /// user-facing storage error).
    fn block_on_thread<F, T, E>(&self, future: F, thread_err: E) -> Result<T, E>
    where
        F: std::future::Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        // `runtime` is `Some` for the entire lifetime of the adapter except
        // during `Drop`.  The `None` branch is unreachable in normal use.
        let handle = match &self.runtime {
            Some(rt) => rt.handle().clone(),
            None => return Err(thread_err),
        };
        let join_handle = match std::thread::Builder::new()
            .name("lancedb-async-bridge".to_owned())
            .spawn(move || handle.block_on(future))
        {
            Ok(jh) => jh,
            Err(_) => return Err(thread_err),
        };
        join_handle.join().unwrap_or(Err(thread_err))
    }
}

impl Drop for LanceDbSemanticIndexAdapter {
    /// Shut down the Tokio runtime on a dedicated OS thread.
    ///
    /// Dropping a `tokio::runtime::Runtime` from within an async executor
    /// context (e.g. a Tokio task) panics because Tokio's shutdown path
    /// internally calls `block_on`.  By moving the runtime to a freshly
    /// spawned OS thread and dropping it there, we guarantee a clean
    /// shutdown regardless of the calling context.
    ///
    /// If spawning the shutdown thread fails (rare OS error), we fall back
    /// to dropping the runtime in place, which is still correct when the
    /// caller is not inside a Tokio executor.
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // Attempt to shut down on a dedicated OS thread to avoid the
            // "dropping a runtime in an async context" panic.  If spawning
            // the thread fails, the closure (and the captured `runtime`) is
            // returned inside the `Err` variant and dropped here on the
            // current thread — a fallback that is still correct when the
            // caller is not inside a Tokio executor.
            let result = std::thread::Builder::new()
                .name("lancedb-runtime-shutdown".to_owned())
                .spawn(move || drop(runtime));
            if let Ok(handle) = result {
                // Best-effort join; ignore errors (we're in Drop).
                let _ = handle.join();
            }
        }
    }
}

impl SemanticIndexPort for LanceDbSemanticIndexAdapter {
    /// Insert a fragment and its embedding into the LanceDB table.
    ///
    /// The table is created on first insert (schema is inferred from the
    /// embedding dimension). Subsequent inserts append rows.
    ///
    /// If two concurrent callers race on an empty database, the loser of
    /// the `create_table` race retries with `open_table`, ensuring no insert
    /// is silently dropped.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticIndexError::InsertFailed`] if the insert operation
    /// fails.
    fn insert(&self, fragment: &CodeFragment, embedding: &[f32]) -> Result<(), SemanticIndexError> {
        // Reject non-UTF-8 source paths: the Arrow Utf8 column cannot represent
        // arbitrary byte sequences, so a lossy conversion would silently corrupt
        // the stored path and prevent round-tripping back to the original PathBuf.
        let source_path_str = fragment
            .source_path
            .to_str()
            .ok_or_else(|| SemanticIndexError::InsertFailed {
                source: format!(
                    "source_path is not valid UTF-8: {}",
                    fragment.source_path.display()
                ),
            })?
            .to_owned();

        let dim = embedding.len() as i32;
        let schema = Arc::new(Self::arrow_schema(dim));

        // Build Arrow arrays for a single row.
        let path_arr = Arc::new(StringArray::from(vec![source_path_str]));
        let content_arr = Arc::new(StringArray::from(vec![fragment.content().to_owned()]));
        let float_values: Vec<Option<f32>> = embedding.iter().map(|&v| Some(v)).collect();
        let vector_arr = Arc::new(FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
            vec![Some(float_values)],
            dim,
        ));

        let batch = RecordBatch::try_new(schema.clone(), vec![path_arr, content_arr, vector_arr])
            .map_err(|e| SemanticIndexError::InsertFailed { source: e.to_string() })?;

        // RecordBatchIterator must be boxed as dyn RecordBatchReader + Send for
        // lancedb's Scannable trait bound.
        let reader: Box<dyn RecordBatchReader + Send> = Box::new(RecordBatchIterator::new(
            vec![Ok::<RecordBatch, ArrowError>(batch)].into_iter(),
            schema.clone(),
        ));

        let connection = Arc::clone(&self.connection);
        self.block_on_thread(
            async move {
                // Open existing table or create a new one.  Only fall through to
                // create_table when the error is specifically "table not found";
                // other errors (permissions, corruption, I/O) are propagated as
                // InsertFailed.
                let table = match connection.open_table(TABLE_NAME).execute().await {
                    Ok(t) => t,
                    Err(e) if is_table_not_found(&e) => {
                        // Table does not exist — create it with an empty batch to
                        // establish the schema, then fall through to add the row.
                        // If a concurrent caller races here and creates the table
                        // first, we catch the "already exists" error and fall back
                        // to open_table so the insert is not lost.
                        let empty_reader: Box<dyn RecordBatchReader + Send> =
                            Box::new(RecordBatchIterator::new(
                                std::iter::empty::<Result<RecordBatch, ArrowError>>(),
                                schema.clone(),
                            ));
                        match connection.create_table(TABLE_NAME, empty_reader).execute().await {
                            Ok(t) => t,
                            Err(ce) if is_table_already_exists(&ce) => {
                                // Concurrent creator won the race; open the table
                                // it just created.
                                connection.open_table(TABLE_NAME).execute().await.map_err(|oe| {
                                    SemanticIndexError::InsertFailed { source: oe.to_string() }
                                })?
                            }
                            Err(ce) => {
                                return Err(SemanticIndexError::InsertFailed {
                                    source: ce.to_string(),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        return Err(SemanticIndexError::InsertFailed { source: e.to_string() });
                    }
                };

                table
                    .add(reader)
                    .execute()
                    .await
                    .map(|_| ())
                    .map_err(|e| SemanticIndexError::InsertFailed { source: e.to_string() })
            },
            SemanticIndexError::InsertFailed {
                source: "async bridge thread panicked during insert".to_owned(),
            },
        )
    }

    /// Search the index for the top-k most similar fragments using ANN cosine
    /// similarity.
    ///
    /// Raw cosine distance (LanceDB cosine distance = `1 - cosine_similarity`)
    /// is converted back to similarity as `1 - distance`, normalized/clamped
    /// to `[0, 1]`, and wrapped in [`SimilarityScore`].
    ///
    /// # Errors
    ///
    /// Returns [`SemanticIndexError::SearchFailed`] if the query fails or if
    /// result deserialization fails.
    fn search(
        &self,
        embedding: &[f32],
        top_k: domain::semantic_dup::TopK,
    ) -> Result<Vec<SimilarFragment>, SemanticIndexError> {
        // Clamp the requested k to a sane maximum before forwarding to LanceDB's
        // `.limit()` call.  Passing an extremely large value (e.g. usize::MAX)
        // to LanceDB can cause oversized/unbounded result sets.  1_000_000 is a
        // generous cap that exceeds realistic workspace sizes while remaining
        // representable as a `usize` on all supported platforms.
        const MAX_SEARCH_LIMIT: usize = 1_000_000;
        let k = top_k.value().min(MAX_SEARCH_LIMIT);
        let query_vec: Vec<f32> = embedding.to_vec();
        let connection = Arc::clone(&self.connection);

        let batches: Vec<RecordBatch> = self.block_on_thread(
            async move {
                // If the table does not exist yet (first-run graceful path), return
                // empty.  Any other open_table failure (permissions, corruption,
                // I/O errors) is propagated as SearchFailed.
                let table = match connection.open_table(TABLE_NAME).execute().await {
                    Ok(t) => t,
                    Err(e) if is_table_not_found(&e) => {
                        return Ok::<Vec<RecordBatch>, SemanticIndexError>(Vec::new());
                    }
                    Err(e) => {
                        return Err(SemanticIndexError::SearchFailed { source: e.to_string() });
                    }
                };

                let stream = table
                    .vector_search(query_vec)
                    .map_err(|e| SemanticIndexError::SearchFailed { source: e.to_string() })?
                    .distance_type(DistanceType::Cosine)
                    .limit(k)
                    .execute()
                    .await
                    .map_err(|e| SemanticIndexError::SearchFailed { source: e.to_string() })?;

                stream
                    .try_collect::<Vec<_>>()
                    .await
                    .map_err(|e| SemanticIndexError::SearchFailed { source: e.to_string() })
            },
            SemanticIndexError::SearchFailed {
                source: "async bridge thread panicked during search".to_owned(),
            },
        )?;

        let mut results = Vec::new();
        for batch in &batches {
            results.extend(extract_similar_fragments(batch)?);
        }
        Ok(results)
    }
}

/// Return `true` when a LanceDB error indicates that the requested table does
/// not exist yet (i.e. no data has been inserted).
///
/// Matches `lancedb::Error::TableNotFound` directly rather than
/// substring-searching `to_string()`. The actual missing-table error text is
/// `Table '{name}' was not found`, which the old substring patterns
/// (`"does not exist"` / `"Table not found"`) did not match — causing the
/// fresh-database create path in `insert` and the graceful empty-return path
/// in `search` to never trigger.
///
/// All other storage errors (permissions, corruption, I/O) are NOT matched,
/// so callers must propagate them rather than treating them as "empty index".
fn is_table_not_found(e: &lancedb::Error) -> bool {
    matches!(e, lancedb::Error::TableNotFound { .. })
}

/// Return `true` when a LanceDB error indicates that a table already exists.
///
/// Matches `lancedb::Error::TableAlreadyExists` directly rather than
/// substring-searching `to_string()`.
///
/// Used to handle the TOCTOU race in `insert`: if two callers concurrently
/// detect "table not found" and both attempt `create_table`, the loser gets
/// this error and should fall back to `open_table` instead of propagating the
/// error.
fn is_table_already_exists(e: &lancedb::Error) -> bool {
    matches!(e, lancedb::Error::TableAlreadyExists { .. })
}

/// Extract [`SimilarFragment`]s from a LanceDB result [`RecordBatch`].
///
/// LanceDB appends a `_distance` column to vector-search results. For cosine
/// similarity, the distance metric is `1 - cosine_similarity`, so we recover
/// the raw cosine as `1 - distance` and then normalize to `[0, 1]`.
fn extract_similar_fragments(
    batch: &RecordBatch,
) -> Result<Vec<SimilarFragment>, SemanticIndexError> {
    let path_col =
        batch.column_by_name(COL_PATH).ok_or_else(|| SemanticIndexError::SearchFailed {
            source: format!("missing column '{COL_PATH}' in search result"),
        })?;
    let content_col =
        batch.column_by_name(COL_CONTENT).ok_or_else(|| SemanticIndexError::SearchFailed {
            source: format!("missing column '{COL_CONTENT}' in search result"),
        })?;
    let distance_col =
        batch.column_by_name("_distance").ok_or_else(|| SemanticIndexError::SearchFailed {
            source: "missing '_distance' column in search result".to_owned(),
        })?;

    let paths = path_col.as_any().downcast_ref::<StringArray>().ok_or_else(|| {
        SemanticIndexError::SearchFailed { source: format!("column '{COL_PATH}' is not Utf8") }
    })?;
    let contents = content_col.as_any().downcast_ref::<StringArray>().ok_or_else(|| {
        SemanticIndexError::SearchFailed { source: format!("column '{COL_CONTENT}' is not Utf8") }
    })?;
    let distances = distance_col.as_any().downcast_ref::<Float32Array>().ok_or_else(|| {
        SemanticIndexError::SearchFailed { source: "'_distance' column is not Float32".to_owned() }
    })?;

    let mut fragments = Vec::with_capacity(batch.num_rows());
    for i in 0..batch.num_rows() {
        let path_str = paths.value(i);
        let content_str = contents.value(i);
        let distance = distances.value(i);

        // distance = 1 - cosine_similarity; recover raw cosine.
        let raw_cosine = 1.0_f32 - distance;
        let normalized = LanceDbSemanticIndexAdapter::normalize_cosine(raw_cosine);

        let score = SimilarityScore::new(normalized).map_err(|e| {
            SemanticIndexError::SearchFailed { source: format!("score normalization error: {e}") }
        })?;

        let fragment =
            CodeFragment::new(PathBuf::from(path_str), content_str.to_owned()).map_err(|e| {
                SemanticIndexError::SearchFailed {
                    source: format!("invalid fragment from index: {e}"),
                }
            })?;

        fragments.push(SimilarFragment { fragment, score });
    }

    Ok(fragments)
}
