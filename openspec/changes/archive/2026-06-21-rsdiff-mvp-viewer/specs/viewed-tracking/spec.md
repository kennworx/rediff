## ADDED Requirements

### Requirement: Mark files reviewed
The system SHALL let the user mark a file as reviewed and unmark it, and SHALL show each file's
reviewed state in the sidebar.

#### Scenario: Toggle reviewed
- **WHEN** the user marks the current file as reviewed
- **THEN** the sidebar shows that file as reviewed, and unmarking restores the unreviewed state

### Requirement: Jump to next unviewed
The system SHALL provide a command to jump to the next file that has not been marked reviewed.

#### Scenario: Skip reviewed files
- **WHEN** the user invokes next-unviewed while some files are marked reviewed
- **THEN** the stream jumps to the next file that is not marked reviewed

#### Scenario: All reviewed
- **WHEN** the user invokes next-unviewed and every file is reviewed
- **THEN** the system indicates there are no unviewed files remaining

### Requirement: Collapse reviewed files
The system SHALL be able to collapse reviewed files in the sidebar so the remaining work is
easier to scan.

#### Scenario: Reviewed files collapse
- **WHEN** a file is marked reviewed and collapsing is enabled
- **THEN** that file's entry is collapsed in the sidebar while unreviewed files stay expanded
