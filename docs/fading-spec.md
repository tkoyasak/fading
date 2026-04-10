# Fading Spec

Fading is a Markdown dialect for journaling. Each file covers one calendar month.

## File Layout

```
entries/
  YYYY-MM.md
  YYYY-MM.md
  ...
```

Each file is named `YYYY-MM.md` (e.g. `2026-01.md`) and contains a TOML frontmatter block followed by day entries.

## Structure

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

A file consists of two parts:

1. **Frontmatter** — TOML block enclosed in `+++` delimiters
2. **Entries** — day sections separated by h6 headings

## Frontmatter

The frontmatter block must appear at the very start of the file.

```toml
id = "YYYY-MM"
created = YYYY-MM-DD
modified = YYYY-MM-DD
```

| Field      | Type         | Description                                                           |
| ---------- | ------------ | --------------------------------------------------------------------- |
| `id`       | `"YYYY-MM"`  | Month identifier; must match the filename                             |
| `created`  | `YYYY-MM-DD` | Date the file was first created                                       |
| `modified` | `YYYY-MM-DD` | Date the file was last modified (auto-updated by the language server) |

### Validation rules

- `id` must be in `YYYY-MM` format and match the filename (e.g. `2026-01.md` → `id = "2026-01"`)
- `created` and `modified` must be valid calendar dates
- `created` must not be later than `modified`

## Day Entries

Each day is introduced by an h6 heading:

```
###### YYYY-MM-DD Day
```

- Only h6 headings (`######`) are allowed
- The heading text must be `YYYY-MM-DD Day` where `Day` is the abbreviated weekday (Mon, Tue, … Sun)
- Content follows the heading until the next heading or end of file

### Empty days

Days with no content use `<!-- -->` as a placeholder:

```markdown
###### 2026-01-01 Thu

<!-- -->

###### 2026-01-02 Fri

今日は良い日だった．
```

The placeholder preserves blank lines when running a Markdown formatter (e.g. oxfmt). Days whose content is only `<!-- -->` are treated as empty and excluded from KV sync.

## Formatting

Fading files are CommonMark-compatible and can be formatted with standard Markdown formatters. The `<!-- -->` placeholder ensures empty days survive formatting without losing their visual separation.
