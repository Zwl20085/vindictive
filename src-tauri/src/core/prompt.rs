//! A title for an agent tip from its prompt, so the user only has to write
//! the prompt. Mirrored by `titleFromPrompt` in `src/lib/prompt.ts`; both
//! run the vectors in `src/lib/prompt-vectors.json`.
//!
//! Rule:
//! 1. U+FEFF is removed. Runner blocks (`<!-- agent:begin … -->` through
//!    `<!-- agent:end -->`, or to the end when unclosed) and HTML comments
//!    (multi-line too, or to the end when unclosed) are dropped whole.
//! 2. The first line with text, horizontal rules skipped, without Markdown
//!    markers: headings, quotes, `-`/`*`/`+` and ASCII-digit list markers,
//!    task boxes, `**` `__` `~~` and backticks; links and images keep their
//!    text. Whitespace (the one class `WS` below) is collapsed and trimmed.
//! 3. Longer than `MAX_DERIVED_TITLE_CHARS` characters, it is cut at the last
//!    space within the limit (hard at the limit when that space comes before
//!    `MIN_WORD_CUT_CHARS`). A cut never leaves half a character cluster: a
//!    trailing joiner, combining mark or emoji modifier goes with the base
//!    it belongs to. Trailing punctuation is dropped and `…` appended; when
//!    nothing would be left but `…`, the bounded head is kept instead.

use std::sync::LazyLock;

use regex::Regex;

/// Longest derived title, in characters, before the ellipsis.
pub const MAX_DERIVED_TITLE_CHARS: usize = 60;
/// A cut at a space earlier than this would lose too much; cut hard instead.
const MIN_WORD_CUT_CHARS: usize = 20;
const ELLIPSIS: char = '…';
const ZWJ: char = '\u{200D}';
/// The whitespace class shared with `prompt.ts` (spelled the same there).
const WS: &str = r"[\t\n\x0B\x0C\r \x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}]";
/// Trailing punctuation dropped before the ellipsis.
const TRAILING_PUNCT: &str = ",;:.-–—";

fn re(pattern: &str) -> Regex {
    Regex::new(&pattern.replace("WS", WS)).expect("valid prompt regex")
}

static AGENT_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?s)<!--WS*agent:begin.*?<!--WS*agent:end(?:WS)*-->|<!--WS*agent:begin.*")
});
static COMMENT: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)<!--.*?-->|<!--.*"));
/// Block markers at the start of a line, removed repeatedly.
static LEAD: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(?:#{1,6}WS+|>WS?|[-*+]WS+|[0-9]+[.)]WS+|\[[ xX]\]WS+)"));
static IMAGE: LazyLock<Regex> = LazyLock::new(|| re(r"!\[([^\]]*)\]\([^)]*\)"));
static LINK: LazyLock<Regex> = LazyLock::new(|| re(r"\[([^\]]+)\]\([^)]*\)"));
static EMPHASIS: LazyLock<Regex> = LazyLock::new(|| re(r"\*\*|__|~~|`"));
static RULE: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:[-*_=]|WS)+$"));
static SPACES: LazyLock<Regex> = LazyLock::new(|| re(r"WS+"));
static EXTEND: LazyLock<Regex> = LazyLock::new(|| {
    re(r"^[\p{M}\x{200D}\x{FE00}-\x{FE0F}\x{1F3FB}-\x{1F3FF}\x{E0020}-\x{E007F}]$")
});

fn is_ws(c: char) -> bool {
    SPACES.is_match(c.encode_utf8(&mut [0; 4]))
}

/// A character that continues the cluster before it.
fn is_extend(c: char) -> bool {
    EXTEND.is_match(c.encode_utf8(&mut [0; 4]))
}

/// `text` with whitespace (class `WS`) collapsed to single spaces and trimmed.
fn collapse(text: &str) -> String {
    SPACES.replace_all(text, " ").trim_matches(' ').to_string()
}

/// The title for a tip whose prompt is `prompt`; `None` when it has no text.
pub fn title_from_prompt(prompt: &str) -> Option<String> {
    let text = prompt.replace('\u{FEFF}', "");
    let text = AGENT_BLOCK.replace_all(&text, "");
    let text = COMMENT.replace_all(&text, "");
    text.lines()
        .map(collapse)
        .filter(|line| !line.is_empty() && !RULE.is_match(line))
        .map(|line| plain_line(&line))
        .find(|line| !line.is_empty())
        .map(|line| shorten(&line))
}

/// `line` without Markdown markers, whitespace collapsed.
fn plain_line(line: &str) -> String {
    let mut text = line.to_string();
    loop {
        let stripped = LEAD.replace(&text, "").into_owned();
        if stripped == text {
            break;
        }
        text = stripped;
    }
    let text = IMAGE.replace_all(&text, "$1");
    let text = LINK.replace_all(&text, "$1");
    collapse(&EMPHASIS.replace_all(&text, ""))
}

/// `kept` without a cluster the cut split: when the next character still
/// belongs to the last one, or `kept` ends in a joiner, the whole trailing
/// cluster (base, marks and joined parts) is dropped.
fn drop_split_cluster(mut kept: Vec<char>, next: Option<char>) -> Vec<char> {
    let split = next.is_some_and(is_extend) || kept.last() == Some(&ZWJ);
    if !split {
        return kept;
    }
    loop {
        while kept.last().copied().is_some_and(is_extend) {
            kept.pop();
        }
        kept.pop();
        if kept.last() != Some(&ZWJ) {
            return kept;
        }
    }
}

/// `text` cut to `MAX_DERIVED_TITLE_CHARS` at a word boundary, with `…`.
fn shorten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_DERIVED_TITLE_CHARS {
        return text.to_string();
    }
    let head = &chars[..MAX_DERIVED_TITLE_CHARS];
    let cut = head
        .iter()
        .rposition(|&c| is_ws(c))
        .filter(|&i| i >= MIN_WORD_CUT_CHARS)
        .unwrap_or(MAX_DERIVED_TITLE_CHARS);
    let kept: String = drop_split_cluster(head[..cut].to_vec(), chars.get(cut).copied())
        .into_iter()
        .collect();
    let kept = kept.trim_end_matches(|c: char| is_ws(c) || TRAILING_PUNCT.contains(c));
    if kept.is_empty() {
        let bounded: String = head.iter().collect();
        return format!("{}{ELLIPSIS}", bounded.trim_end_matches(is_ws));
    }
    format!("{kept}{ELLIPSIS}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vectors both implementations run: `src/lib/prompt-vectors.json`,
    /// read by `src/lib/prompt.test.ts` too, so the two cannot drift.
    const VECTORS: &str = include_str!("../../../src/lib/prompt-vectors.json");

    #[test]
    fn titles_from_prompts_match_the_shared_vectors() {
        let vectors: Vec<(String, Option<String>)> = serde_json::from_str(VECTORS).unwrap();
        assert!(vectors.len() >= 25, "{} vectors", vectors.len());
        for (prompt, want) in &vectors {
            assert_eq!(
                title_from_prompt(prompt).as_deref(),
                want.as_deref(),
                "prompt {prompt:?}"
            );
        }
    }

    #[test]
    fn derived_titles_are_short() {
        let long = "word ".repeat(100);
        let title = title_from_prompt(&long).unwrap();
        assert!(
            title.chars().count() <= MAX_DERIVED_TITLE_CHARS + 1,
            "{title}"
        );
        assert!(title.ends_with(ELLIPSIS));
    }
}
