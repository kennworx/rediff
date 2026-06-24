## ADDED Requirements

### Requirement: Single multi-file review stream
The system SHALL render all changed files as one continuous top-to-bottom review stream in the
changeset's file order.

#### Scenario: Multiple files shown in order
- **WHEN** a changeset with several files is opened
- **THEN** every file's diff appears in one scrollable stream, in changeset order, each under its own file header

#### Scenario: Selecting a file does not collapse the stream
- **WHEN** a file is selected from the sidebar
- **THEN** the stream scrolls to that file's position and continues to show all other files (it does not reduce to a single-file view)

### Requirement: Windowed rendering
The system SHALL render only the rows visible in the viewport (plus a small overscan) so
performance does not degrade with changeset size.

#### Scenario: Large changeset stays responsive
- **WHEN** a changeset with thousands of diff rows is scrolled
- **THEN** only viewport-visible rows are rendered each frame and scrolling remains smooth

### Requirement: Scrolling
The system SHALL support scrolling the review stream by keyboard and mouse wheel.

#### Scenario: Keyboard scroll
- **WHEN** the user presses scroll keys
- **THEN** the viewport moves by the corresponding amount within sub-frame latency

#### Scenario: Mouse-wheel scroll
- **WHEN** the user scrolls the mouse wheel over the stream
- **THEN** the viewport moves accordingly
