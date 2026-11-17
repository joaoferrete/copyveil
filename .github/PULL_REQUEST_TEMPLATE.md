<!--
Title: Conventional Commits, e.g. `feat(detect): flag Slack bot tokens`.
The title becomes the squash commit on main and the CHANGELOG line.
-->

## What and why

<!-- What changes for the user or the codebase, and why. Link the issue: Closes #123 -->

## How it was tested

<!-- Commands run, cases added, what you checked by hand on which desktop. -->

## Security impact

<!--
Required when the PR is labelled `security-sensitive`. Answer each line, or "none" with a reason.
-->

- Can Item content reach a log, panic message, file, D-Bus signal or the UI unmasked?
- Does this change how or where anything is persisted, encrypted or expired?
- New dependency? Name it and why it is worth the supply-chain cost.

## Checklist

- [ ] Tests cover the change; `just ci` passes locally
- [ ] Terms follow `GLOSSARY.md` (code, UI strings, docs)
- [ ] Docs updated: rustdoc on public API, `docs/` for behaviour, `docs/design/` for new components
- [ ] ADR added in `docs/adr/` if this is hard to reverse, surprising, and a real trade-off
- [ ] UI changes: screenshots in light and dark theme
