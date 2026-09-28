//! Split and join `---` delimited YAML frontmatter.

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrontmatterError {
    #[error("file does not start with a `---` frontmatter block")]
    MissingOpen,
    #[error("frontmatter block is never closed with `---`")]
    MissingClose,
}

/// Returns `(yaml, body)`. Accepts CRLF and LF line endings and a BOM.
pub fn split(text: &str) -> Result<(&str, &str), FrontmatterError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = text
        .strip_prefix("---\r\n")
        .or_else(|| text.strip_prefix("---\n"))
        .ok_or(FrontmatterError::MissingOpen)?;
    split_at_close(rest).ok_or(FrontmatterError::MissingClose)
}

fn split_at_close(rest: &str) -> Option<(&str, &str)> {
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            let yaml = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return Some((yaml, body));
        }
        offset += line.len();
    }
    None
}

pub fn join(yaml: &str, body: &str) -> String {
    let yaml = yaml.trim_end_matches('\n');
    let body = body.trim_start_matches('\n');
    if body.is_empty() {
        format!("---\n{yaml}\n---\n")
    } else {
        format!("---\n{yaml}\n---\n\n{body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lf_and_crlf() {
        assert_eq!(split("---\na: 1\n---\nbody"), Ok(("a: 1\n", "body")));
        assert_eq!(
            split("---\r\na: 1\r\n---\r\nbody"),
            Ok(("a: 1\r\n", "body"))
        );
    }

    #[test]
    fn empty_body_is_ok() {
        assert_eq!(split("---\na: 1\n---"), Ok(("a: 1\n", "")));
        assert_eq!(split("---\na: 1\n---\n"), Ok(("a: 1\n", "")));
    }

    #[test]
    fn bom_is_ignored() {
        assert_eq!(split("\u{feff}---\na: 1\n---\n"), Ok(("a: 1\n", "")));
    }

    #[test]
    fn errors() {
        assert_eq!(split("hello"), Err(FrontmatterError::MissingOpen));
        assert_eq!(split("---\na: 1\n"), Err(FrontmatterError::MissingClose));
    }

    #[test]
    fn join_is_inverse_of_split() {
        let text = join("a: 1", "body\n");
        assert_eq!(text, "---\na: 1\n---\n\nbody\n");
        assert_eq!(split(&text), Ok(("a: 1\n", "\nbody\n")));
        assert_eq!(join("a: 1\n", ""), "---\na: 1\n---\n");
    }
}
