## ADDED Requirements

### Requirement: Single-file peek overlay
The system SHALL provide a modal, full-area, scrollable, syntax-highlighted overlay showing exactly one file, opened from the selected file. The overlay SHALL be ephemeral: it captures input while open, does not create a view-history entry, and closing it SHALL restore the previous view unchanged. The overlay SHALL NOT provide viewed-tracking.

#### Scenario: Open and close
- **WHEN** the user opens the peek for a file and then presses Esc
- **THEN** the peek closes and the previous view is shown exactly as before

#### Scenario: No history entry
- **WHEN** the peek is open and the user closes it
- **THEN** the view-history back/forward state is unchanged (the peek created no entry)

#### Scenario: Scrolls and highlights
- **WHEN** the peek shows a long file
- **THEN** its content is syntax-highlighted and can be scrolled independently of the main view

### Requirement: Content and diff modes
The peek SHALL have two modes — content (the whole file, no diff markers) and diff (a unified diff for the file) — and Tab SHALL toggle between them in place.

#### Scenario: Toggle modes
- **WHEN** the user presses Tab in the peek
- **THEN** the peek switches between showing the full file content and showing the file's diff

#### Scenario: Content mode shows the whole file
- **WHEN** the peek is in content mode
- **THEN** every line of the file is shown with line numbers and highlighting and no add/remove markers

### Requirement: History and review open keys
The system SHALL open the peek from the selected file with two keys whose diffs share the same end point (`TOP`, the newest side of the current review context) but differ in start point:
- `p` (history) SHALL open in content mode showing the file at the commit being viewed, with its diff being that commit versus `TOP`.
- `=` (review) SHALL open in diff mode showing the view's own change for the file (its base versus `TOP`), with the context level expanded beyond the main view's.

#### Scenario: History peek from a commit
- **WHEN** the user presses `p` on a file while viewing a commit
- **THEN** the peek shows that commit's version of the file, and toggling to diff shows the change from that commit to `TOP`

#### Scenario: Review peek anchors at TOP
- **WHEN** the user presses `=` on a file
- **THEN** the peek shows the file's own change diff (base to `TOP`) with expanded context

#### Scenario: TOP follows the review context
- **WHEN** the active view is a range review `base..target`
- **THEN** the peek's diffs end at the target commit, not at the working copy

### Requirement: Adjustable diff context
In diff mode the peek SHALL expand the surrounding context with `=`/`+` and compact it with `-`/`_`, rebuilding the diff at the new context level. The level SHALL be clamped between a minimal hunk view and the whole file.

#### Scenario: Expand context
- **WHEN** the user presses `=` in diff mode
- **THEN** more unchanged lines are shown around each change

#### Scenario: Compact context
- **WHEN** the user presses `-` in diff mode
- **THEN** fewer unchanged lines are shown around each change

### Requirement: Source color
The peek SHALL inherit the origin view's source accent: blue when opened from a local or staged view, green/magenta (the commit accent) when opened from a commit or range view.

#### Scenario: Local origin is blue
- **WHEN** the peek is opened from a working-tree or staged view
- **THEN** its frame uses the local (blue) accent

#### Scenario: Commit origin is the commit accent
- **WHEN** the peek is opened from a commit or range view
- **THEN** its frame uses the commit accent
