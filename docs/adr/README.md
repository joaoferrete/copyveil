# Architecture Decision Records

An ADR records a decision and why it was made, so nobody "fixes" something that was deliberate.

## When to write one

Write an ADR when all three are true:

1. **Hard to reverse**: changing course later costs real effort.
2. **Surprising without context**: a future reader would ask "why did they do it this way?"
3. **A real trade-off**: there were genuine alternatives, and this one won for specific reasons.

If any of the three is missing, skip it. Architectural shape, integration patterns, technology with lock-in, scope boundaries and deliberate deviations from the obvious usually qualify. Library choices and easily reversed details don't.

## Format

Files are `NNNN-short-slug.md`, numbered in sequence. An ADR can be a single paragraph:

```md
# {Short title of the decision}

{1-3 sentences: the context, what we decided, and why.}
```

Add sections only when they help:

- `Status` (`proposed | accepted | deprecated | superseded by ADR-NNNN`), when a decision is revisited.
- `Considered Options`, when the rejected alternatives are worth remembering.
- `Consequences`, when the downstream effects aren't obvious.

ADRs are not edited to reverse a decision. Write a new ADR that supersedes the old one, and mark the old one as superseded.

## Index

- [0001: Desktop-agnostic core with pluggable capture sources](0001-desktop-agnostic-core-with-pluggable-capture-sources.md)
- [0002: Secrets never touch disk; at-rest key lives only in the system keyring](0002-secrets-never-touch-disk-and-keyring-only-at-rest-key.md)
- [0003: Design tokens as single source; Sensitivity shown by shape](0003-design-tokens-as-single-source-and-sensitivity-by-shape.md)
- [0004: Blind paste targets Pin slots, not History positions](0004-blind-paste-targets-pin-slots-not-history-positions.md)
