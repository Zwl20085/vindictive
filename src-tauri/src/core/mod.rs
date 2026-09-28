//! Pure domain logic: no Tauri, no network, no clock. Everything here takes
//! `now` as a parameter so it can be tested deterministically.

pub mod capture;
pub mod deadline;
pub mod frontmatter;
pub mod nextup;
pub mod recur;
pub mod template;
pub mod timeparse;
pub mod tip;
