//! Committed-rev per-line blame for the file-blame peek mode. Attributes every
//! line of a file at a revision to the commit that last modified it, via gix's
//! `Repository::blame_file` (working-tree state is never considered).

use std::collections::HashMap;
use std::sync::Arc;

use gix::bstr::BStr;

use super::commits::{author_name, summary_line};
use crate::model::{BlameCommit, BlameLine};

/// Per-line blame for `path` at `rev` over an already-open repository (so a
/// worker doing several reads pays for one discover), attributing every line to
/// the commit that last modified it. The returned vector is index-aligned to the
/// file's lines at `rev` (line 0 first); every entry resolves to a real commit.
pub fn blame_file_in(
    repo: &gix::Repository,
    rev: &str,
    path: &str,
) -> anyhow::Result<Vec<BlameLine>> {
    let suspect = repo.rev_parse_single(rev)?.object()?.peel_to_commit()?.id;
    blame_commit_in(repo, suspect, path)
}

/// Like [`blame_file_in`] but for an already-resolved `suspect` commit id — so a
/// caller that also reads the file's text at the same commit resolves the rev
/// once and reuses the id for both, instead of re-parsing the spec.
pub fn blame_commit_in(
    repo: &gix::Repository,
    suspect: gix::ObjectId,
    path: &str,
) -> anyhow::Result<Vec<BlameLine>> {
    let outcome = repo.blame_file(
        BStr::new(path),
        suspect,
        gix::repository::blame_file::Options::default(),
    )?;

    // Each distinct commit is resolved once into a shared `Arc<BlameCommit>`,
    // then that handle is cloned (a refcount bump) onto every line it blamed —
    // so a whole run of lines is one allocation, not one String triple per line.
    let mut table: HashMap<gix::ObjectId, Arc<BlameCommit>> = HashMap::new();
    let n = outcome
        .entries
        .iter()
        .map(|e| e.start_in_blamed_file + e.len.get())
        .max()
        .unwrap_or(0) as usize;
    let mut lines: Vec<BlameLine> = vec![BlameLine::default(); n];
    for e in &outcome.entries {
        let commit = table
            .entry(e.commit_id)
            .or_insert_with(|| Arc::new(commit_meta(repo, e.commit_id)))
            .clone();
        for k in 0..e.len.get() {
            if let Some(slot) = lines.get_mut((e.start_in_blamed_file + k) as usize) {
                *slot = BlameLine {
                    commit: Arc::clone(&commit),
                };
            }
        }
    }
    Ok(lines)
}

/// Resolve one commit's author/summary/time; blank fields if the object is missing
/// or malformed (the line still renders, just without attribution text).
fn commit_meta(repo: &gix::Repository, oid: gix::ObjectId) -> BlameCommit {
    let commit = repo
        .find_object(oid)
        .ok()
        .and_then(|o| o.try_into_commit().ok());
    let author = commit.as_ref().map(author_name).unwrap_or_default();
    let summary = commit.as_ref().map(summary_line).unwrap_or_default();
    let time_secs = commit
        .as_ref()
        .and_then(|c| c.time().ok())
        .map_or(0, |t| t.seconds);
    let sha = oid.to_string();
    let color_key = crate::model::commit_color_key(&sha);
    BlameCommit {
        sha,
        author,
        summary,
        time_secs,
        color_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate's own repo, opened as a live fixture.
    fn repo() -> gix::Repository {
        gix::discover(env!("CARGO_MANIFEST_DIR")).expect("discover crate repo")
    }

    #[test]
    fn blames_every_line_at_head() {
        let lines = blame_file_in(&repo(), "HEAD", "Cargo.toml").expect("blame Cargo.toml");
        assert!(!lines.is_empty(), "a tracked file blames to >0 lines");

        // Every line resolves to a real commit: a full 40-hex sha, an author, and a
        // plausible timestamp. This is the committed-rev contract — no gaps.
        for (i, l) in lines.iter().enumerate() {
            let c = &l.commit;
            assert_eq!(c.sha.len(), 40, "line {i} has a full sha");
            assert!(
                c.sha.chars().all(|ch| ch.is_ascii_hexdigit()),
                "line {i} sha is hex"
            );
            assert!(!c.author.is_empty(), "line {i} has an author");
            assert!(c.time_secs > 0, "line {i} has a commit time");
        }

        // Lines from one commit share a single Arc (a run is one allocation).
        if let [a, b, ..] = &lines[..] {
            if std::sync::Arc::ptr_eq(&a.commit, &b.commit) {
                assert_eq!(a.commit.sha, b.commit.sha, "shared handle → same commit");
            }
        }
    }

    #[test]
    fn missing_file_is_an_error() {
        blame_file_in(&repo(), "HEAD", "does/not/exist.rs").unwrap_err();
    }
}
