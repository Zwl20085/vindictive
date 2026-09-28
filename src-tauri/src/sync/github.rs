//! Minimal GitHub Contents API client. Only what the board needs:
//! list a directory, read a file, write a file.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde::Deserialize;

const API_ROOT: &str = "https://api.github.com";
const UA: &str = concat!("vindictive/", env!("CARGO_PKG_VERSION"));
const TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: String,
    pub repo: String,
    pub branch: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RemoteFile {
    pub name: String,
    pub path: String,
    pub sha: String,
    #[serde(default)]
    pub size: u64,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Deserialize)]
struct ContentResponse {
    sha: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    encoding: String,
}

#[derive(Debug, Deserialize)]
struct PutResponse {
    content: PutContent,
}

#[derive(Debug, Deserialize)]
struct PutContent {
    sha: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("GitHub rejected the token (401). Check it in Settings.")]
    Unauthorized,
    #[error("not found: {0}. Check owner/repo/branch/dir.")]
    NotFound(String),
    #[error("GitHub rate limit reached. Try again later.")]
    RateLimited,
    #[error("{path} changed on GitHub since it was loaded; synced, please retry.")]
    Conflict { path: String },
    #[error("GitHub returned {status}: {body}")]
    Status { status: u16, body: String },
    #[error("could not decode file content: {0}")]
    Decode(String),
    #[error("invalid configuration: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, GithubError>;

#[derive(Clone)]
pub struct GithubClient {
    http: reqwest::Client,
    repo: RepoRef,
}

impl GithubClient {
    pub fn new(repo: RepoRef, token: &str) -> Result<Self> {
        validate_repo(&repo)?;
        let mut headers = HeaderMap::new();
        let auth = HeaderValue::from_str(&format!("Bearer {}", token.trim()))
            .map_err(|_| GithubError::Config("token contains invalid characters".into()))?;
        headers.insert(AUTHORIZATION, auth);
        headers.insert(USER_AGENT, HeaderValue::from_static(UA));
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()?;
        Ok(Self { http, repo })
    }

    pub fn repo(&self) -> &RepoRef {
        &self.repo
    }

    /// List the files directly inside `dir`.
    pub async fn list_dir(&self, dir: &str) -> Result<Vec<RemoteFile>> {
        let url = contents_url(&self.repo, dir);
        let res = self
            .http
            .get(&url)
            .header(ACCEPT, "application/vnd.github+json")
            .query(&[("ref", self.repo.branch.as_str())])
            .send()
            .await?;
        let res = check(res, dir).await?;
        Ok(res.json::<Vec<RemoteFile>>().await?)
    }

    /// Fetch a text file and its blob SHA.
    pub async fn get_text(&self, path: &str) -> Result<(String, String)> {
        let url = contents_url(&self.repo, path);
        let res = self
            .http
            .get(&url)
            .header(ACCEPT, "application/vnd.github+json")
            .query(&[("ref", self.repo.branch.as_str())])
            .send()
            .await?;
        let res = check(res, path).await?;
        let body: ContentResponse = res.json().await?;
        let text = decode_content(&body.content, &body.encoding)?;
        Ok((text, body.sha))
    }

