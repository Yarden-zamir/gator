# gator

🐊 Shared Rust TUI and tooling primitives for the gator app family.

Nerd Font is recommended for all gator-family CLIs so built-in icons render correctly.

`gator` intentionally contains generic infrastructure only. Git, GitHub, project navigation, issue exploration, and session source behavior belong in the implementation crates.

## Modules

| Module | What it provides |
| --- | --- |
| root | Terminal setup and teardown, TTY handling, selection output, clipboard |
| `theme` | `Theme` (auto/light/dark), the shared `Palette`, OS dark-mode detection |
| `keymap` | Chord parsing and formatting, plus a `Keymap` engine generic over each app's own contexts and actions |
| `config` | TOML discovery, layered file and CLI loading, JSON Schema generation, `$schema` injection, starter files, path normalization |
| `layout` | The shared two-pane shell and a help footer driven by the active keymap |
| `text` | Truncation, `~` collapsing, ANSI and plain line building, wrapping, match highlighting, list windowing, rect and color helpers |
| `search` | `fuzzy_match` and `match_score` ranking |
| `process` | Command output capture, interactive and shell runners, opening URLs |
| `xdg` | State and cache paths, atomic JSON persistence, cross-process locking, a batched worker pool |

Apps keep their own vocabulary: they define the `BindingContext` and `CoreAction` enums and the config sections they need, and implement gator's traits so the shared engines drive them.

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
