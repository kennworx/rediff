## MODIFIED Requirements

### Requirement: Per-file highlight cache
The system SHALL cache highlighting results per file so that re-displaying a file does not recompute its highlighting, keyed such that a change of active theme yields the correct colors for already-cached content.

#### Scenario: Re-display uses cache
- **WHEN** a file is scrolled away from and back into view without a theme change
- **THEN** its highlighting is served from cache rather than recomputed

#### Scenario: Theme change reflects new colors
- **WHEN** the active theme changes to a different theme
- **THEN** visible content reflects the new theme's colors (cached results are not served with the previous theme's colors)

## ADDED Requirements

### Requirement: Theme-sourced highlight colors
The system SHALL source highlight colors for both the tree-sitter and syntect paths from the active theme: the syntect path SHALL use the active theme directly, and the tree-sitter path SHALL map its capture names to the active theme's colors via a fixed capture-to-scope mapping.

#### Scenario: Tree-sitter colors come from the active theme
- **WHEN** a bundled-language file is highlighted under a given theme
- **THEN** each capture (keywords, strings, comments, types, etc.) is colored from that theme's resolved colors, not a separate hand-coded palette

#### Scenario: Both paths follow the same theme
- **WHEN** the active theme changes
- **THEN** both tree-sitter-highlighted and syntect-highlighted files render in the new theme's colors

### Requirement: Live theme switching for bundled languages
The system SHALL switch the colors of already-highlighted bundled-language content when the active theme changes without blocking input, so that previewing themes re-colors visible content responsively.

#### Scenario: Preview re-colors visible content
- **WHEN** the user moves the cursor across themes in the theme picker
- **THEN** visible bundled-language content re-colors to each previewed theme without blocking input
