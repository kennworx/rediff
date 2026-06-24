# theme-selection Specification

## Purpose
TBD - created by archiving change adopt-theme-set-and-picker. Update Purpose after archive.
## Requirements
### Requirement: Theme picker overlay
The system SHALL provide a theme picker overlay listing the available themes in a multi-column grid, summoned over the current view and capturing input until dismissed.

#### Scenario: Open the picker
- **WHEN** the user presses the theme key from the normal view
- **THEN** a multi-column grid of the available themes opens with the cursor on the currently active theme

#### Scenario: Single overlay at a time
- **WHEN** the theme picker is open
- **THEN** it captures input and no other overlay is shown simultaneously

### Requirement: Grid navigation
The system SHALL let the user move the picker cursor with both arrow keys and `hjkl`, where horizontal keys move by one entry and vertical keys move by one row, clamped to the bounds of the grid.

#### Scenario: Horizontal movement
- **WHEN** the user presses `l` or the right arrow
- **THEN** the cursor moves to the next theme in the grid

#### Scenario: Vertical movement
- **WHEN** the user presses `j` or the down arrow
- **THEN** the cursor moves down by one row (one column-count of entries)

#### Scenario: Movement is clamped
- **WHEN** the cursor is at an edge and the user presses a key that would move past the first or last entry
- **THEN** the cursor stays within the valid range

### Requirement: Live preview on selection
The system SHALL apply the theme under the cursor to the entire UI immediately as the cursor moves, so the view behind the picker re-renders in the highlighted theme before the choice is committed.

#### Scenario: Preview follows the cursor
- **WHEN** the user moves the cursor to a different theme in the grid
- **THEN** the whole UI (including the diff content behind the picker) re-renders with that theme's colors

### Requirement: Commit applies and persists
The system SHALL, when the user confirms the highlighted theme, keep it as the active theme and persist it as the configured default.

#### Scenario: Confirm a theme
- **WHEN** the user presses Enter on a highlighted theme
- **THEN** the picker closes, that theme remains active, and it is written to the configuration file

### Requirement: Cancel rolls back
The system SHALL, when the user cancels the picker, restore the theme that was active when the picker was opened and SHALL NOT persist any change.

#### Scenario: Cancel after previewing
- **WHEN** the user has previewed one or more themes and then presses Esc
- **THEN** the picker closes, the theme reverts to the one active when the picker opened, and the configuration file is unchanged

