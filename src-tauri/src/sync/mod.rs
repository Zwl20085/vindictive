//! Data sources: the local tips folder, plus arXiv, Crossref and Open-Meteo.
//! Parsing functions are pure and tested; the async fetchers are thin.

pub mod folder;
pub mod meta;
pub mod weather;
