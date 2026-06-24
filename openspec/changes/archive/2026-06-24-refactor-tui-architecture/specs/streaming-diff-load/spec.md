## ADDED Requirements

### Requirement: Snapshot file set per view
A view's enumerated file set SHALL be fixed for the lifetime of the view. Switching away from a view and returning SHALL NOT re-enumerate its files or change their identity or order.

#### Scenario: File list unchanged on return
- **WHEN** the user switches away from a view whose diff is still loading and later returns to it
- **THEN** the file list (paths and order) is identical to what it was, and no re-enumeration is performed

### Requirement: Resume preserves completed diffs
When a view's background load is abandoned (by switching away) and the view is later shown again, the system SHALL preserve the diffs that already completed and SHALL re-compute only the files that had not yet been diffed.

#### Scenario: Returning resumes only the remainder
- **WHEN** a load had completed all but a few files when the user switched away, and the user returns to that view
- **THEN** only the not-yet-diffed files are computed; the already-diffed files are not re-diffed

#### Scenario: No from-scratch redo
- **WHEN** the user switches away near the end of a large load and returns
- **THEN** the view does not re-diff the whole changeset from the beginning

## MODIFIED Requirements

### Requirement: Cancel during load
The system SHALL let the user cancel an in-progress load with Esc or q. At startup this SHALL quit; for an in-session view switch it SHALL return to the previous view. Abandoning a load by switching away SHALL retain the diffs completed so far on the abandoned view, so that returning resumes rather than restarts.

#### Scenario: Cancel at startup quits
- **WHEN** the initial load is in progress and the user presses Esc or q
- **THEN** rsdiff stops loading, restores the terminal, and exits

#### Scenario: Cancel a switch returns to the previous view
- **WHEN** a load triggered by switching to another commit/branch is in progress and the user cancels
- **THEN** the load is abandoned and the previous view is shown

#### Scenario: Switching away retains progress
- **WHEN** the user switches away from a view that is partway through loading
- **THEN** the completed diffs on that view are retained so a later return resumes from where it stopped
