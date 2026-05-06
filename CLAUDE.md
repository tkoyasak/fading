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

- `crates/cli`: `fading-cli` — CLI binary (open, new, stats, push, get, put, notify, ls commands)
- `crates/ls`: `fading-ls` — Language server (sync, code actions, diagnostics)
- `crates/zed`: `fading-zed` — Zed editor extension (cdylib)
- `apps/web`: Web viewer — Vite + React RSC + Cloudflare Workers

## Commands

### Secrets (`secretspec`)

Environment variables for CLI commands are managed via [secretspec](https://secretspec.dev). Profiles and vars are defined in `secretspec.toml`.

```bash
secretspec run -- cargo run -p fading-cli -- push
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
cargo run -p fading-cli -- push
cargo run -p fading-cli -- push --full
cargo run -p fading-cli -- get
cargo run -p fading-cli -- put
cargo run -p fading-cli -- notify
cargo run -p fading-cli -- ls

env UPDATE_XFLAGS=1 cargo build -p fading-cli   # Regenerate xflags code
```

### Web (`apps/web`)

```bash
bun run dev       # React Router dev server (dev env, local KV)
bun run build     # dev build → build/server/wrangler.json
bun run build:prod  # production build → build/server/wrangler.json
bun run check     # react-router typegen + tsc type check
bun run release   # production build + wrangler deploy --strict
bun run types     # Regenerate worker-configuration.d.ts

# dev: preview KV (0d8eec8f81a34da393ee7e7a36712656), workers.dev
# production: prod KV (d7d628060e0e43d89aeae77dd95160b5), custom domain

# Wrangler environments (wrangler.json has top-level=production, env.dev=dev)
# - CLOUDFLARE_ENV: Cloudflare Vite plugin uses this at build/dev time to flatten
#                   wrangler.json → build/server/wrangler.json
#                   hardcoded in package.json scripts (no .env files)
# - --env: wrangler CLI uses this directly (e.g. wrangler triggers deploy --env dev)

# Upload dev version (generates preview URL):
#   bun run build && bun wrangler versions upload
#   bun wrangler triggers deploy --env dev   # run once to enable workers.dev
```

## Key Patterns

### CLI (`fading-cli`)

- `xflags` macro generates argument parsing in `src/flags.rs`
- Commands implement `Cmd` trait: `run(self) -> anyhow::Result<()>`
- `FADING_HOME` env var is required for `open`, `new`, `push`, `get`, and `put` commands
- `FADING_CLI_CF_ACCOUNT_ID` env var is required for `push`, `get`, `put` (Cloudflare account ID)
- `FADING_CLI_R2_BUCKET` env var is required for `get`, `put` (R2 bucket name)
- `FADING_CLI_R2_ACCESS_KEY_ID` env var is required for `get`, `put` (R2 API token access key)
- `FADING_CLI_R2_SECRET_ACCESS_KEY` env var is required for `get`, `put` (R2 API token secret key)
- `FADING_CLI_CF_API_TOKEN` env var is required for `push` (Cloudflare API token)
- `FADING_CLI_KV_NAMESPACE_ID` env var is required for `push` (KV namespace ID)
- `FADING_CLI_ENCRYPTION_KEY` env var is required for `get` and `put` (64 hex chars = 32-byte AES-256-GCM key; generate with `openssl rand -hex 32`)

### Language Server (`fading-ls`)

- **Activation**: Only when `FADING_HOME` env var exactly matches workspace path (single-root only)
- **Provider pattern**: `CodeActionProvider` trait in `code_actions.rs`, `DiagnosticProvider` trait in `diagnostics.rs`
- **Document storage**: `papaya::HashMap` for concurrent access
- **Text operations**: `crop::Rope` for incremental edits
- **Parsing**: `tree-sitter-md` for heading analysis

### Web (`apps/web`)

- **Stack**: Vite 8 + React Router 7 (RSC framework mode, experimental) + `@vitejs/plugin-rsc` + Cloudflare Workers
- **Styling**: Tailwind CSS v4 (`@tailwindcss/vite`), custom prose styles in `app/global.css`
- **Layout**: React Router `app/` convention — `app/root.tsx` (Layout/App/ErrorBoundary), `app/routes.ts` (route table), `app/routes/home.tsx` (`ServerComponent` export), `app/actions.tsx` (`"use server"`), `app/viewer.tsx` (`"use client"`)
- **Wrangler `main`**: `@react-router/dev/config/default-rsc-entries/entry.rsc` — virtual entry resolved by the RR Vite plugin; no hand-written entry files
- **Data**: Cloudflare KV — `__index` key holds `{keys: string[], commit: string}`; each date key (`YYYYMMDD`) holds markdown content
- **Rendering**: `marked` parses markdown to HTML on the server; `dangerouslySetInnerHTML` renders it on the client

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
