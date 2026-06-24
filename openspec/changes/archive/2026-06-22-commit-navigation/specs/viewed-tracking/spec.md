## ADDED Requirements

### Requirement: Per-view review sessions
Reviewed state SHALL be a property of a view rather than a single global. A view launched as working-tree, staged, or `review` changes SHALL be a review session that carries reviewed state; a view reached by browsing into a commit SHALL NOT be a review session. Reviewed-tracking commands (`v`, next-unviewed, collapse, the reviewed count) SHALL act on the current view only when it is a review session, and SHALL be inert otherwise. A review session's progress SHALL persist while the user browses other views and SHALL be restored when that view is shown again. The system SHALL let the user promote the current browse view into a review session with `R`.

#### Scenario: Reviewed commands inert while browsing
- **WHEN** the current view is a browsed commit (not a review session)
- **THEN** pressing `v` or next-unviewed has no effect and no reviewed count is shown

#### Scenario: Review progress survives browsing
- **WHEN** the user marks files reviewed in a review session, browses into a commit, and returns to the review session
- **THEN** the previously reviewed files are still marked reviewed

#### Scenario: Promote a commit to a review
- **WHEN** the user presses `R` on a browsed commit view
- **THEN** that view becomes a review session with its own reviewed state and reviewed-tracking commands take effect

## MODIFIED Requirements

### Requirement: Mark files reviewed
The system SHALL let the user mark a file as reviewed and unmark it within the current review session, and SHALL show each file's reviewed state in the sidebar. Marking SHALL have no effect when the current view is not a review session.

#### Scenario: Toggle reviewed
- **WHEN** the user marks the current file as reviewed in a review session
- **THEN** the sidebar shows that file as reviewed, and unmarking restores the unreviewed state

#### Scenario: Marking inert outside a review session
- **WHEN** the current view is a browse view (not a review session) and the user attempts to mark a file reviewed
- **THEN** nothing changes
