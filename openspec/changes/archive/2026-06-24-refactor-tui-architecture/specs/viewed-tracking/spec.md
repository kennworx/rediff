## ADDED Requirements

### Requirement: Stable file identity within a view
A view's set of files SHALL have stable identity for the lifetime of the view: reviewed state and the active-file cursor SHALL stay aligned to the files they refer to, and reviewed-tracking operations SHALL never index outside the current file set. The reviewed-state collection SHALL always match the view's file count.

#### Scenario: Next-unviewed never goes out of bounds
- **WHEN** a working-tree review is left mid-load and returned to after the working tree has changed, and the user invokes next-unviewed
- **THEN** the command operates within the current file set without error and lands on an unreviewed file (or reports none remaining)

#### Scenario: Reviewed flags stay with their files
- **WHEN** a review session's file set is unchanged between leaving and returning to the view
- **THEN** each file's reviewed flag is exactly as it was left

## MODIFIED Requirements

### Requirement: Jump to next unviewed
The system SHALL provide a command to jump to the next file that has not been marked reviewed. The command SHALL operate only over the current view's file set and SHALL remain within bounds even if the view was re-entered after its underlying changes were refreshed.

#### Scenario: Skip reviewed files
- **WHEN** the user invokes next-unviewed while some files are marked reviewed
- **THEN** the stream jumps to the next file that is not marked reviewed

#### Scenario: All reviewed
- **WHEN** the user invokes next-unviewed and every file is reviewed
- **THEN** the system indicates there are no unviewed files remaining

#### Scenario: Safe after a view is re-entered
- **WHEN** the user invokes next-unviewed in a review session that was re-entered after a load was abandoned and resumed
- **THEN** the command completes without panicking, regardless of whether the file count grew or shrank
