//! Every example tip shipped in `examples/tips` must parse, round-trip, load
//! without warnings and point only at figures that exist; together they must
//! show every kind of tip.

use std::path::{Path, PathBuf};

use vindictive_lib::core::tip::{Kind, Tip};

const ALL_KINDS: [Kind; 5] = [
    Kind::Task,
    Kind::Deadline,
    Kind::Note,
    Kind::Reading,
    Kind::Event,
];

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/tips")
}

/// Top-level `.md` files in `examples/tips`, sorted (subfolders such as
/// `figures/` hold assets and their own README, not tips).
fn example_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(examples_dir())
        .expect("examples/tips exists")
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort();
    files
}

fn parse(path: &Path) -> Tip {
    let text = std::fs::read_to_string(path).unwrap();
    let rel = format!("tips/{}", path.file_name().unwrap().to_string_lossy());
    Tip::parse(&rel, None, &text)
        .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()))
}

/// Targets of Markdown images, `![alt](target)`, in `body`.
fn body_images(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("![") {
        rest = &rest[start + 2..];
        let Some(close) = rest.find("](") else { break };
        let after = &rest[close + 2..];
        let Some(end) = after.find(')') else { break };
        // Drop an optional title: ![alt](path "title").
        let target = after[..end].split_whitespace().next().unwrap_or("");
        found.push(target.to_string());
        rest = &after[end..];
    }
    found
}

fn is_remote(target: &str) -> bool {
    ["http://", "https://", "data:"]
        .iter()
        .any(|scheme| target.starts_with(scheme))
}

#[test]
fn all_example_tips_parse_and_roundtrip() {
    let files = example_files();
    assert!(!files.is_empty(), "no example tips found");
    let mut parsed = 0;
    for path in &files {
        let tip = parse(path);
        assert!(
            !tip.front.title.is_empty(),
            "{} has no title",
            path.display()
        );
        assert!(
            tip.warnings.is_empty(),
            "{} loads with warnings: {:?}",
            path.display(),
            tip.warnings
        );
        let rel = format!("tips/{}", path.file_name().unwrap().to_string_lossy());
        let again = Tip::parse(&rel, None, &tip.to_markdown().unwrap()).unwrap();
        assert_eq!(
            again.front,
            tip.front,
            "{} did not round-trip",
            path.display()
        );
        parsed += 1;
    }
    assert_eq!(
        parsed,
        files.len(),
        "every .md file in examples/tips is a tip"
    );
}

#[test]
fn examples_cover_every_kind() {
    let seen: Vec<Kind> = example_files()
        .iter()
        .map(|p| parse(p).front.kind)
        .collect();
    for kind in ALL_KINDS {
        assert!(seen.contains(&kind), "no example tip has kind {kind:?}");
    }
}

#[test]
fn referenced_local_figures_exist() {
    let dir = examples_dir();
    let mut checked = 0;
    for path in example_files() {
        let tip = parse(&path);
        let targets = tip
            .front
            .images
            .iter()
            .cloned()
            .chain(body_images(&tip.body));
        for target in targets.filter(|t| !is_remote(t)) {
            let file = dir.join(&target);
            assert!(
                file.is_file(),
                "{} references {target}, which is not at {}",
                path.display(),
                file.display()
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "expected at least one local figure in the examples"
    );
}
