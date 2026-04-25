# fading-cli

Command-line interface for the fading language tooling ecosystem.

## Installation

```bash
cargo install --path crates/cli
```

## Commands

### `fading open [month]`

Open a fading entry file in Helix editor.

```bash
fading open              # Current month, jump to today's heading
fading open today        # Current month, jump to today's heading
fading open 12           # 12 months in the past
fading open 2025-01      # Specific month
```

### `fading new [month]`

Create a new monthly entry file.

```bash
fading new              # Current month
fading new today        # Current month
fading new 12           # 12 months in the past
fading new 2025-01      # Specific month
```

Idempotent — skips if the entry already exists.

### `fading stats [month]`

Show a contribution calendar for the past 365 days of journal writing activity.

```bash
fading stats             # Past year up to today
fading stats today       # Past year up to today
fading stats 1           # Past year up to 1 month in the past
fading stats 2025-01     # Past year up to 2025-01-31
```

Activity is scaled by quartile (p25/p50/p75) of character counts across written days.

### `fading push [--full]`

Push entries to Cloudflare KV.

```bash
fading push          # Differential push (since last synced commit)
fading push --full   # Full push (ignore last synced commit)
```

### `fading get`

Download the git bundle from R2 and fetch into the local repository.

### `fading put`

Upload the git bundle to R2.

### `fading notify`

Display a macOS notification. Clicking "Open" opens today's entry in Helix.

### `fading ls`

Start the fading language server.

## Configuration

| Variable                          | Required             | Description                                                          |
| --------------------------------- | -------------------- | -------------------------------------------------------------------- |
| `FADING_HOME`                     | Yes                  | Path to the fading directory                                         |
| `FADING_CLI_CF_ACCOUNT_ID`        | `push`, `get`, `put` | Cloudflare account ID                                                |
| `FADING_CLI_R2_BUCKET`            | `get`, `put`         | Cloudflare R2 bucket name                                            |
| `FADING_CLI_R2_ACCESS_KEY_ID`     | `get`, `put`         | R2 API token access key                                              |
| `FADING_CLI_R2_SECRET_ACCESS_KEY` | `get`, `put`         | R2 API token secret key                                              |
| `FADING_CLI_CF_API_TOKEN`         | `push`               | Cloudflare API token                                                 |
| `FADING_CLI_KV_NAMESPACE_ID`      | `push`               | Cloudflare KV namespace ID                                           |
| `FADING_CLI_ENCRYPTION_KEY`       | `get`, `put`         | AES-256-GCM key for R2 bundle (64 hex chars; `openssl rand -hex 32`) |

Entry file location: `${FADING_HOME}/entries/${YYYY-MM}.md`
