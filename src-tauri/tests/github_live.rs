//! Live round trip against the GitHub Contents API. Ignored by default.
//!
//! ```text
//! VINDICTIVE_TEST_TOKEN=<token> VINDICTIVE_TEST_REPO=owner/repo \
//!   cargo test --test github_live -- --ignored
//! ```
//! Creates and updates `tips/.vindictive-smoke-test.md` in that repo.

use vindictive_lib::core::tip::Tip;
use vindictive_lib::sync::github::{GithubClient, RepoRef};

const SMOKE_PATH: &str = "tips/.vindictive-smoke-test.md";

fn client() -> Option<GithubClient> {
    let token = std::env::var("VINDICTIVE_TEST_TOKEN").ok()?;
    let repo = std::env::var("VINDICTIVE_TEST_REPO").ok()?;
    let (owner, name) = repo.split_once('/')?;
    let repo = RepoRef {
        owner: owner.into(),
        repo: name.into(),
        branch: "main".into(),
    };
    GithubClient::new(repo, &token).ok()
}

#[tokio::test]
#[ignore = "needs network and VINDICTIVE_TEST_TOKEN / VINDICTIVE_TEST_REPO"]
async fn lists_reads_and_writes() {
    let Some(client) = client() else {
        eprintln!("skipping: env not set");
        return;
    };
    let files = client.list_dir("tips").await.expect("list_dir");
    let md: Vec<_> = files.iter().filter(|f| f.name.ends_with(".md")).collect();
    assert!(!md.is_empty(), "tips dir has no markdown files");

    let (text, sha) = client.get_text(&md[0].path).await.expect("get_text");
    let tip = Tip::parse(&md[0].path, Some(sha), &text).expect("parse fetched tip");
    assert!(!tip.front.title.is_empty());

    let existing = client.get_text(SMOKE_PATH).await.ok().map(|(_, sha)| sha);
    let body = format!(
        "---\ntitle: Vindictive smoke test\nkind: note\n---\n\nWritten at {}.\n",
        chrono::Local::now()
    );
    let new_sha = client
        .put_text(
            SMOKE_PATH,
            &body,
            existing.as_deref(),
            "test: vindictive smoke test",
        )
        .await
        .expect("put_text");
    let (again, sha2) = client.get_text(SMOKE_PATH).await.expect("re-read");
    assert_eq!(sha2, new_sha);
    assert_eq!(again, body);

    let stale = client
        .put_text(
            SMOKE_PATH,
            &body,
            Some("0000000000000000000000000000000000000000"),
            "x",
        )
        .await;
    assert!(stale.is_err(), "stale sha must be rejected");
}
