# CLAUDE.md

## Fading Language

Fading is a Markdown dialect for journaling. Entries are organized by month in separate files, structured with TOML frontmatter and h6 headings.

```markdown
+++
id = "2026-01"
created = 2026-01-01
modified = 2026-01-15
+++

###### 2026-01-14 Wed

こんにちは．

###### 2026-01-15 Thu

また明日ね．

###### 2026-01-16 Fri

<!-- -->
```

- **Frontmatter**: TOML enclosed in `+++` — `id` (YYYY-MM, matches filename), `created`, `modified` (auto-updated by LS)
- **Entries**: Separated by `###### YYYY-MM-DD Day` headings
- **Empty days**: Use `<!-- -->` as placeholder (preserved by Markdown formatters)

## Repository Structure

Rust workspace (Edition 2024, MSRV 1.93.0).

- `crates/cli`: `fading-cli` — CLI binary (open, new, stats, get, put, notify, ls commands)
- `crates/ls`: `fading-ls` — Language server (sync, code actions, diagnostics)
- `crates/zed`: `fading-zed` — Zed editor extension (cdylib)
- `crates/xtask`: `xtask` — developer tasks (fixture generation)

`fixtures/entries/*.md` are **generated** (git-ignored). In debug builds `fading-cli`
uses `fixtures` as `FADING_HOME`. Two steps, both git-ignored:
`cargo xtask corpus` downloads the source corpus into `crates/xtask/corpus/` (needs
`gh`), then `cargo xtask fixtures` regenerates the entries from that cache (offline).
Sources are listed in `crates/xtask/works.txt`.

## Commands

### Secrets (`secretspec`)

Environment variables for CLI commands are managed via [secretspec](https://secretspec.dev). Profiles and vars are defined in `secretspec.toml`.

```bash
secretspec run -- cargo run -p fading-cli -- put
```

The `development` profile covers all `FADING_CLI_*` vars. `FADING_HOME` is not included (set separately).

### Rust

Replace `{package}` with `fading-cli`, `fading-ls`, or `fading-zed`.

```bash
cargo build -p {package}
cargo test -p {package}
cargo clippy -p {package} -- -D warnings
cargo fmt
cargo llvm-cov -p {package}

cargo run -p fading-cli -- open
cargo run -p fading-cli -- new
cargo run -p fading-cli -- new 2026-04
cargo run -p fading-cli -- stats
cargo run -p fading-cli -- stats 2026-04
cargo run -p fading-cli -- stats 1   # 1 month back
cargo run -p fading-cli -- get
cargo run -p fading-cli -- put
cargo run -p fading-cli -- notify
cargo run -p fading-cli -- ls

env UPDATE_XFLAGS=1 cargo build -p fading-cli   # Regenerate xflags code

cargo xtask corpus     # Download source corpus into crates/xtask/corpus (needs `gh`)
cargo xtask fixtures   # Regenerate fixtures/entries/*.md from the cached corpus
```

## Key Patterns

### CLI (`fading-cli`)

- `xflags` macro generates argument parsing in `src/flags.rs`
- Commands implement `Cmd` trait: `run(self) -> anyhow::Result<()>`
- `FADING_HOME` env var is required for `open`, `new`, `get`, and `put` commands
- `FADING_CLI_CF_ACCOUNT_ID` env var is required for `get`, `put` (Cloudflare account ID)
- `FADING_CLI_R2_BUCKET` env var is required for `get`, `put` (R2 bucket name)
- `FADING_CLI_R2_ACCESS_KEY_ID` env var is required for `get`, `put` (R2 API token access key)
- `FADING_CLI_R2_SECRET_ACCESS_KEY` env var is required for `get`, `put` (R2 API token secret key)
- `FADING_CLI_ENCRYPTION_KEY` env var is required for `get` and `put` (64 hex chars = 32-byte AES-256-GCM key; generate with `openssl rand -hex 32`)

### Language Server (`fading-ls`)

- **Activation**: Only when `FADING_HOME` env var exactly matches workspace path (single-root only)
- **Provider pattern**: `CodeActionProvider` trait in `code_actions.rs`, `DiagnosticProvider` trait in `diagnostics.rs`
- **Document storage**: `papaya::HashMap` for concurrent access
- **Text operations**: `crop::Rope` for incremental edits
- **Parsing**: `tree-sitter-md` for heading analysis

## Testing

All tests are inline `#[cfg(test)]` modules (no `tests/` directories).

### Property-Based Testing

Uses `proptest` extensively. Convention: `ProptestConfig::with_cases(1000)`.

- Define reusable strategies in `mod strategies` within test modules
- Prefer generating valid data directly over filtering (e.g., day offsets instead of regex dates)
- Property test names start with `prop_`

### Environment Variable Tests

`std::env::set_var` / `remove_var` are **unsafe** in Edition 2024. Tests that mutate env vars must:

1. Wrap calls in `unsafe { }` blocks
2. Use `#[serial_test::serial]` to prevent parallel execution

### Per-Crate Test Notes

- **fading-ls backend**: Async tests with `#[tokio::test]` + `#[serial]`
- **fading-zed**: No tests (thin wrapper around external API)
