## ADDED Requirements

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
The system SHALL cache highlighting results per file so that re-displaying a file does not
recompute its highlighting.

#### Scenario: Re-display uses cache
- **WHEN** a file is scrolled away from and back into view
- **THEN** its highlighting is served from cache rather than recomputed

### Requirement: Full-coverage highlighting
Highlighting for a bundled language SHALL cover the file's syntax (keywords, strings, comments,
types, punctuation, and language-specific constructs), not a sparse subset.

#### Scenario: TSX is fully highlighted
- **WHEN** a TSX file is highlighted
- **THEN** base JavaScript, JSX, and TypeScript constructs are all highlighted (the query bundle combines the base, dialect, and injection queries)
