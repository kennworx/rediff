## ADDED Requirements

### Requirement: Render not-yet-diffed files
The review stream and sidebar SHALL render files that have not yet been diffed — a placeholder in the stream and placeholder stats in the sidebar — and SHALL replace them with the real diff and stats when each file's computation completes.

#### Scenario: Sidebar placeholder stats
- **WHEN** a file is listed but not yet diffed
- **THEN** the sidebar shows its path and status with placeholder `+/−` stats, which become real numbers once it is diffed

#### Scenario: Stream placeholder replaced
- **WHEN** the user is positioned on a file whose diff has not yet computed
- **THEN** the diff pane shows a placeholder/progress, and the file's diff appears in place once computed

#### Scenario: Navigation tolerates undiffed files
- **WHEN** files are still streaming in
- **THEN** moving between files and the file list does not error on undiffed files
