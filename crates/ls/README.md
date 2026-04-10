# fading-ls

Language server implementation for the fading language.

## Features

### Text Document Synchronization

Incremental sync with UTF-8 position encoding.

### Code Actions

| Action                         | Description                                                                                                                                   |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `source.updateMetadata.fading` | Updates `modified` field to today's date. Triggers when document is modified, frontmatter is valid TOML, and `modified` is not already today. |

### Diagnostics

| Diagnostic               | Description                                             |
| ------------------------ | ------------------------------------------------------- |
| `missing-frontmatter`    | Document lacks `+++` delimited TOML frontmatter         |
| `invalid-toml`           | Frontmatter is not valid TOML                           |
| `missing-field`          | Required field (`id`, `created`, `modified`) is missing |
| `invalid-id-format`      | `id` is not in YYYY-MM format                           |
| `invalid-date`           | Date field is invalid or malformed                      |
| `created-after-modified` | `created` date is later than `modified` date            |
| `invalid-heading-level`  | Entry heading is not h6 (`######`)                      |
| `invalid-heading-format` | h6 heading doesn't match `YYYY-MM-DD Day` format        |
| `id-filename-mismatch`   | `id` field doesn't match filename (without `.md`)       |

## Configuration

| Variable      | Required | Description                                       |
| ------------- | -------- | ------------------------------------------------- |
| `FADING_HOME` | Yes      | Must exactly match the workspace path to activate |

Only single-root workspaces are supported.

## Usage

Via `fading-cli`:

```bash
fading ls
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
