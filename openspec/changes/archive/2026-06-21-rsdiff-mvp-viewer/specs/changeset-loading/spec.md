## ADDED Requirements

### Requirement: Load working-tree changes
The system SHALL load the git working-tree changes for `rsdiff diff` into one normalized
changeset of files and hunks, including staged and unstaged modifications and untracked files.

#### Scenario: Mixed working-tree changes
- **WHEN** the repository has a staged modification, an unstaged modification, and an untracked file
- **THEN** the changeset contains one file entry for each, with hunks computed from the appropriate old/new content

#### Scenario: Untracked files included by default
- **WHEN** `rsdiff diff` runs and untracked files are present
- **THEN** those untracked files appear in the changeset (matching hunk's default, unlike `git diff`)

#### Scenario: Exclude untracked on request
- **WHEN** `rsdiff diff --exclude-untracked` runs
- **THEN** untracked files are omitted and only tracked changes appear

### Requirement: Load staged-only changes
The system SHALL load only staged changes (HEAD vs index) for `rsdiff diff --staged`.

#### Scenario: Staged diff
- **WHEN** `rsdiff diff --staged` runs with a staged modification and an unstaged modification to the same file
- **THEN** the changeset reflects only the staged (HEAD-to-index) difference

### Requirement: Load a commit or range
The system SHALL load the changes introduced by a commit for `rsdiff show [ref]` (defaulting to
HEAD) and by a range expression.

#### Scenario: Show a commit
- **WHEN** `rsdiff show HEAD` runs
- **THEN** the changeset contains the diff between HEAD's parent tree and HEAD's tree

#### Scenario: Show an earlier commit
- **WHEN** `rsdiff show <ref>` runs for a non-HEAD ref
- **THEN** the changeset reflects that commit's changes

### Requirement: Decode renames
The system SHALL detect renames and copies and render them with their source and destination
paths rather than as an unrelated add and delete.

#### Scenario: Rename with edits
- **WHEN** a file is renamed and modified
- **THEN** the file entry shows the source path, the destination path, a "renamed" indication, and the content diff between the two sides

#### Scenario: Pure rename
- **WHEN** a file is renamed with no content change
- **THEN** the file entry shows the rename with source and destination paths and an empty body

### Requirement: Produce git-faithful diffs
Hunk bodies and hunk headers produced by the system SHALL match the content that git produces
for the same change.

#### Scenario: Body matches git
- **WHEN** a file is modified and loaded into the changeset
- **THEN** the added/removed lines and hunk header ranges match `git diff` for that file
