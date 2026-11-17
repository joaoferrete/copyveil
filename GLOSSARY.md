# Copyveil

Copyveil is a clipboard history manager for Linux whose defining trait is handling secrets (passwords, API keys, tokens) with care. Primary audience: developers.

## Language

### History

**Item**:
A piece of content copied to the clipboard and captured by the app.
_Avoid_: Clip, entry, copy

**History**:
The ordered collection of Items the user can browse and paste again.
_Avoid_: List, log

**Pin**:
A Plain Item the user chose to keep indefinitely at the top of the History. Secret and Suspected Items cannot be pinned.
_Avoid_: Favorite, star, bookmark

**Pin slot**:
The position of a Pin (1 to 9) in the user's chosen order, which a global shortcut pastes without opening the History.
_Avoid_: Quick slot, favorite number

**Expiry**:
The moment a Secret or Suspected Item is erased from memory without leaving any trace in the History.
_Avoid_: Timeout, TTL

### Sensitivity

**Sensitivity**:
The classification of an Item that decides how it is stored, displayed and expired. Values: Plain, Suspected, Secret.

**Secret**:
An Item its source explicitly declared as secret (e.g. a password manager flagged it).
_Avoid_: Password (just one kind of Secret)

**Suspected**:
An Item the app judges, from its content alone, might be a secret (token, key, high-entropy text), without confirmation from the source.
_Avoid_: Likely secret, sensitive

**Plain**:
An Item with no indication of being a secret.
_Avoid_: Normal, common, safe

**Mask**:
The redaction bar that hides a Secret or Suspected Item's content in the UI: solid for Secret, hatched for Suspected.
_Avoid_: Blur, censor, veil

**Reveal**:
Temporarily showing a Masked Item's content while the user holds the reveal control.
_Avoid_: Unmask, show

**Override**:
The user changing an Item's Sensitivity by hand, either up (promote) or down (demote).
_Avoid_: Manual tag, reclassify