    /// Fetch raw bytes (for images).
    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let url = contents_url(&self.repo, path);
        let res = self
            .http
            .get(&url)
            .header(ACCEPT, "application/vnd.github.raw+json")
            .query(&[("ref", self.repo.branch.as_str())])
            .send()
            .await?;
        let res = check(res, path).await?;
        Ok(res.bytes().await?.to_vec())
    }

    /// Create or update a file. Returns the new blob SHA.
    pub async fn put_text(
        &self,
        path: &str,
        text: &str,
        sha: Option<&str>,
        message: &str,
    ) -> Result<String> {
        let url = contents_url(&self.repo, path);
        let mut body = serde_json::json!({
            "message": message,
            "content": B64.encode(text.as_bytes()),
            "branch": self.repo.branch,
        });
        if let Some(sha) = sha {
            body["sha"] = serde_json::Value::String(sha.to_string());
        }
        let res = self
            .http
            .put(&url)
            .header(ACCEPT, "application/vnd.github+json")
            .json(&body)
            .send()
            .await?;
        if res.status() == StatusCode::CONFLICT {
            return Err(GithubError::Conflict {
                path: path.to_string(),
            });
        }
        if res.status() == StatusCode::UNPROCESSABLE_ENTITY {
            let body = res.text().await.unwrap_or_default();
            return Err(if is_sha_mismatch(&body) {
                GithubError::Conflict {
                    path: path.to_string(),
                }
            } else {
                GithubError::Status {
                    status: 422,
                    body: truncate(&body, 200),
                }
            });
        }
        let res = check(res, path).await?;
        let put: PutResponse = res.json().await?;
        Ok(put.content.sha)
    }
}

impl GithubClient {
    /// Delete a file. `sha` must be the blob SHA the board holds; GitHub
    /// refuses the delete when the file changed since, which surfaces as
    /// `Conflict` so the caller can resync.
    pub async fn delete_file(&self, path: &str, sha: &str, message: &str) -> Result<()> {
        let url = contents_url(&self.repo, path);
        let body = serde_json::json!({
            "message": message,
            "sha": sha,
            "branch": self.repo.branch,
        });
        let res = self
            .http
            .delete(&url)
            .header(ACCEPT, "application/vnd.github+json")
            .json(&body)
            .send()
            .await?;
        if res.status() == StatusCode::CONFLICT {
            return Err(GithubError::Conflict {
                path: path.to_string(),
            });
        }
        if res.status() == StatusCode::UNPROCESSABLE_ENTITY {
            let body = res.text().await.unwrap_or_default();
            return Err(if is_sha_mismatch(&body) {
                GithubError::Conflict {
                    path: path.to_string(),
                }
            } else {
                GithubError::Status {
                    status: 422,
                    body: truncate(&body, 200),
                }
            });
        }
        check(res, path).await.map(|_| ())
    }
}

async fn check(res: reqwest::Response, what: &str) -> Result<reqwest::Response> {
    let status = res.status();
    if status.is_success() {
        return Ok(res);
    }
    let remaining = res
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = res.text().await.unwrap_or_default();
    Err(match status {
        StatusCode::UNAUTHORIZED => GithubError::Unauthorized,
        StatusCode::NOT_FOUND => GithubError::NotFound(what.to_string()),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            if remaining.as_deref() == Some("0") || body.contains("rate limit") =>
        {
            GithubError::RateLimited
        }
        _ => GithubError::Status {
            status: status.as_u16(),
            body: truncate(&body, 200),
        },
    })
}

