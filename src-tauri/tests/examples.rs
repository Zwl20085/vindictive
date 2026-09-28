//! Every example tip shipped in `examples/tips` must parse and round-trip.

use std::path::PathBuf;

use vindictive_lib::core::tip::Tip;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/tips")
}

#[test]
fn all_example_tips_parse_and_roundtrip() {
    let entries = std::fs::read_dir(examples_dir()).expect("examples/tips exists");
    let mut count = 0;
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let rel = format!("tips/{}", path.file_name().unwrap().to_string_lossy());
        let tip = Tip::parse(&rel, None, &text)
            .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
        assert!(!tip.front.title.is_empty());
        let again = Tip::parse(&rel, None, &tip.to_markdown().unwrap()).unwrap();
        assert_eq!(
            again.front,
            tip.front,
            "{} did not round-trip",
            path.display()
        );
        count += 1;
    }
    assert!(
        count >= 8,
        "expected at least 8 example tips, found {count}"
    );
}
