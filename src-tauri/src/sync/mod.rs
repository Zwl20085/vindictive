//! Network adapters: GitHub Contents API, arXiv, Crossref, Open-Meteo.
//! Parsing functions are pure and tested; the async fetchers are thin.

pub mod github;
pub mod meta;
pub mod weather;
