# Security

Copyveil's reason to exist is handling secrets with care, so security reports are the most valuable contributions this project gets.

## Reporting a vulnerability

Report privately through [GitHub Security Advisories](https://github.com/joaoferrete/copyveil/security/advisories/new). Do not open a public issue.

Include what an attacker can do, the steps to reproduce, and the affected version or commit. Never include a real secret; a made-up value with the same shape is enough.

You will get an acknowledgement within 7 days. Once a fix is released, the advisory is published with credit to you, unless you prefer otherwise.

## Supported versions

Before 1.0, only the latest release gets security fixes.

## Threat model

### What Copyveil protects against

- **Someone reading the disk**: a stolen laptop, a leaked backup of `~`, dotfiles synced to the cloud. Secret and Suspected Items never touch the disk. Plain Items are encrypted one by one with XChaCha20-Poly1305 under a key held in the Secret Service keyring ([ADR 0002](docs/adr/0002-secrets-never-touch-disk-and-keyring-only-at-rest-key.md), [ADR 0005](docs/adr/0005-plain-store-is-sqlite-with-per-item-xchacha20-poly1305.md)).
- **Someone looking at the screen**: shoulder surfing, screen sharing, screenshots. Secret and Suspected Items are masked. A Suspected Item shows only its first and last 4 characters, and only when it is at least 20 characters long, so you can tell tokens apart; a Secret shows nothing, not even its length. A Reveal lasts only while held, for at most 5 seconds. Screen readers announce that an Item is hidden, never its value.
- **Secrets that linger**: Secret and Suspected Items expire, and a Secret is also cleared from the system clipboard if it's still there. Expiry leaves no trace in the History.
- **Recovering a secret through search**: search matches Secret and Suspected Items only by their description, never their content.
- **Settings that weaken the defaults too far**: expiry times and the History size are configurable only within fixed limits (a Secret lives 10 s to 5 min), which the daemon enforces whatever client asks. Turning on memory-only erases the saved History and deletes its key from the keyring at once.

### What Copyveil does not protect against

- **Malicious code running as your user.** A process with your user's privileges can talk to your unlocked keyring, call the daemon's D-Bus API and watch the clipboard directly. No clipboard manager can defend against that, and Copyveil does not claim to. It does raise the cost: the daemon makes itself non-dumpable at start-up, so it leaves no core dump and other processes can't attach to it with `ptrace` or read its memory through `/proc`.
- **Other apps reading the clipboard.** While something is on the system clipboard, any app allowed to read it can. Copyveil shortens how long a Secret stays there, but cannot hide it from the compositor or from apps it grants access to.
- **Secrets the detectors miss.** Content detection is a heuristic. A secret that looks like ordinary text is stored as Plain, which is why Plain Items are encrypted too.
- **A keyring unlocked by your login.** Anyone who can log in as you can read Plain History.

## Guarantees enforced in code and CI

- `unsafe` code is forbidden across the workspace, and lints reject `println!`/`dbg!`-style output.
- From v0.1: Item content uses types that cannot be printed or logged, and a leak test runs on every PR to check that no Secret or Suspected value appears in anything written to disk ([docs/testing.md](docs/testing.md)).
- Dependencies are checked for licence compatibility and known advisories (`cargo-deny`), and releases ship with an SBOM and build provenance attestations.
- No telemetry of any kind.
