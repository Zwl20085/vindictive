//! The file written for a brand-new tip: the captured frontmatter, a commented
//! reference of every optional key that was not set, and an example body.
//! The comments survive until the app rewrites the file (Done, snooze, a
//! pushed local edit); by then the user has usually edited past them.

use super::tip::{FrontMatter, Tip, TipError};

/// Commented examples for keys the capture line did not set.
fn key_hints(front: &FrontMatter, zh: bool) -> Vec<String> {
    let mut lines = vec![if zh {
        "# ---- 可选键（去掉 # 即可启用）----".to_string()
    } else {
        "# ---- optional keys (remove the # to use one) ----".to_string()
    }];
    let mut hint = |missing: bool, en: &str, zh_text: &str| {
        if missing {
            lines.push(format!("# {}", if zh { zh_text } else { en }));
        }
    };
    hint(
        front.due.is_none(),
        "due: 2026-10-15 17:00            # a bare date means 23:59",
        "due: 2026-10-15 17:00            # 只写日期则为 23:59",
    );
    hint(
        front.remind.is_empty(),
        "remind: [-1d, -2h]               # relative to due, or \"2026-10-13 09:00\"",
        "remind: [-1d, -2h]               # 相对截止时间，或绝对时间 \"2026-10-13 09:00\"",
    );
    hint(
        front.location.is_none(),
        "location: Lab 302",
        "location: 实验室 302",
    );
    hint(
        front.tags.is_empty(),
        "tags: [paper, lab]",
        "tags: [论文, 实验]",
    );
    hint(
        front.links.is_empty(),
        "links: [https://example.org]",
        "links: [https://example.org]",
    );
    hint(
        front.images.is_empty(),
        "images: [figures/plot.svg]       # png, jpg, svg… relative to the tips directory",
        "images: [figures/plot.svg]       # png、jpg、svg…，相对 tips 目录",
    );
    hint(
        front.repeat.is_none(),
        "repeat: weekly on mon at 10:00   # daily | weekdays | weekly on mon,thu | monthly on 1 | yearly on 03-15",
        "repeat: weekly on mon at 10:00   # daily | weekdays | weekly on mon,thu | monthly on 1 | yearly on 03-15",
    );
    hint(
        front.color.is_none(),
        "color: \"#4A3B6B\"                 # tile colour override",
        "color: \"#4A3B6B\"                 # 自定义磁贴颜色",
    );
    hint(
        front.arxiv.is_none() && front.doi.is_none(),
        "arxiv: 2401.12345                # or doi: 10.1109/TIE.2024.1234567 (fetches the paper)",
        "arxiv: 2401.12345                # 或 doi: 10.1109/TIE.2024.1234567（自动获取文献信息）",
    );
    lines
}

/// Example body for a new tip.
pub fn body(zh: bool) -> String {
    if zh {
        "在这里写说明。下面都是示例，用不到的可以删掉。

## 清单

- [ ] 第一步
- [ ] 第二步

## 图表

把文件放到 `figures/` 目录，在上方 `images:` 中列出，或直接内嵌：

<!-- ![示例图](figures/plot.svg) -->

## 链接

- https://example.org
"
        .to_string()
    } else {
        "Describe the tip here. Everything below is an example; delete what you do not need.

## Checklist

- [ ] First step
- [ ] Second step

## Figures

Put files under `figures/`, list them in `images:` above, or embed one inline:

<!-- ![example plot](figures/plot.svg) -->

## Links

- https://example.org
"
        .to_string()
    }
}

/// Full file text for a new tip, plus the parsed tip the board should hold.
pub fn render(path: &str, front: FrontMatter, zh: bool) -> Result<(Tip, String), TipError> {
    let yaml = serde_yaml::to_string(&front)?;
    let hints = key_hints(&front, zh).join("\n");
    let text = format!("---\n{}{}\n---\n\n{}", yaml, hints, body(zh));
    let tip = Tip::parse(path, None, &text)?;
    Ok((tip, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn front() -> FrontMatter {
        FrontMatter {
            title: "Calibrate torque sensor".into(),
            tags: vec!["lab".into()],
            due: Some("2026-10-02".into()),
            ..Default::default()
        }
    }

    #[test]
    fn template_parses_back_to_the_same_frontmatter() {
        let (tip, text) = render("tips/x.md", front(), false).unwrap();
        assert_eq!(tip.front, front());
        assert!(tip.due_at.is_some());
        assert!(text.starts_with("---\n"));
        assert!(text.contains("## Checklist"));
        // Hints only for keys that are not set.
        assert!(!text.contains("# tags:"));
        assert!(!text.contains("# due:"));
        assert!(text.contains("# location:"));
        assert!(text.contains("# repeat:"));
        // Reparsing the canonical form drops comments but keeps the body.
        let again = Tip::parse("tips/x.md", None, &tip.to_markdown().unwrap()).unwrap();
        assert_eq!(again.body, tip.body);
    }

    #[test]
    fn chinese_template() {
        let (tip, text) = render("tips/y.md", front(), true).unwrap();
        assert!(text.contains("## 清单"));
        assert!(text.contains("可选键"));
        assert_eq!(tip.front.title, "Calibrate torque sensor");
    }
}
