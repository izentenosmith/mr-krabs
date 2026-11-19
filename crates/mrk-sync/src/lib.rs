//! LAN sync engine (see `docs/05-sync.md`).
//!
//! M0 ships the Hybrid Logical Clock; change log, protocol and transport arrive in M7/M8.

pub mod hlc;

pub use hlc::{Hlc, HlcClock, HlcError};
