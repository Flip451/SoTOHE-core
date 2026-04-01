# Version Baseline — 2026-03-11

Research performed via Gemini CLI (researcher capability).

## Tools (Dockerfile)

| Item | Current | Latest Stable | Recommendation |
|------|---------|---------------|----------------|
| Rust Toolchain | 1.93.1 | 1.94.0 | Upgrade to 1.94.0 |
| cargo-make | 0.37.24 | 0.37.24 | Stay |
| cargo-nextest | 0.9.129 | 0.9.129 | Stay |
| cargo-deny | 0.19.0 | 0.19.0 | Stay |
| cargo-machete | 0.9.1 | 0.9.1 | Stay |
| cargo-llvm-cov | 0.8.4 | 0.8.4 | Stay |
| sccache | 0.14.0 | 0.14.0 | Stay |
| cargo-chef | 0.1.76 | 0.1.77 | Update to 0.1.77 |

## Crates (Cargo.toml)

| Item | Latest Stable | MSRV | Recommendation |
|------|---------------|------|----------------|
| clap | 4.5.60 | 1.74.0 | Use 4.5 (derive) |
| reqwest | 0.13.2 | 1.64.0 | Use 0.13 (blocking) |
| config | 0.15.19 | 1.75.0 | Use 0.15 |
| mockall | 0.14.0 | 1.77.0 | Use 0.14 (dev) |
| thiserror | 2.0.18 | 1.68.0 | Use 2.0 |
| serde | 1.0.228 | 1.56.0 | Use 1.0 (derive) |
| serde_json | 1.0.149 | 1.56.0 | Use 1.0 |
| uuid | 1.22.0 | 1.60.0 | Use 1.22 (v4, serde) |
| chrono | 0.4.44 | 1.62.0 | Use 0.4 (serde) |
| tracing | 0.1.44 | 1.65.0 | Use 0.1 |
| tracing-subscriber | 0.3.22 | 1.65.0 | Use 0.3 |

## MSRV Compatibility

All crates are compatible with MSRV 1.85. Highest requirement: mockall at 1.77.0.

## Notes

- reqwest 0.13: rustls is now the default TLS backend. Enable `native-tls` feature if system certificates are needed.
- Rust 1.94.0: TOML 1.1 support and `array_windows` stabilization.
