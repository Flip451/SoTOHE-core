# Filesystem Persistence Guard Convention

## Problem

`std::fs::create_dir_all(parent)` unconditionally creates the directory hierarchy.
When directory existence is part of a guard condition (e.g., "only persist if the track
directory already exists"), calling `create_dir_all` before the check silently voids the guard.

## Rule

**Treat `create_dir_all` as an implicit state transition.** If directory existence is part of
a precondition or invariant, validate the directory's presence BEFORE calling `create_dir_all`.
Only call `create_dir_all` when directory creation is the intended behavior.

## Pattern

```rust
// BAD: create_dir_all voids the "directory must exist" guard
fn persist(dir: &Path, data: &[u8]) -> Result<(), Error> {
    std::fs::create_dir_all(dir)?;           // <-- always creates
    if !dir.join("sentinel.json").exists() {  // <-- guard is now meaningless
        return Err(Error::NotInitialized);
    }
    atomic_write_file(&dir.join("data"), data)?;
    Ok(())
}

// GOOD: validate first, create only when intended
fn persist(dir: &Path, data: &[u8]) -> Result<(), Error> {
    if !dir.is_dir() {
        return Err(Error::NotInitialized);   // <-- guard holds
    }
    atomic_write_file(&dir.join("data"), data)?;
    Ok(())
}

// GOOD: create_dir_all is the intended behavior (initialization path)
fn initialize(dir: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(dir)?;           // <-- intentional creation
    atomic_write_file(&dir.join("sentinel.json"), b"{}")?;
    Ok(())
}
```

## Motivating Case

`libs/infrastructure/src/review_v2/persistence/commit_hash_store.rs` calls
`create_dir_all(parent)` before writing `.commit_hash`. In this case the creation is
intentional (the track directory should already exist, and the parent is the track item
directory). However, if a guard depended on the parent directory's absence, the
`create_dir_all` would silently defeat it.

## When to Apply

- Any infrastructure adapter that persists files to disk
- Any path where directory existence is checked as a precondition
- Review `create_dir_all` calls in code review: verify that directory creation is the
  intended behavior, not an accidental side effect that voids a guard
