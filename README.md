# gator

Shared Rust TUI and tooling primitives for the gator app family.

`gator` intentionally contains generic infrastructure only: terminal setup, selection output, clipboard support, command output helpers, fuzzy matching, and small TUI helpers. Git, GitHub, project navigation, issue exploration, and session source behavior belong in implementation crates.

## Build

```sh
cargo build
```

## Check

```sh
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

MIT
