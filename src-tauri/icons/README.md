# Menu bar template

`tray-template.png` is a 46×36 RGBA asset (2× the logical menu bar size).
The key, opposing arrows and three sparkles come from Keyturn's existing app
logo. The blue square is absent; transparent arrow cutouts retain the logo's
identity. macOS `icon_as_template(true)` supplies the appropriate menu bar color.
The full-color application icons stay separate.

Created with the built-in imagegen tool from `src/assets/app-icon@2x.png`, then
cropped to remove excess transparent margin and resized with `sips`.

Final prompt:

> Edit the referenced Keyturn app logo into a precise monochrome macOS menu bar template icon. Preserve the exact recognizable horizontal key silhouette, the two opposing switch arrows CUT OUT of the key bow as transparent negative space, and the three four-point sparkles in their original relative positions. Remove the blue rounded square completely; the background must be fully transparent. All remaining glyph pixels must be solid black, with clean antialiased edges. Flat vector-like silhouette, no gradient, no shadows, no frame, no text, no rectangle, no decorations. Center the entire key-plus-sparkles mark in a square canvas with only a small transparent margin, approximately 8% on each side, so it reads at 22px menu bar size. Faithfully retain logo geometry; do not invent another key design.
