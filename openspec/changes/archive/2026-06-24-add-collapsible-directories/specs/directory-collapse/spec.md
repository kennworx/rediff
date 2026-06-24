## ADDED Requirements

### Requirement: Fold and unfold a directory
In the directory-grouped sidebar, the system SHALL let the user fold a directory
so its files are removed from the list and the diff body, and unfold it to
restore them. Folding SHALL be per directory (folding a directory does not fold
its subdirectories) and SHALL be available only in the grouped view. Folding and
unfolding SHALL be reachable by keyboard and by mouse.

#### Scenario: Fold a directory from the keyboard
- **WHEN** the cursor is on a file and the user presses the fold key
- **THEN** that file's directory folds — its files leave the sidebar list and the diff body — and the cursor moves to that directory's collapsed placeholder

#### Scenario: Unfold from the keyboard
- **WHEN** the cursor is on a collapsed directory's placeholder and the user presses the fold key
- **THEN** the directory unfolds — its files return to the list and the diff body — and the cursor moves to the directory's first file

#### Scenario: Fold by mouse
- **WHEN** the user clicks a directory header line or its collapsed placeholder
- **THEN** that directory's fold toggles

#### Scenario: Per-line, not subtree
- **WHEN** the user folds a directory that has subdirectories with their own files
- **THEN** only that directory's own files are hidden; its subdirectories remain visible as their own (separately foldable) lines

#### Scenario: Collapse and expand all
- **WHEN** the user invokes collapse-all (or expand-all)
- **THEN** every directory folds (or unfolds) at once

### Requirement: Collapsed directory presentation
A folded directory's header line SHALL remain non-selectable chrome, and its
files SHALL be replaced by a single selectable placeholder indicating how many
files are hidden, shown both in the sidebar and as a row in the diff body. The
folded directory's files SHALL NOT appear in the diff body (no file header, no
hunks).

#### Scenario: Placeholder replaces the files
- **WHEN** a directory is folded
- **THEN** its file rows are replaced by one placeholder line showing the hidden file count, and the directory header line is still shown but is not selectable

#### Scenario: Folded files are out of the diff body
- **WHEN** a directory is folded and the user scrolls the diff
- **THEN** none of that directory's file headers or hunks appear in the stream; a single placeholder marks where they were

### Requirement: Selection spans files and collapsed placeholders
The active selection SHALL be either a file or a collapsed-directory placeholder.
File-step navigation and the jump digits SHALL move the selection across visible
files and collapsed placeholders, skipping directory header lines. File actions
(toggle reviewed, peek, jump digits) SHALL apply only when a file is selected and
SHALL be inert when a collapsed placeholder is selected; the placeholder's action
SHALL be to unfold.

#### Scenario: Navigate onto a placeholder
- **WHEN** the user steps the selection past the files preceding a folded directory
- **THEN** the selection lands on that directory's collapsed placeholder

#### Scenario: File actions inert on a placeholder
- **WHEN** a collapsed placeholder is selected and the user presses a file action key (toggle reviewed or peek)
- **THEN** nothing happens (the action applies only to files)

#### Scenario: Jump digit targets a file, not a placeholder
- **WHEN** the user presses a jump digit
- **THEN** the selection lands on a visible file (collapsed placeholders are never jump-digit targets)

### Requirement: Auto-collapse a completed directory
The system SHALL automatically fold a directory, in a review session, when its
last unreviewed file is marked reviewed. This SHALL happen once, on the
completion transition; a directory the user unfolds again SHALL stay unfolded.

#### Scenario: Finishing a directory folds it
- **WHEN** the user marks the final unreviewed file in a directory as reviewed
- **THEN** that directory folds, removing its (now all-reviewed) files from the list and diff body

#### Scenario: Cursor advances after an auto-fold
- **WHEN** marking a file reviewed auto-folds its directory (hiding the file the cursor was on)
- **THEN** the selection advances to the next unviewed file (or, if none remain in view, to the new placeholder), so the review keeps flowing

#### Scenario: Manual unfold of a finished directory persists
- **WHEN** an auto-folded directory is unfolded by the user
- **THEN** it stays unfolded and does not re-fold on subsequent redraws

### Requirement: Jumping to a folded file unfolds it
The system SHALL unfold a folded directory and reveal a file inside it when the
user jumps to that file by path — via the fuzzy file-jump palette, a sidebar
click, or a landing target — so the jump always lands on the chosen file.
File-step navigation, by contrast, walks only visible files and never targets a
folded file.

#### Scenario: Fuzzy-jump into a folded directory
- **WHEN** the user fuzzy-jumps to a file whose directory is folded
- **THEN** that directory unfolds, the file is revealed, and it becomes the selection

#### Scenario: Step navigation does not unfold
- **WHEN** the user steps the selection with file-step navigation past a folded directory
- **THEN** the selection lands on the directory's placeholder (it does not unfold)

### Requirement: Collapse reduces review scope
Folded files SHALL be excluded from the diff body and skipped by file navigation
and next-unviewed, and the scroll percentage SHALL reflect the visible rows. The
overall reviewed count SHALL nonetheless include folded files (collapse is a view
filter, not exclusion). The set of folded directories SHALL be a property of the
view and SHALL persist as the user moves through the view history.

#### Scenario: Next-unviewed skips folded directories
- **WHEN** the user invokes next-unviewed and the only unreviewed files are inside folded directories
- **THEN** next-unviewed reports none remaining in view and indicates how many unreviewed files are hidden in folded directories (so the remainder reads as folded away, not lost)

#### Scenario: Reviewed count still includes folded files
- **WHEN** a directory is folded
- **THEN** its files still contribute to the reviewed-count total (and to the reviewed tally when they are reviewed)

#### Scenario: Fold state restored with the view
- **WHEN** the user folds directories, switches to another view, and returns
- **THEN** the same directories are folded as when the view was left
