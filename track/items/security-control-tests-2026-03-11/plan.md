<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Security Control Tests: CI validation for new hardening measures

Regression tests and CI validation for all hardening measures from the other 5 tracks.
Ensure security controls cannot silently degrade without CI detection.
This track should be implemented last, after the other 5 tracks complete.

## Container Security Tests

Verify .git read-only mount and sensitive directory exclusion are enforced.

- [ ] Add CI test verifying .git is read-only inside tools container (docker compose exec git status should fail to write)
- [ ] Add CI test verifying private/ and config/secrets/ are empty/inaccessible inside container

## Hook Behavior Tests

Verify fail-closed error handling in security-critical hooks.

- [ ] Add hook selftest for fail-closed behavior in block-direct-git-ops.py error paths

## Concurrency Tests

Verify file locking prevents metadata.json corruption under concurrent access.

- [ ] Add selftest for filelock-based metadata.json locking (concurrent write detection)

## CI Integration

Wire all new tests into the cargo make ci pipeline.

- [ ] Integrate new security tests into cargo make ci pipeline