/// GitHub answers 422 both for a stale `sha` and for unrelated validation
/// errors; only the former should be reported as a conflict.
pub fn is_sha_mismatch(body: &str) -> bool {
    let b = body.to_ascii_lowercase();
    b.contains("does not match")
        || (b.contains("sha") && (b.contains("wasn't supplied") || b.contains("was not supplied")))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// Build the Contents API URL. Each path segment is percent-encoded.
pub fn contents_url(repo: &RepoRef, path: &str) -> String {
    let encoded: Vec<String> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(encode_segment)
        .collect();
    format!(
        "{API_ROOT}/repos/{}/{}/contents/{}",
        repo.owner,
        repo.repo,
        encoded.join("/")
    )
}

fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// GitHub base64 content contains newlines every 60 chars.
pub fn decode_content(content: &str, encoding: &str) -> Result<String> {
    if encoding != "base64" {
        return Err(GithubError::Decode(format!(
            "unexpected encoding {encoding:?}"
        )));
    }
    let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = B64
        .decode(cleaned)
        .map_err(|e| GithubError::Decode(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| GithubError::Decode(e.to_string()))
}

/// The web URL to view or edit a file.
pub fn blob_url(repo: &RepoRef, path: &str) -> String {
    format!(
        "https://github.com/{}/{}/blob/{}/{}",
        repo.owner, repo.repo, repo.branch, path
    )
}

pub fn validate_repo(repo: &RepoRef) -> Result<()> {
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if !ok(&repo.owner) {
        return Err(GithubError::Config(
            "owner must be a GitHub user or org name".into(),
        ));
    }
    if !ok(&repo.repo) {
        return Err(GithubError::Config("repo must be a repository name".into()));
    }
    if repo.branch.is_empty() || repo.branch.contains("..") || repo.branch.contains(' ') {
        return Err(GithubError::Config("branch is not a valid git ref".into()));
    }
    Ok(())
}

/// Validate a directory inside the repo. Rejects traversal and absolute paths.
pub fn validate_dir(dir: &str) -> Result<String> {
    let d = dir.trim().trim_matches('/');
    if d.is_empty() {
        return Ok(String::new());
    }
    if d.split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
        || d.contains('\\')
    {
        return Err(GithubError::Config(
            "dir must be a relative path like `tips`".into(),
        ));
    }
    Ok(d.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> RepoRef {
        RepoRef {
            owner: "Zwl20085".into(),
            repo: "vindictive-tips".into(),
            branch: "main".into(),
        }
    }

    #[test]
    fn builds_urls() {
        assert_eq!(
            contents_url(&repo(), "tips"),
            "https://api.github.com/repos/Zwl20085/vindictive-tips/contents/tips"
        );
        assert_eq!(
            contents_url(&repo(), "/tips/2026 plan.md"),
            "https://api.github.com/repos/Zwl20085/vindictive-tips/contents/tips/2026%20plan.md"
        );
        assert_eq!(
            contents_url(&repo(), ""),
            "https://api.github.com/repos/Zwl20085/vindictive-tips/contents/"
        );
        assert_eq!(
            blob_url(&repo(), "tips/a.md"),
            "https://github.com/Zwl20085/vindictive-tips/blob/main/tips/a.md"
        );
    }

    #[test]
    fn decodes_wrapped_base64() {
        let encoded = "LS0tCnRpdGxlOiBoaQotLS0K\nYm9keQ==\n";
        assert_eq!(
            decode_content(encoded, "base64").unwrap(),
            "---\ntitle: hi\n---\nbody"
        );
        assert!(decode_content("xx", "utf-8").is_err());
        assert!(decode_content("!!!", "base64").is_err());
    }

    #[test]
    fn validates_config() {
        assert!(validate_repo(&repo()).is_ok());
        let bad = RepoRef {
            owner: "a/b".into(),
            ..repo()
        };
        assert!(validate_repo(&bad).is_err());
        let bad = RepoRef {
            branch: "".into(),
            ..repo()
        };
        assert!(validate_repo(&bad).is_err());
        assert_eq!(validate_dir(" tips/ ").unwrap(), "tips");
        assert_eq!(validate_dir("").unwrap(), "");
        assert!(validate_dir("../etc").is_err());
        assert!(validate_dir("a//b").is_err());
    }

    #[test]
    fn client_rejects_bad_token() {
        assert!(GithubClient::new(repo(), "bad\ntoken").is_err());
        assert!(GithubClient::new(repo(), "ghp_ok").is_ok());
    }

    #[test]
    fn recognises_sha_mismatch_bodies() {
        assert!(is_sha_mismatch(
            r#"{"message":"tips/a.md does not match abc123"}"#
        ));
        assert!(is_sha_mismatch(
            r#"{"message":"Invalid request.\n\n\"sha\" wasn't supplied."}"#
        ));
        assert!(!is_sha_mismatch(
            r#"{"message":"Invalid request.\n\n\"content\" wasn't supplied."}"#
        ));
    }

    #[test]
    fn truncates_bodies() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("0123456789abc", 10).chars().count(), 11);
    }
}
