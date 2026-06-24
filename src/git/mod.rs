//! Load git changes into the normalized `Changeset` via gix + imara-diff.
//!
//! Sources:
//! - working tree (`rediff diff`): HEAD vs the working tree, untracked included
//! - staged (`rediff diff --staged`): HEAD vs the index
//! - commit / range (`rediff show [ref]`, `rediff diff a..b`): tree vs tree

mod blame;
mod commits;
mod diff;
mod enumerate;
mod filter;
mod types;

pub use blame::*;
pub use commits::*;
pub use diff::*;
pub use enumerate::*;
pub use filter::*;
pub use types::*;
