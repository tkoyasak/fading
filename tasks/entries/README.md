# task-entries

Task runner for automating monthly entry generation. Designed to run in GitHub Actions cron jobs.

## Prerequisites

- [GitHub CLI (`gh`)](https://cli.github.com/)

## Usage

```bash
cargo run -p task-entries -- generate
```

## Configuration

| Variable            | Description                                            |
| ------------------- | ------------------------------------------------------ |
| `GH_TOKEN`          | GitHub authentication token (requires repo scope)      |
| `GITHUB_REPOSITORY` | Repository name (e.g., `owner/repo`)                   |
| `GITHUB_SHA`        | Commit SHA                                             |
| `GITHUB_ACTIONS`    | GitHub Actions environment flag (for error formatting) |

## Behavior

- **Idempotent**: Checks for existing branches and PRs before creation
- **Verified commits**: No custom author/committer information is specified, enabling [bot signature verification](https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification#signature-verification-for-bots)
