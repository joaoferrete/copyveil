# Secrets never touch disk; at-rest key lives only in the system keyring

Secret and Suspected Items are held in memory only (zeroized on expiry) and are never written to disk, whatever the settings. Plain Items are persisted, but each one is encrypted, because content heuristics will inevitably miss some secrets. The master key lives in the Secret Service keyring (GNOME Keyring / KWallet), unlocked at login. If no Secret Service is available, the app runs in memory-only mode and tells the user; it never falls back to a key file on disk, since a key next to the data it protects defeats the "stolen disk / leaked backup" threat model.

## Consequences

- Users without a keyring lose History on logout. This is deliberate.
- Demoting a Suspected Item to Plain is the only path for a once-suspected value to reach disk.
