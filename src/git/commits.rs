//! Commit enumeration and range-id computation for the commit picker.

use std::collections::HashSet;
use std::path::Path;

use gix::bstr::ByteSlice;

use super::enumerate::tree_oid_at;
use crate::model::{CommitInfo, CommitMessage};

/// Default cap on how many commits the picker enumerates.
pub const COMMIT_CAP: usize = 200;

/// Enumerate commits reachable from `tip` (newest first), up to `cap`. When
/// `path` is set, only commits that changed that path are returned. Ids in
/// `exclude` are skipped. Returns the list and whether enumeration hit the cap.
pub fn enumerate_commits<S: ::std::hash::BuildHasher>(
    repo_dir: &Path,
    tip: &str,
    cap: usize,
    path: Option<&str>,
    exclude: &HashSet<String, S>,
) -> anyhow::Result<(Vec<CommitInfo>, bool)> {
    use gix::revision::walk::Sorting;
    use gix::traverse::commit::simple::CommitTimeOrder;
    let repo = gix::discover(repo_dir)?;
    let tip_id = repo.rev_parse_single(tip)?.detach();
    let walk = repo
        .rev_walk(Some(tip_id))
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::default()))
        .all()?;

    let mut out = Vec::new();
    let mut truncated = false;
    for info in walk {
        let info = info?;
        let commit = info.object()?;
        let id_hex = commit.id().to_string();
        if exclude.contains(&id_hex) {
            continue;
        }
        if let Some(p) = path {
            if !touches(&commit, p) {
                continue;
            }
        }
        out.push(commit_info(&commit));
        if out.len() >= cap {
            truncated = true;
            break;
        }
    }
    Ok((out, truncated))
}

/// The set of commit ids in `base..target` (reachable from `target` but not
/// `base`), used to exclude a reviewed range's own commits from the picker.
pub fn range_commit_ids(
    repo_dir: &Path,
    base: &str,
    target: &str,
) -> anyhow::Result<HashSet<String>> {
    use gix::revision::walk::Sorting;
    use gix::traverse::commit::simple::CommitTimeOrder;
    let repo = gix::discover(repo_dir)?;
    let base_id = repo.rev_parse_single(base)?.detach();
    let target_id = repo.rev_parse_single(target)?.detach();

    // Ancestors of base (bounded), to subtract from target's history.
    let mut base_set: HashSet<String> = HashSet::new();
    if let Ok(walk) = repo
        .rev_walk(Some(base_id))
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::default()))
        .all()
    {
        for info in walk.take(5000).flatten() {
            base_set.insert(info.id.to_string());
        }
    }

    let mut range: HashSet<String> = HashSet::new();
    let walk = repo
        .rev_walk(Some(target_id))
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::default()))
        .all()?;
    for info in walk.take(5000) {
        let info = info?;
        let hex = info.id.to_string();
        if base_set.contains(&hex) {
            continue;
        }
        range.insert(hex);
    }
    Ok(range)
}

/// Fetch one commit's full message and identity by `sha` (any rev-parseable
/// spec). Used by the commit-message popup and the commit-view banner, both of
/// which need the body that enumeration discards.
pub fn commit_message(repo_dir: &Path, sha: &str) -> anyhow::Result<CommitMessage> {
    commit_message_in(&gix::discover(repo_dir)?, sha)
}

/// Like [`commit_message`], over an already-open repository — so a caller that
/// also enumerates the commit's files for the same switch shares one discover.
pub fn commit_message_in(repo: &gix::Repository, sha: &str) -> anyhow::Result<CommitMessage> {
    let commit = repo.rev_parse_single(sha)?.object()?.peel_to_commit()?;
    // Identity extraction is commit_info's job — one place for the shorten /
    // lossy-author / date chains; this only adds the body enumeration drops.
    let info = commit_info(&commit);
    let body = commit
        .message_raw_sloppy()
        .to_str_lossy()
        .trim_end()
        .to_string();
    Ok(CommitMessage {
        sha: info.id,
        short: info.short,
        author: info.author,
        date: info.date,
        body,
    })
}

fn commit_info(commit: &gix::Commit) -> CommitInfo {
    let id = commit.id();
    let short = id
        .shorten()
        .map_or_else(|_| id.to_string(), |p| p.to_string());
    let date = commit
        .time()
        .ok()
        .map(|t| ymd(t.seconds))
        .unwrap_or_default();
    CommitInfo {
        id: id.to_string(),
        short,
        summary: summary_line(commit),
        author: author_name(commit),
        date,
    }
}

