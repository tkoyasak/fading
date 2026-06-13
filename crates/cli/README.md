# fading-cli

Command-line interface for the fading language tooling ecosystem.

## Installation

```bash
cargo install --path crates/cli
```

## Commands

`open`, `new`, and `stats` share an optional `when` argument that selects the
target month or day. It has four forms:

| Form          | Meaning                                                       |
| ------------- | ------------------------------------------------------------- |
| _(omitted)_   | Today / the current month                                     |
| `N`           | N months back — a bare number is always an offset, not a year |
| `YYYY-MM`     | A specific month                                              |
| `YYYY-MM-DD`  | A specific date                                               |

### `fading open [when]`

Open a fading entry file in Helix editor.

```bash
fading open              # Current month, jump to today's heading
fading open 12           # 12 months back, jump to the 1st
fading open 2025-01      # Specific month, jump to the 1st
fading open 2025-01-15   # Specific date, jump to that heading
```

### `fading new [when]`

Create a new monthly entry file. Idempotent — skips if the entry already
exists. A date (`YYYY-MM-DD`) is rejected.

```bash
fading new              # Current month
fading new 12           # 12 months back
fading new 2025-01      # Specific month
```

### `fading stats [when]`

Show a contribution calendar for the past 365 days of journal writing activity.

```bash
fading stats             # Past year up to today
fading stats 1           # Past year up to the end of last month
fading stats 2025-01     # Past year up to 2025-01-31
fading stats 2025-01-15  # Past year up to 2025-01-15
```

Activity is scaled relative to the busiest day: its character count fills the
cell, and the range up to it is split into four equal bands.

### `fading get`

Download the git bundle from R2 and fetch into the local repository.

### `fading put`

Upload the git bundle to R2.

### `fading notify`

Display a macOS notification. Clicking "Open" opens today's entry in Helix.

### `fading ls`

Start the fading language server.

## Configuration

| Variable                          | Required     | Description                                                          |
| --------------------------------- | ------------ | -------------------------------------------------------------------- |
| `FADING_HOME`                     | Yes          | Path to the fading directory                                         |
| `FADING_CLI_CF_ACCOUNT_ID`        | `get`, `put` | Cloudflare account ID                                                |
| `FADING_CLI_R2_BUCKET`            | `get`, `put` | Cloudflare R2 bucket name                                            |
| `FADING_CLI_R2_ACCESS_KEY_ID`     | `get`, `put` | R2 API token access key                                              |
| `FADING_CLI_R2_SECRET_ACCESS_KEY` | `get`, `put` | R2 API token secret key                                              |
| `FADING_CLI_ENCRYPTION_KEY`       | `get`, `put` | AES-256-GCM key for R2 bundle (64 hex chars; `openssl rand -hex 32`) |

Entry file location: `${FADING_HOME}/entries/${YYYY-MM}.md`
