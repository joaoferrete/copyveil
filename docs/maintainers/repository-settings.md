# Repository settings

These settings live in GitHub, not in the repository, so they're recorded here. Re-check them after any change to the CI job names.

## Rulesets on `main`

Two rulesets, so the maintainer can merge without an approval while nobody, maintainer included, can bypass the checks.

### `main-integrity` (no bypass for anyone)

- Restrict deletions.
- Block force pushes.
- Require linear history.
- Require signed commits. Squash merges made through GitHub are signed by GitHub, so contributors don't need their own signing keys.
- Require a pull request before merging (0 approvals in this ruleset).
- Require status checks to pass, with branches up to date:
  - `Format and clippy`
  - `Tests`
  - `Coverage gate (core, detect >= 90%)`
  - `Mutation testing (core, detect)`
  - `Licences, advisories, sources`
  - `Design tokens and contrast`
  - `GNOME extension lint`
  - `Meson build and data validation`
  - `Conventional Commits title`

### `main-review` (bypass: repository admin, pull requests only)

- Require a pull request with 1 approval.
- Require review from code owners.
- Dismiss stale approvals when new commits are pushed.
- Require conversation resolution.

## Merge options

- Allow squash merging only, with the PR title as the default commit message.
- Disable merge commits and rebase merging.
- Automatically delete head branches.

## Security

- Enable private vulnerability reporting.
- Enable Dependabot alerts and security updates.
- Enable secret scanning and push protection.
- Workflow permissions: read-only by default (each workflow asks for what it needs).

## Secrets

- `RELEASE_PLEASE_TOKEN`: a fine-grained token (or GitHub App token) with `contents: write` and `pull-requests: write` on this repository. Pushes made with `GITHUB_TOKEN` don't trigger workflows, so without it CI would never run on the release PR.

## Labels

Create `security-sensitive`, `design`, `docs`, `detector`, `bug` and `enhancement`. The labeler workflow applies the first three automatically.
