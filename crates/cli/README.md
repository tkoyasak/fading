# fading-cli

Command-line interface for the fading language tooling ecosystem.

## Installation

```bash
cargo install --path crates/cli
```

## Commands

### `fading ls`

Start the fading language server.

### `fading notify`

Display a notification with the current date (macOS only, uses `osascript`).

### `fading new [month]`

Create a new monthly entry file.

```bash
fading new              # Current month
fading new today        # Current month
fading new +1           # 1 month in the future
fading new -12          # 12 months in the past
fading new 2025-01      # Specific month
```

Idempotent — skips if the entry already exists.

### `fading open [month]`

Open a fading entry file in Helix editor.

```bash
fading open              # Current month
fading open today        # Current month
fading open +1           # 1 month in the future
fading open -12          # 12 months in the past
fading open 2025-01      # Specific month
```

### `fading push [target] [--full]`

Push to Cloudflare (R2 backup and/or KV sync).

```bash
fading push              # R2 backup + KV differential sync
fading push r2           # R2 backup only
fading push kv           # KV differential sync only
fading push kv --full    # KV full sync (ignore last synced commit)
fading push --full       # R2 backup + KV full sync
```

R2 and KV operations run independently — if one fails, the other continues.

## Configuration

| Variable                     | Required    | Description                  |
| ---------------------------- | ----------- | ---------------------------- |
| `FADING_DIR`                 | Yes         | Path to the fading directory |
| `FADING_CLI_R2_BUCKET`       | `push` (R2) | Cloudflare R2 bucket name    |
| `FADING_CLI_KV_NAMESPACE_ID` | `push` (KV) | Cloudflare KV namespace ID   |

Entry file location: `${FADING_DIR}/entries/${YYYY-MM}.md`
