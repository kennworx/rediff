# syntax highlighting

## Purpose

Highlight diff content per file off the input thread — tree-sitter for bundled languages, syntect as the breadth fallback — with colors sourced from the active theme so highlighting follows theme changes without blocking navigation.
## Requirements
### Requirement: Non-blocking highlighting
The system SHALL compute syntax highlighting off the input thread so that navigation and
scrolling never block on highlighting.

#### Scenario: Scroll into un-highlighted file
- **WHEN** the user scrolls into a file whose highlighting is not yet computed
- **THEN** the content renders immediately as plain text and is replaced with highlighted content once ready, without blocking input

### Requirement: Pluggable highlighter
The system SHALL highlight via a single highlighter abstraction with tree-sitter as the primary
engine and syntect as a fallback for languages without a bundled tree-sitter grammar.

#### Scenario: Bundled language uses tree-sitter
- **WHEN** a file's language has a bundled tree-sitter grammar
- **THEN** the system highlights it with tree-sitter

#### Scenario: Unbundled language falls back
- **WHEN** a file's language has no bundled tree-sitter grammar but is known to syntect
- **THEN** the system highlights it with syntect

#### Scenario: Unknown language
- **WHEN** a file's language is recognized by neither engine
- **THEN** the content is shown as plain text without error

### Requirement: Per-file highlight cache
The system SHALL cache highlighting results per file so that re-displaying a file does not recompute its highlighting, keyed such that a change of active theme yields the correct colors for already-cached content.

#### Scenario: Re-display uses cache
- **WHEN** a file is scrolled away from and back into view without a theme change
- **THEN** its highlighting is served from cache rather than recomputed

#### Scenario: Theme change reflects new colors
- **WHEN** the active theme changes to a different theme
- **THEN** visible content reflects the new theme's colors (cached results are not served with the previous theme's colors)

### Requirement: Full-coverage highlighting
Highlighting for a bundled language SHALL cover the file's syntax (keywords, strings, comments,
types, punctuation, and language-specific constructs), not a sparse subset.

#### Scenario: TSX is fully highlighted
- **WHEN** a TSX file is highlighted
- **THEN** base JavaScript, JSX, and TypeScript constructs are all highlighted (the query bundle combines the base, dialect, and injection queries)

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

