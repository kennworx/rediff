## ADDED Requirements

### Requirement: Peek loads its own file during streaming
The single-file peek SHALL source the peeked file's content directly from git (by path and the view's base/new refs) rather than from the changeset's cached text, so that preview and diff work on any file the moment the file list appears — even before that file's bulk diff has run.

#### Scenario: Preview an undiffed file
- **WHEN** the file list is shown, a file has not yet been diffed, and the user opens the peek in content mode
- **THEN** the file's content is loaded and shown without waiting for the bulk diff

#### Scenario: Diff an undiffed file
- **WHEN** the user opens the peek in diff mode on a not-yet-diffed file
- **THEN** that one file's diff is computed on demand against the view's base and shown
