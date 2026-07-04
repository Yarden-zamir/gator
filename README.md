# gator

Shared Rust TUI and tooling primitives for the gator app family.

`gator` contains generic infrastructure: terminal setup, selection output, clipboard support, command output helpers, fuzzy matching, and small TUI helpers.

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
