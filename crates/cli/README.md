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

### `fading open [month]`

Open a fading entry file in Helix editor.

```bash
fading open              # Current month
fading open today        # Current month
fading open +1           # 1 month in the future
fading open -12          # 12 months in the past
fading open 2025-01      # Specific month
```

## Configuration

| Variable     | Required | Description                                     |
| ------------ | -------- | ----------------------------------------------- |
| `FADING_DIR` | Yes      | Path to the fading directory containing entries |

Entry file location: `${FADING_DIR}/entries/${YYYY-MM}.md`