/// One commit's author name, blank if unreadable. The single place the
/// lossy-name extraction lives (used by the picker, the popup, and blame).
pub(crate) fn author_name(commit: &gix::Commit) -> String {
    commit
        .author()
        .ok()
        .map(|a| a.name.to_str_lossy().into_owned())
        .unwrap_or_default()
}

/// One commit's summary (first message line), blank if unreadable.
pub(crate) fn summary_line(commit: &gix::Commit) -> String {
    commit
        .message()
        .ok()
        .map(|m| m.summary().to_string())
        .unwrap_or_default()
}

/// Whether `commit` changed `path` relative to its first parent (compares the
/// blob object id at that path; presence flips count as a change).
fn touches(commit: &gix::Commit, path: &str) -> bool {
    let cur = commit.tree().ok().and_then(|t| tree_oid_at(&t, path));
    let par = commit
        .parent_ids()
        .next()
        .and_then(|pid| pid.object().ok())
        .and_then(|o| o.into_commit().tree().ok())
        .and_then(|t| tree_oid_at(&t, path));
    cur != par
}

/// Format unix seconds as `YYYY-MM-DD` (proleptic Gregorian, UTC).
fn ymd(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::multi_commit_repo;

    #[test]
    fn enumerate_lists_history_newest_first() {
        let repo = multi_commit_repo();
        let dir = repo.path();
        let (commits, _truncated) =
            enumerate_commits(dir, "HEAD", COMMIT_CAP, None, &HashSet::new()).unwrap();
        assert!(commits.len() >= 2, "repo should have history");
        for c in &commits {
            assert!(!c.id.is_empty() && !c.short.is_empty());
        }
        let head = gix::discover(dir)
            .unwrap()
            .rev_parse_single("HEAD")
            .unwrap()
            .to_string();
        assert_eq!(commits[0].id, head, "newest commit first");
    }

    #[test]
    fn enumerate_respects_cap() {
        let repo = multi_commit_repo();
        let (commits, truncated) =
            enumerate_commits(repo.path(), "HEAD", 1, None, &HashSet::new()).unwrap();
        assert_eq!(commits.len(), 1);
        assert!(truncated, "cap of 1 against >1 commits is truncated");
    }

    #[test]
    fn enumerate_excludes_ids() {
        let repo = multi_commit_repo();
        let head = gix::discover(repo.path())
            .unwrap()
            .rev_parse_single("HEAD")
            .unwrap()
            .to_string();
        let mut exclude = HashSet::new();
        exclude.insert(head.clone());
        let (commits, _) =
            enumerate_commits(repo.path(), "HEAD", COMMIT_CAP, None, &exclude).unwrap();
        assert!(commits.iter().all(|c| c.id != head), "HEAD excluded");
    }

    #[test]
    fn commit_message_loads_full_body_by_sha() {
        let repo = multi_commit_repo();
        let head = gix::discover(repo.path())
            .unwrap()
            .rev_parse_single("HEAD")
            .unwrap()
            .to_string();
        let msg = commit_message(repo.path(), "HEAD").unwrap();
        assert_eq!(msg.sha, head, "full sha matches HEAD");
        assert!(head.starts_with(&msg.short), "short is a prefix of the sha");
        assert_eq!(msg.author, "t", "author from the fixture");
        assert_eq!(msg.body, "second", "the fixture's HEAD message body");
        assert_eq!(msg.date.len(), 10, "date is YYYY-MM-DD");
    }

    #[test]
    fn commit_message_errors_on_unknown_rev() {
        let repo = multi_commit_repo();
        commit_message(repo.path(), "deadbeefdeadbeef").unwrap_err();
    }

    #[test]
    fn range_ids_contains_target_not_base() {
        let fixture = multi_commit_repo();
        let repo = gix::discover(fixture.path()).unwrap();
        let head = repo.rev_parse_single("HEAD").unwrap().to_string();
        let parent = repo.rev_parse_single("HEAD~1").unwrap().to_string();
        let ids = range_commit_ids(fixture.path(), "HEAD~1", "HEAD").unwrap();
        assert!(ids.contains(&head), "HEAD is in HEAD~1..HEAD");
        assert!(
            !ids.contains(&parent),
            "the base itself is not in the range"
        );
    }
}
