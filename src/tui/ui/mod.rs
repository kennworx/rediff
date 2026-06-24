//! Rendering: navigation sidebar + windowed review stream (stack or split) +
//! status bar + fuzzy-jump overlay. Colors come from the active `Theme`.

mod frame;
mod overlays;
mod stream;

pub use frame::*;
