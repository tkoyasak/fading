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
```

- **Frontmatter**: TOML enclosed in `+++` — `id` (YYYY-MM, matches filename), `created`, `modified` (auto-updated by LS)
- **Entries**: Separated by `###### YYYY-MM-DD Day` headings

## Repository Structure

Rust workspace (Edition 2024, MSRV 1.93.0).

- `crates/cli`: `fading-cli` — CLI binary (ls, notify, open commands)
- `crates/ls`: `fading-ls` — Language server (sync, code actions, diagnostics)
- `crates/zed`: `fading-zed` — Zed editor extension (cdylib)
- `tasks/entries`: `task-entries` — GitHub Actions task runner (entry generation)

## Commands

Replace `{package}` with `fading-cli`, `fading-ls`, `fading-zed`, or `task-entries`.

```bash
cargo build -p {package}
cargo test -p {package}
cargo clippy -p {package} -- -D warnings
cargo fmt
cargo llvm-cov -p {package}

cargo run -p fading-cli -- ls
cargo run -p fading-cli -- open
cargo run -p task-entries -- generate

env UPDATE_XFLAGS=1 cargo build -p fading-cli   # Regenerate xflags code
```

## Key Patterns

### CLI (`fading-cli`)

- `xflags` macro generates argument parsing in `src/flags.rs`
- Commands implement `Cmd` trait: `run(self) -> anyhow::Result<()>`
- `FADING_DIR` env var is required for `open` command

### Language Server (`fading-ls`)

- **Activation**: Only when `FADING_DIR` env var exactly matches workspace path (single-root only)
- **Provider pattern**: `CodeActionProvider` trait in `code_actions.rs`, `DiagnosticProvider` trait in `diagnostics.rs`
- **Document storage**: `papaya::HashMap` for concurrent access
- **Text operations**: `crop::Rope` for incremental edits
- **Parsing**: `tree-sitter-md` for heading analysis

### Task Runner (`task-entries`)

- Runs in GitHub Actions cron jobs (monthly, UTC-based)
- Idempotent: checks existing branches/PRs before creation
- Uses `gh` CLI for all GitHub API operations

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
