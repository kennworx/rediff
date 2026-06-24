## ADDED Requirements

### Requirement: Two-stage loading
The system SHALL be able to enumerate a changeset's files (paths, status, rename source) without computing their diffs, and to compute a single file's diff (hunks and stats) separately. A file MAY exist in the changeset before it has been diffed.

#### Scenario: Enumerate without diffing
- **WHEN** a changeset is enumerated
- **THEN** every changed file's path and status are available with no blob contents read and no hunks computed

#### Scenario: Diff one file
- **WHEN** a single enumerated file is diffed
- **THEN** that file's hunks and `+/−` stats are produced from its two sides

### Requirement: Streaming load with progress and cancel
The loader SHALL run per-file diffs in the background, report progress (files completed of total), deliver each completed file to the caller, and stop promptly when cancellation is requested.

#### Scenario: Progress reported
- **WHEN** files are being diffed in the background
- **THEN** the caller can observe how many of the total files have completed

#### Scenario: Cancellation stops work
- **WHEN** cancellation is requested mid-load
- **THEN** the loader stops diffing further files and releases its workers
