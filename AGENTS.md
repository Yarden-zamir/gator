# AGENTS.md

- This repo is the shared `gator` library crate.
- Keep only generic Rust TUI/tooling primitives here.
- Do not add Git, GitHub, project navigation, issue explorer, session source, config, tag, or app-specific compositor behavior.
- Verify with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.
