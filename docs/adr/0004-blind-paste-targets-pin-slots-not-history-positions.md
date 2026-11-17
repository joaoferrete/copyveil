# Blind paste targets Pin slots, not History positions

Users can paste by number in two ways. Inside the popup, `Ctrl+1…9` pastes the Item at that position in the History. That's safe because the user can see what they're pasting. The global `Super+Alt+1…9`, which pastes without opening the popup, targets Pin slots instead of History positions. History positions shift with every copy, so a blind "paste position 2" would eventually paste a Secret into a chat window. Pins are stable, ordered by the user, and can only be Plain, so a blind paste can never paste a secret.

## Considered Options

- **Global shortcut by History position**: rejected. It's unpredictable, and it can paste Secret or Suspected Items without the user seeing them.
- **Global shortcut by History position, skipping masked Items**: rejected. Positions would still shift, and they would no longer match the numbers shown in the popup.
