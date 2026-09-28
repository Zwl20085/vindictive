//! Paper metadata from arXiv (Atom) and Crossref (JSON).

use crate::core::tip::Paper;

const ARXIV_API: &str = "https://export.arxiv.org/api/query";
const CROSSREF_API: &str = "https://api.crossref.org/works";
const UA: &str = concat!(
    "vindictive/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Zwl20085/vindictive)"
);
const TIMEOUT_SECS: u64 = 15;

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("no metadata found for {0}")]
    NotFound(String),
}

fn http() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .build()
}

pub async fn fetch_arxiv(id: &str) -> Result<Paper, MetaError> {
    let xml = http()?
        .get(ARXIV_API)
        .query(&[("id_list", id), ("max_results", "1")])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_arxiv_atom(&xml).ok_or_else(|| MetaError::NotFound(format!("arXiv:{id}")))
}

pub async fn fetch_crossref(doi: &str) -> Result<Paper, MetaError> {
    let url = format!("{CROSSREF_API}/{}", encode_doi(doi.trim()));
    let json = http()?
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    parse_crossref(&json).ok_or_else(|| MetaError::NotFound(format!("doi:{doi}")))
}

/// Percent-encode a DOI for use in a URL path (slashes kept).
pub fn encode_doi(doi: &str) -> String {
    let mut out = String::with_capacity(doi.len());
    for b in doi.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'/'
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Parse the first `<entry>` of an arXiv Atom feed.
pub fn parse_arxiv_atom(xml: &str) -> Option<Paper> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let entry = doc.descendants().find(|n| n.has_tag_name("entry"))?;
    let text = |tag: &str| -> Option<String> {
        entry
            .children()
            .find(|n| n.has_tag_name(tag))
            .and_then(|n| n.text())
            .map(collapse_ws)
            .filter(|s| !s.is_empty())
    };
    let title = text("title")?;
    if title.eq_ignore_ascii_case("Error") {
        return None;
    }
    let authors = entry
        .children()
        .filter(|n| n.has_tag_name("author"))
        .filter_map(|a| {
            a.children()
                .find(|n| n.has_tag_name("name"))
                .and_then(|n| n.text())
        })
        .map(collapse_ws)
        .collect();
    let year = text("published").and_then(|p| p.get(..4)?.parse().ok());
    let venue = entry
        .children()
        .find(|n| n.has_tag_name("journal_ref"))
        .and_then(|n| n.text())
        .map(collapse_ws);
    let url = text("id");
    Some(Paper {
        title,
        authors,
        year,
        venue,
        url,
    })
}

/// Parse a Crossref `works/{doi}` response.
pub fn parse_crossref(json: &serde_json::Value) -> Option<Paper> {
    let msg = json.get("message")?;
    let first_str = |key: &str| -> Option<String> {
        msg.get(key)?.as_array()?.first()?.as_str().map(collapse_ws)
    };
    let title = first_str("title")?;
    let authors = msg
        .get("author")
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(author_name).collect())
        .unwrap_or_default();
    let year = ["published-print", "published-online", "issued", "created"]
        .iter()
        .find_map(|k| msg.get(k)?.get("date-parts")?.get(0)?.get(0)?.as_i64())
        .map(|y| y as i32);
    let venue = first_str("container-title").or_else(|| first_str("short-container-title"));
    let url = msg.get("URL").and_then(|u| u.as_str()).map(str::to_string);
    Some(Paper {
        title,
        authors,
        year,
        venue,
        url,
    })
}

fn author_name(a: &serde_json::Value) -> Option<String> {
    let given = a.get("given").and_then(|g| g.as_str()).unwrap_or("");
    let family = a.get("family").and_then(|f| f.as_str());
    match (given, family) {
        ("", None) => a.get("name").and_then(|n| n.as_str()).map(str::to_string),
        ("", Some(f)) => Some(f.to_string()),
        (g, Some(f)) => Some(format!("{g} {f}")),
        (g, None) => Some(g.to_string()),
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATOM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
  <title>ArXiv Query</title>
  <entry>
    <id>http://arxiv.org/abs/2401.12345v2</id>
    <published>2024-01-23T18:00:00Z</published>
    <title>Thermal Modelling of
      Hairpin Windings</title>
    <author><name>Ada Lovelace</name></author>
    <author><name>Wentao Zhang</name></author>
    <arxiv:journal_ref>IEEE Trans. Ind. Electron.</arxiv:journal_ref>
  </entry>
</feed>"#;

    #[test]
    fn parses_arxiv() {
        let p = parse_arxiv_atom(ATOM).unwrap();
        assert_eq!(p.title, "Thermal Modelling of Hairpin Windings");
        assert_eq!(p.authors, vec!["Ada Lovelace", "Wentao Zhang"]);
        assert_eq!(p.year, Some(2024));
        assert_eq!(p.venue.as_deref(), Some("IEEE Trans. Ind. Electron."));
        assert_eq!(p.url.as_deref(), Some("http://arxiv.org/abs/2401.12345v2"));
    }

    #[test]
    fn arxiv_error_entry_is_none() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><title>Error</title></entry></feed>"#;
        assert!(parse_arxiv_atom(xml).is_none());
        assert!(parse_arxiv_atom("not xml").is_none());
        assert!(parse_arxiv_atom(r#"<feed xmlns="http://www.w3.org/2005/Atom"></feed>"#).is_none());
    }

    #[test]
    fn parses_crossref() {
        let json = serde_json::json!({
            "message": {
                "title": ["A  Great\nPaper"],
                "author": [
                    {"given": "Grace", "family": "Hopper"},
                    {"family": "Turing"},
                    {"name": "IEEE Working Group"}
                ],
                "issued": {"date-parts": [[2023, 5]]},
                "container-title": ["IEEE Transactions on Power Electronics"],
                "URL": "https://doi.org/10.1109/x"
            }
        });
        let p = parse_crossref(&json).unwrap();
        assert_eq!(p.title, "A Great Paper");
        assert_eq!(
            p.authors,
            vec!["Grace Hopper", "Turing", "IEEE Working Group"]
        );
        assert_eq!(p.year, Some(2023));
        assert_eq!(
            p.venue.as_deref(),
            Some("IEEE Transactions on Power Electronics")
        );
        assert_eq!(p.url.as_deref(), Some("https://doi.org/10.1109/x"));
    }

    #[test]
    fn encodes_doi_reserved_characters() {
        assert_eq!(
            encode_doi("10.1109/TIE.2024.1234567"),
            "10.1109/TIE.2024.1234567"
        );
        assert_eq!(encode_doi("10.1000/a?b#c d"), "10.1000/a%3Fb%23c%20d");
    }

    #[test]
    fn crossref_without_title_is_none() {
        assert!(parse_crossref(&serde_json::json!({"message": {}})).is_none());
        assert!(parse_crossref(&serde_json::json!({"status": "error"})).is_none());
    }
}
