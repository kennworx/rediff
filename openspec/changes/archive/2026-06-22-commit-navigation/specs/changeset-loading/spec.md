## ADDED Requirements

### Requirement: Enumerate commits
The system SHALL enumerate commits reachable from a tip (defaulting to HEAD) up to a fixed cap, exposing for each commit its short SHA, summary, author, and time, for use by the commit picker.

#### Scenario: Recent commits available
- **WHEN** the commit picker requests the commit list
- **THEN** commits reachable from HEAD are returned, newest first, up to the cap

#### Scenario: List is capped
- **WHEN** the repository has more commits than the cap
- **THEN** at most the cap number of commits are returned and the truncation is indicated

### Requirement: File-scoped commit history
The system SHALL determine, for a given path, which of the enumerated commits changed that path, by comparing the path's blob between each commit's tree and its parent's tree.

#### Scenario: Commits that touched a path
- **WHEN** the file-scoped history for a path is requested
- **THEN** only commits whose tree differs from their parent at that path are returned

#### Scenario: Path absent from a commit
- **WHEN** a commit neither contains nor removes the path relative to its parent
- **THEN** that commit is omitted from the file-scoped history

### Requirement: Review a commit or range
The system SHALL load a review changeset for `rsdiff review [sha] [--from <base>]`. With no `sha`, the target SHALL be HEAD. Without `--from`, the changeset SHALL be the single commit's diff (target vs its parent). With `--from <base>`, the changeset SHALL be the combined net diff between the merge-base of `base` and the target and the target's tree.

#### Scenario: Review the latest commit
- **WHEN** `rsdiff review` runs with no arguments
- **THEN** the changeset contains the diff that HEAD introduced over its parent

#### Scenario: Review a specific commit
- **WHEN** `rsdiff review <sha>` runs
- **THEN** the changeset contains the diff that commit introduced over its parent

#### Scenario: Review a branch range as a net diff
- **WHEN** `rsdiff review <sha> --from <base>` runs
- **THEN** the changeset is the combined net diff between the merge-base of `base` and the target and the target, as one flat file list

#### Scenario: Base moved ahead after branching
- **WHEN** `--from <base>` is given and `base` has commits not present in the target
- **THEN** the net diff is computed against the merge-base so only the target's own changes appear
