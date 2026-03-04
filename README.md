# fading-crates

Tooling ecosystem for the fading language — a Markdown dialect for journaling.

## Packages

| Package                       | Description                                       |
| ----------------------------- | ------------------------------------------------- |
| [fading-cli](crates/cli)      | Command-line interface (ls, notify, open)         |
| [fading-ls](crates/ls)        | Language server (sync, code actions, diagnostics) |
| [fading-zed](crates/zed)      | Zed editor extension                              |
| [task-entries](tasks/entries) | GitHub Actions task runner for entry generation   |
| [web](apps/web)               | Web viewer (Cloudflare Workers + React RSC)       |
