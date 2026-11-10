//! Question tips: Markdown files in the `questions/` subfolder of the tips
//! folder, where a Claude session leaves questions for the owner to answer.
//!
//! Being a question is derived from where the file lives, never stored in
//! it. A question tip is an ordinary tip in every other way, except that it
//! never counts as an agent task: the runner only scans the top level.

/// The subfolder, relative to the tips folder, read one level deep.
pub const QUESTIONS_DIR: &str = "questions";

/// `questions/<name>` with a plain file name (no further folder).
pub fn is_question_path(path: &str) -> bool {
    path.strip_prefix(QUESTIONS_DIR)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|name| !name.is_empty() && !name.contains(['/', '\\']))
}

/// The id of the tip at `path`: the file name without `.md`, and for a
/// question tip `questions/<stem>`. A `/` never occurs in a main tip's id,
/// so a question can never share an id with a main tip of the same name.
pub fn id_for_path(path: &str) -> String {
    let named = if is_question_path(path) {
        path
    } else {
        path.rsplit('/').next().unwrap_or(path)
    };
    named.strip_suffix(".md").unwrap_or(named).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_files_directly_in_questions_are_questions() {
        assert!(is_question_path("questions/pick-a-mesh.md"));
        for no in [
            "pick-a-mesh.md",
            "questions/",
            "questions",
            "questions/deeper/x.md",
            "questions\\x.md",
            "questionsx.md",
            "agent/questions/x.md",
            "Questions-later.md",
        ] {
            assert!(!is_question_path(no), "{no}");
        }
    }

    #[test]
    fn question_ids_carry_the_folder_and_main_ids_do_not() {
        assert_eq!(id_for_path("a.md"), "a");
        assert_eq!(id_for_path("questions/a.md"), "questions/a");
        assert_ne!(id_for_path("a.md"), id_for_path("questions/a.md"));
        // Other paths keep the old rule: the last file name.
        assert_eq!(id_for_path("tips/2026-camera.md"), "2026-camera");
        assert_eq!(id_for_path("questions/deeper/x.md"), "x");
    }
}
