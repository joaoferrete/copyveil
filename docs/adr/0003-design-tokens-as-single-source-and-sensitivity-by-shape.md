# Design tokens as single source; Sensitivity shown by shape, brand kept out of app chrome

The design system lives in code, not in a design tool. `design/tokens.json` is the single source; it generates the GTK stylesheet and the website/README styles, and CI checks WCAG AA contrast in both light and dark themes. The app chrome follows the user's system accent colour. The brand palette ("security envelope": Ink, Sheet, Slate `#34465F`, the security-tint pattern, Kraft `#C49A5C` for the icon only, Marker `#F1DF74` for search matches only) appears only in the icon, Sensitivity states, onboarding and marketing. A Mask shows a Secret as a solid bar of fixed width, so the value's length doesn't leak, and a Suspected Item as the hatched security tint with its first and last characters visible. Because the two are told apart by shape, not only by colour, the distinction holds for colour-blind users, in monochrome panels and in high-contrast themes. In dark theme the bar turns light, like correction tape.

## Considered Options

- **Brand-coloured app chrome**: rejected. It fights the user's desktop and looks foreign next to other GNOME apps.
- **Figma as source of truth**: rejected. It's proprietary, lives outside the repo, and drifts from the shipped UI.
- **"Redaction" palette (cream paper, black, vermilion)**: rejected after review. The idea was right, but that combination is one of the most common looks in generated design. The security envelope keeps the idea and grounds it in a real object.
