# entries

Task runner for automating monthly entry generation. Designed to run in GitHub Actions cron jobs.

## Usage

```bash
# Generate monthly entry and create PR
cargo run -p task-entries -- generate
```

## GitHub Actions Execution

This program is designed to run in GitHub Actions environments.

### Required Environment Variables

- `GITHUB_REPOSITORY`: Repository name (e.g., `owner/repo`)
- `GITHUB_SHA`: Commit SHA
- `GITHUB_ACTIONS`: Flag indicating GitHub Actions environment (for error formatting)

### Timezone Handling

GitHub Actions runs in UTC, but this program uses **Asia/Tokyo (JST)** timezone for date calculations to ensure entries are generated for the correct month in Japanese time.

### Idempotency

The program is idempotent and safe to re-run. It checks for existing resources before creation:

- Checks if branch exists before creating
- Checks if PR exists before creating

### Verified Commits with GitHub Apps

GitHub Apps can use bots to sign commits. If a commit has a bot signature that is cryptographically verifiable, GitHub marks the commit as verified.

Signature verification for bots will only work if the request meets all of these conditions:

- Request is verified and authenticated as the GitHub App or bot
- Contains no custom author information
- Contains no custom committer information
- Contains no custom signature information (such as Commits API)

References:

- [About commit signature verification - Signature verification for bots](https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification#signature-verification-for-bots)
- [Commit signing support for bots and other GitHub Apps](https://github.blog/engineering/platform-security/commit-signing-support-for-bots-and-other-github-apps)

This is why the program does not specify custom author/committer information when creating commits via `gh api repos/{repo}/contents/{path}`.
