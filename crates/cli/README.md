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
fading new -- -12       # 12 months in the past (-- required for negative offsets)
fading new 2025-01      # Specific month
```

Idempotent — skips if the entry already exists.

### `fading open [month]`

Open a fading entry file in Helix editor.

```bash
fading open              # Current month, jump to today's heading
fading open today        # Current month, jump to today's heading
fading open +1           # 1 month in the future
fading open -- -12       # 12 months in the past (-- required for negative offsets)
fading open 2025-01      # Specific month
```

### `fading stats [month]`

Show a contribution calendar for the past 365 days of journal writing activity.

```bash
fading stats             # Past year up to today
fading stats today       # Past year up to today
fading stats +1          # Past year up to 1 month in the future
fading stats -- -1       # Past year up to 1 month in the past (-- required for negative offsets)
fading stats 2025-01     # Past year up to 2025-01-31
```

Activity is scaled by quartile (p25/p50/p75) of character counts across written days.

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

| Variable                          | Required    | Description                  |
| --------------------------------- | ----------- | ---------------------------- |
| `FADING_HOME`                     | Yes         | Path to the fading directory |
| `FADING_CLI_CF_ACCOUNT_ID`        | `push`      | Cloudflare account ID        |
| `FADING_CLI_R2_BUCKET`            | `push` (R2) | Cloudflare R2 bucket name    |
| `FADING_CLI_R2_ACCESS_KEY_ID`     | `push` (R2) | R2 API token access key      |
| `FADING_CLI_R2_SECRET_ACCESS_KEY` | `push` (R2) | R2 API token secret key      |
| `FADING_CLI_CF_API_TOKEN`         | `push` (KV) | Cloudflare API token         |
| `FADING_CLI_KV_NAMESPACE_ID`      | `push` (KV) | Cloudflare KV namespace ID   |

Entry file location: `${FADING_HOME}/entries/${YYYY-MM}.md`

## Cache

`stats` and `push kv` share a local cache at `${FADING_HOME}/.cache/fading-cli.json`, keyed by git HEAD commit hash. The cache is rebuilt automatically when the commit changes. Add `.cache/` to `${FADING_HOME}/.gitignore` to avoid committing it.
