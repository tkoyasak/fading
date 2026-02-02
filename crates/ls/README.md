# fading-ls

Language server implementation for the fading language.

## Features

### Text Document Synchronization

- **Sync mode**: Incremental
- **Position encoding**: UTF-8
- **Open/Close tracking**: Supported
- **Change tracking**: Incremental updates
- **Save notifications**: Supported

### Code Actions

#### `source.updateMetadata.fading`

Automatically updates the `modified` field in the frontmatter to today's date when the document is modified.

**Trigger conditions**:

- Document has been modified since opening
- Frontmatter exists and is valid TOML
- `modified` field is not already set to today's date

**Example**:

```markdown
+++
id = "2026-01"
created = 2026-01-01
modified = 2026-01-15  # Will be updated to today's date
+++
```

### Diagnostics

The language server provides the following diagnostics:

#### `missing-frontmatter`

Triggered when a document lacks TOML frontmatter enclosed in `+++` delimiters.

#### `invalid-toml`

Triggered when the frontmatter is not valid TOML syntax.

#### `missing-field`

Triggered when required fields (`id`, `created`, `modified`) are missing from the frontmatter.

#### `invalid-id-format`

Triggered when the `id` field is not in YYYY-MM format.

#### `invalid-date`

Triggered when date fields (`created`, `modified`) are invalid or malformed.

#### `created-after-modified`

Triggered when the `created` date is later than the `modified` date.

#### `invalid-heading-level`

Triggered when entry headings are not h6 (######).

#### `invalid-heading-format`

Triggered when h6 headings don't match the YYYY-MM-DD Day format.

#### `id-filename-mismatch`

Triggered when the frontmatter `id` field does not match the filename (without `.md` extension).

**Example**:

- Filename: `2026-01.md`
- Expected `id`: `"2026-01"`

## Activation

The language server activates when the `FADING_DIR` environment variable matches the workspace path.

This prevents the server from running in unrelated projects.

**Note**: Only single-root workspaces are supported. Multi-root workspaces will not activate the server.

## Architecture

### Key Components

- **backend.rs**: LSP protocol implementation and request handling
- **document.rs**: Document state management with `crop::Rope` for efficient text operations
- **code_actions.rs**: Code action providers (metadata updates)
- **diagnostics.rs**: Diagnostic generation for validation errors

### Concurrency

- Uses `papaya::HashMap` for lock-free concurrent document storage
- Async/await with `tokio` runtime
- Thread-safe document updates

## Usage

The language server is typically invoked through `fading-cli`:

```bash
fading ls
```

Or directly:

```bash
cargo run -p fading-ls
```

### Editor Configuration

#### Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "fading"
scope = "source.fading"
file-types = ["md"]
roots = []
language-servers = ["fading-ls"]

[language-server.fading-ls]
command = "fading"
args = ["ls"]
```

#### Zed

The `fading-zed` extension provides automatic integration.

## Development

### Build

```bash
cargo build -p fading-ls
```

### Test

```bash
cargo test -p fading-ls
```

### Coverage

```bash
cargo llvm-cov -p fading-ls
```

Current coverage: **61.32%**

## Dependencies

- **tower-lsp-server**: LSP protocol implementation
- **tree-sitter**: Markdown parsing
- **jiff**: Date/time handling
- **papaya**: Concurrent hashmap
- **crop**: Rope data structure for efficient text editing
- **toml**: TOML frontmatter parsing
