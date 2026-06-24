//! `#[cfg(test)]` blame-pump helper, kept here (under `tui`) so it can name
//! `App` — the crate-root `testutil` module only sees `crate::tui::app` as a
//! private path, so this can't live alongside the git-scratch scaffolding.

#![cfg(test)]

/// Drive an open blame peek's background fetch to completion (bounded so a stuck
/// worker fails the test instead of hanging it).
pub(crate) fn drive_blame(app: &mut crate::tui::app::App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while app.peek_blame_loading() && std::time::Instant::now() < deadline {
        app.drain_blame();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
