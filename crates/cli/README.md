# fading-cli

Command-line interface for the fading language tooling ecosystem.

## Installation

```bash
cargo install --path crates/cli
```

Or run directly from the repository:

```bash
cargo run -p fading-cli -- <command>
```

## Commands

### `fading ls`

Start the fading language server.

```bash
fading ls
```

The language server provides:
- Text document synchronization
- Code actions for updating metadata
- Diagnostics for frontmatter validation

Only activates when the workspace contains a folder named "fading".

### `fading notify`

Display a notification with the current date (macOS only).

```bash
fading notify
```

Uses `osascript` to show a native macOS notification.

### `fading open [month]`

Open a fading entry file in Helix editor.

```bash
# Open current month's entry
fading open
fading open today

# Open entry from 1 month in the future
fading open +1

# Open entry from 12 months in the past
fading open -12

# Open specific month
fading open 2025-01
```

**Month argument formats:**
- Empty or `today`: Current month (YYYY-MM)
- `+N` or `-N`: N months offset from current month
- `YYYY-MM`: Direct month specification

## Environment Variables

### `FADING_DIR`

Path to the fading repository containing entries.

```bash
export FADING_DIR="$HOME/path/to/fading-repo"
```

**Default**: `.` (current directory)

**Entry file location**: `${FADING_DIR}/entries/${YYYY-MM}.md`

## Development

### Build

```bash
cargo build -p fading-cli
```

### Test

```bash
cargo test -p fading-cli
```

### Coverage

```bash
cargo llvm-cov --package fading-cli
```

Current coverage: **49.64%**

### Update xflags

After modifying `src/flags.rs`:

```bash
env UPDATE_XFLAGS=1 cargo build -p fading-cli
```
