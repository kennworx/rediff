## ADDED Requirements

### Requirement: Load a single file for the peek
The system SHALL load the content and diffs that the single-file peek needs: a file's full text at a given revision (for content mode), the working-copy text of a file, and a single-file unified diff computed at an arbitrary context level.

#### Scenario: File content at a revision
- **WHEN** the peek requests a file's content at a commit
- **THEN** the file's blob text at that commit is returned for full-content rendering

#### Scenario: Single-file diff at a context level
- **WHEN** the peek requests a file's diff at a given context level
- **THEN** the unified diff for that file is computed with that many surrounding context lines

#### Scenario: Missing or binary side
- **WHEN** the file is absent at the requested revision or is binary
- **THEN** the peek is told there is no content/diff to show rather than rendering garbage

### Requirement: Resolve the review top
The system SHALL resolve `TOP`, the newest side of the current review context, so the peek's history diff compares against it: the working copy for a working-tree or staged review, the target commit for a range review, and the commit itself for a single-commit view.

#### Scenario: Working-tree review top
- **WHEN** the active view is a working-tree review and the peek needs `TOP`
- **THEN** `TOP` resolves to the working copy

#### Scenario: Range review top
- **WHEN** the active view is a range review `base..target`
- **THEN** `TOP` resolves to the target commit
