# Contributing

The project is a development candidate hosted in the private staging repository https://github.com/kevinhhc/agent-dlp-lite. Public release remains subject to owner confirmation. Do not add guessed identities or contact addresses.

Build with Rust 1.100+. Run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, then `cargo build --release --locked`. Benchmarks use `python3 scripts/benchmark.py target/release/agent-dlp --samples 200`. Commit Cargo.lock for reproducibility.

Use synthetic data only. Never execute exfiltration fixtures; inspect them as text in isolated temporary directories. Changes to hook decisions must preserve host permissions on clean input and keep detection output free of raw content. Add meaningful regression cases for detection, limits, redaction and configuration preservation.

Update both README languages and the compatibility matrix when behavior changes. Do not mark desktop compatibility verified without versioned host evidence. Do not create a public release until the owner confirms repository identity, publication and acceptance results.
