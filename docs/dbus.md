# D-Bus API

`copyveil-daemon` owns `io.github.joaoferrete.Copyveil.Daemon` on the session bus and serves two interfaces at `/io/github/joaoferrete/Copyveil/Daemon`:

- **`History1`**, for the app.
- **`Capture1`**, for capture sources (the GNOME Shell extension, `ext-data-control` later).

Item ids are `t` (unsigned 64-bit) and last as long as the daemon runs; Plain Items keep theirs across restarts. The `1` in each interface name is its version. A breaking change adds a new interface instead of changing an existing one.

The daemon is D-Bus activatable, so calling it starts it. Rust clients use the proxies, names and `ItemSummary` from the `copyveil-dbus` crate.

## Content stays put

Signals carry Item ids only. `List` never includes the content of a Secret or Suspected Item. Content leaves the daemon only in the reply to `History1.Content`, for one Item the caller names: when the user Reveals or pastes it.

Any process running as the user can call these methods. This is inside the threat model in [`SECURITY.md`](../SECURITY.md): code running as you can already read the clipboard and the daemon's memory.

## `io.github.joaoferrete.Copyveil.History1`

| Member | Signature | What it does |
|---|---|---|
| `List()` | `→ a(tssssuttts)` | Every Item, newest first. See [Item summary](#item-summary). |
| `Search(query)` | `s → at` | Ids of matching Items, newest first. Masked Items match only on their description. |
| `Content(id)` | `t → (ayay)` | The Item's content and its plain-text fallback (empty when there is none). |
| `PinSlot(slot)` | `u → t` | The id of the Item in Pin slot `slot`, counting from 1. |
| `Paste(id)` | `t → ()` | Asks the capture source to paste the Item: emits `Capture1.PasteRequested`. |
| `Remove(id)` | `t → ()` | Removes the Item. |
| `Pin(id)` | `t → ()` | Pins a Plain Item into the next free Pin slot. |
| `Unpin(id)` | `t → ()` | Unpins the Item. Later Pins move up a slot. |
| `MovePin(id, slot)` | `tu → ()` | Moves a pinned Item to Pin slot `slot`. |
| `Override(id, sensitivity)` | `ts → ()` | Sets the Sensitivity: `plain`, `suspected` or `secret`. |
| `Changed` | signal `()` | The History changed. Call `List` again. |
| `OnDisk` | property `b` | Whether Plain Items are kept on disk (`true`) or in memory only. |
| `StorageNotice` | property `s` | What the user should be told about storage, or empty. See [Storage notices](#storage-notices). |

### Item summary

Each element of `List` is a struct with these fields, in order:

| Field | Type | Meaning |
|---|---|---|
| `id` | `t` | Item id |
| `kind` | `s` | `text`, `rich-text` or `image` |
| `mime` | `s` | MIME type of an image; empty otherwise |
| `sensitivity` | `s` | `plain`, `suspected` or `secret` |
| `reason` | `s` | Why it has its Sensitivity ("Looks like a GitHub token"), or empty |
| `pin_slot` | `u` | Pin slot counting from 1; 0 when not pinned |
| `age_ms` | `t` | Milliseconds since it was last copied |
| `expires_in_ms` | `t` | Milliseconds until its Expiry; 0 for Plain Items |
| `size` | `t` | Content size in bytes |
| `preview` | `s` | The first 300 characters of a Plain Item's text. Always empty for Secret and Suspected Items and for images |

### Storage notices

| Notice | Meaning |
|---|---|
| `keyring-unavailable` | No usable keyring, so the History is kept in memory only. |
| `store-unavailable` | The store couldn't be opened or read, so the History is kept in memory only. |
| `store-wiped` | The store couldn't be opened with the key, so it was erased and started empty ([ADR 0005](adr/0005-plain-store-is-sqlite-with-per-item-xchacha20-poly1305.md)). |
| `items-unreadable` | Some stored Items couldn't be decrypted and were erased. |
| `sync-failed` | Writing to disk failed. The next change retries. |

## `io.github.joaoferrete.Copyveil.Capture1`

| Member | Signature | What it does |
|---|---|---|
| `Capture(mime, content, plain_fallback, password_manager_hint)` | `sayayb → t` | Reports a clipboard change and returns the Item's id. Keep the id: it is how `Expired` tells you to clear the clipboard. |
| `Expired(ids)` | signal `at` | These Items reached their Expiry and are gone. If the clipboard still holds one of them, clear it. |
| `PasteRequested(id)` | signal `t` | The user asked to paste this Item. Fetch it with `History1.Content`, put it on the clipboard and paste. |

`mime` is the type the source chose from what the clipboard offered:

- `text/plain` (with or without parameters), `UTF8_STRING`, `STRING` or `TEXT` for text.
- `text/html` for rich text, with its plain-text version in `plain_fallback`.
- `image/*` for images.

Set `password_manager_hint` when the clipboard offered `x-kde-passwordManagerHint` with the value `secret`. Fail safe: when the hint is offered but its value can't be read or is empty, as Mutter sometimes hands it over, set it too. Only a readable value other than `secret` doesn't count. Other types, and empty content, are refused with `InvalidArgument`.

## Errors

Errors are named `io.github.joaoferrete.Copyveil.Error.<Name>`. Their messages name Item ids, never content.

| Name | When |
|---|---|
| `UnknownItem` | No Item has this id. It may have expired. |
| `NotPlain` | Only Plain Items can be pinned. |
| `NotPinned` | The Item isn't pinned. |
| `NoSuchPinSlot` | There is no Pin slot with that number. |
| `InvalidArgument` | Malformed argument: unknown MIME type, empty content or unknown Sensitivity. |
