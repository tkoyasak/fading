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
- **Empty days**: Use `<!-- -->` as placeholder (preserved by Markdown formatters; excluded from KV sync)

## Repository Structure

Rust workspace (Edition 2024, MSRV 1.93.0).

- `crates/cli`: `fading-cli` — CLI binary (ls, notify, open, new, push commands)
- `crates/ls`: `fading-ls` — Language server (sync, code actions, diagnostics)
- `crates/zed`: `fading-zed` — Zed editor extension (cdylib)
- `apps/web`: Web viewer — Vite + React RSC + Cloudflare Workers

## Commands

### Rust

Replace `{package}` with `fading-cli`, `fading-ls`, or `fading-zed`.

```bash
cargo build -p {package}
cargo test -p {package}
cargo clippy -p {package} -- -D warnings
cargo fmt
cargo llvm-cov -p {package}

cargo run -p fading-cli -- ls
cargo run -p fading-cli -- open
cargo run -p fading-cli -- new
cargo run -p fading-cli -- new 2026-04
cargo run -p fading-cli -- push
cargo run -p fading-cli -- push r2
cargo run -p fading-cli -- push kv --full

env UPDATE_XFLAGS=1 cargo build -p fading-cli   # Regenerate xflags code
```

### Web (`apps/web`)

```bash
bun run dev       # Vite dev server (Cloudflare Workers)
bun run build     # Production build
bun run check     # tsc type check
bun run release   # wrangler deploy --strict
bun run types     # Regenerate worker-configuration.d.ts
```

## Key Patterns

### CLI (`fading-cli`)

- `xflags` macro generates argument parsing in `src/flags.rs`
- Commands implement `Cmd` trait: `run(self) -> anyhow::Result<()>`
- `FADING_HOME` env var is required for `open`, `new`, and `push` commands
- `FADING_CLI_CF_ACCOUNT_ID` env var is required for `push` (Cloudflare account ID, shared between R2 and KV)
- `FADING_CLI_R2_BUCKET` env var is required for `push r2` (R2 bucket name)
- `FADING_CLI_R2_ACCESS_KEY_ID` env var is required for `push r2` (R2 API token access key)
- `FADING_CLI_R2_SECRET_ACCESS_KEY` env var is required for `push r2` (R2 API token secret key)
- `FADING_CLI_CF_API_TOKEN` env var is required for `push kv` (Cloudflare API token)
- `FADING_CLI_KV_NAMESPACE_ID` env var is required for `push kv` (KV namespace ID)

### Language Server (`fading-ls`)

- **Activation**: Only when `FADING_HOME` env var exactly matches workspace path (single-root only)
- **Provider pattern**: `CodeActionProvider` trait in `code_actions.rs`, `DiagnosticProvider` trait in `diagnostics.rs`
- **Document storage**: `papaya::HashMap` for concurrent access
- **Text operations**: `crop::Rope` for incremental edits
- **Parsing**: `tree-sitter-md` for heading analysis

### Web (`apps/web`)

- **Stack**: Vite 8 + React 19 RSC + `@vitejs/plugin-rsc` + Cloudflare Workers
- **Styling**: Tailwind CSS v4 (`@tailwindcss/vite`), custom prose styles in `src/global.css`
- **Entry points**: `src/root.tsx` (RSC root), `src/action.tsx` (`"use server"`), `src/client.tsx` (`"use client"`)
- **Data**: Cloudflare KV — `__index` key holds `{keys: string[], commit: string}`; each date key (`YYYY-MM-DD`) holds markdown content
- **Rendering**: `marked` parses markdown to HTML on the server; `dangerouslySetInnerHTML` renders it on the client
- **Dev only**: `RawMarkdown` component shows unparsed markdown source below the next button (`import.meta.env.DEV`)

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
