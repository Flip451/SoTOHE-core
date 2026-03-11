<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Build Isolation: per-worker CARGO_TARGET_DIR separation

CON-05: Single tools-daemon + single target/ causes deadlock with parallel builds.
Introduce WORKER_ID-based CARGO_TARGET_DIR separation for parallel Agent Teams workers.
Default behavior unchanged (single worker), opt-in for parallel isolation.
sccache remains shared across workers for compilation cache efficiency.

## Compose Configuration

Add WORKER_ID env var support: CARGO_TARGET_DIR=/workspace/target-${WORKER_ID:-default}.
Each worker gets isolated target directory while sharing source and sccache.

- [ ] compose.yml: Add CARGO_TARGET_DIR override support via WORKER_ID environment variable

## Makefile Integration

Add WORKER_ID passthrough to -exec tasks.
Default (no WORKER_ID) uses /workspace/target for backward compatibility.

- [ ] Makefile.toml: Add worker-aware -exec tasks that pass WORKER_ID to container

## Documentation and Validation

Document worker isolation pattern.
Verify sccache hit rates remain high with separated target dirs.

- [ ] Document parallel worker usage pattern and WORKER_ID convention
- [ ] Verify sccache sharing works correctly across isolated target directories
