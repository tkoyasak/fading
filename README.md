# fading

A Markdown dialect for journaling, and its tooling.

Fading organizes a journal into one file per calendar month. Each file opens
with a TOML frontmatter block and separates days with h6 headings, so a whole
month reads as plain CommonMark while staying machine-checkable.

```markdown
+++
id = "2026-01"
created = 2026-01-01
modified = 2026-01-15
+++

###### 2026-01-14 Wed

こんにちは．

###### 2026-01-15 Thu

<!-- -->
```

See [`docs/fading-spec.md`](docs/fading-spec.md) for the full language
specification.

## Packages

| Package                  | Description                                                     |
| ------------------------ | --------------------------------------------------------------- |
| [fading-cli](crates/cli) | Command-line interface (open, new, stats, get, put, notify, ls) |
| [fading-ls](crates/ls)   | Language server (sync, code actions, diagnostics)               |
| [fading-zed](crates/zed) | Zed editor extension                                            |
| [xtask](crates/xtask)    | Developer tasks (fixture generation)                            |

## License

[MIT](LICENSE)
