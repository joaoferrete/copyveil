# The Plain store is SQLite with each Item sealed by XChaCha20-Poly1305

Plain Items are stored in one SQLite file, `history.db`, with one row per Item. The row's id is the only column in clear. The Item's metadata (order, capture time, Pin slot, Override, kind) and its content are sealed in two separate values with XChaCha20-Poly1305 under the History key from the keyring. Each value gets a fresh random 192-bit nonce. Its associated data names the column and the Item id, so a value copied to another row or column fails to decrypt instead of showing up as a different Item. Metadata is sealed separately so that pinning or copying an Item again rewrites a few bytes, not an 8 MB image.

The file is created with mode 0600 and SQLite's `secure_delete` is on, so freed pages are zeroed. Plain Items above 8 MB (content and plain-text fallback together) stay in memory only. When the file can't be opened with the key, because the keyring entry was deleted or recreated or the file is corrupt, the store erases it, starts empty and the app tells the user. It never asks for a password and never tries another key. A single row that fails to decrypt is erased and counted.

The keyring client is `oo7`. Inside a Flatpak, `oo7` keeps the key in an encrypted file whose own key comes from the Secret portal. That still meets ADR 0002, because the key that opens the file lives in the host keyring.

## Considered Options

- **One encrypted file, rewritten atomically on every change**: rejected. It leaks even less metadata, but with images, every copy rewrites the whole History.
- **One encrypted file per Item**: rejected. File sizes and modification times reveal the same metadata the SQLite rows hide, and the store has to handle crash consistency itself.
- **SQLCipher (whole-database encryption)**: rejected. It's a C fork to build and audit, and it encrypts pages rather than Items, so a wrong key can't be told apart from a corrupt file.
- **AES-256-GCM**: rejected. Its 96-bit nonce is risky with random nonces at this volume, and it's slow without AES instructions. XChaCha20-Poly1305 has neither problem.
- **`secret-service` or `keyring` crates**: rejected. Neither talks to the Secret portal, which the Flatpak needs in v0.2.

## Consequences

- Deleting the "Copyveil History key" entry in Seahorse is a valid way to erase the History.
- The number of stored Items and their approximate sizes are visible to anyone who can read the file.
