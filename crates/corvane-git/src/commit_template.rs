//! `commit.template`: the per-repository commit message template.
//!
//! GitHub Desktop 3.6.6 has no template support (nothing in `app/src` reads
//! `commit.template`; its commit form only restores a saved draft). Corvane
//! reads the template the way `git commit` does when it opens the editor
//! (`builtin/commit.c` `prepare_to_commit`, `template_file`): the path comes
//! from the repository's resolved config (local over global over system),
//! `~` is expanded, a relative path is taken from the worktree root. Corvane
//! commits with `git commit -F -`, which skips the editor's comment stripping,
//! so comment lines (`core.commentChar`, default `#`) are removed here.
//! Deviation noted in `.docs/deviations.md`.

use std::path::Path;

/// Templates larger than this are ignored (a misconfigured path to a big file
/// should not end up in the description box).
const MAX_TEMPLATE_BYTES: u64 = 64 * 1024;

/// The template text for `repo`, or `None` when unset, missing, unreadable,
/// oversized or only comments.
pub fn read(repo: &gix::Repository, workdir: &Path) -> Option<String> {
    let cfg = repo.config_snapshot();
    let path = cfg.trusted_path("commit.template").ok().flatten()?;
    let path = if path.is_relative() {
        workdir.join(path)
    } else {
        path
    };
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_TEMPLATE_BYTES {
        return None;
    }
    let raw = std::fs::read(&path).ok()?;
    let raw = String::from_utf8_lossy(&raw);
    let comment = comment_char(cfg.string("core.commentChar").map(|v| v.to_string()));
    strip_comments(&raw, comment)
}

/// `core.commentChar` (a single character; `auto` and invalid values mean `#`).
fn comment_char(value: Option<String>) -> char {
    value
        .and_then(|v| {
            let mut chars = v.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Some(c),
                _ => None,
            }
        })
        .unwrap_or('#')
}

/// git's `strbuf_stripspace` with comment stripping, except that inner blank
/// lines are kept so a template's layout (e.g. an empty line before a trailer
/// block) survives. Trailing whitespace on each line and leading and trailing
/// blank lines are removed.
pub fn strip_comments(raw: &str, comment: char) -> Option<String> {
    let lines: Vec<&str> = raw
        .lines()
        .filter(|line| !line.starts_with(comment))
        .map(str::trim_end)
        .collect();
    let text = lines.join("\n");
    let text = text.trim_end().trim_start_matches('\n');
    if text.trim().is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_comment_lines_and_trailing_blank_lines() {
        let raw = "# Why?\n\nFixes: \n\n# trailers\nCo-authored-by: \n\n\n";
        assert_eq!(
            strip_comments(raw, '#').as_deref(),
            Some("Fixes:\n\nCo-authored-by:")
        );
    }

    #[test]
    fn only_comments_is_none() {
        assert_eq!(strip_comments("# a\n# b\n\n", '#'), None);
        assert_eq!(strip_comments("", '#'), None);
    }

    #[test]
    fn honours_comment_char() {
        assert_eq!(
            strip_comments("; note\n# kept\n", ';').as_deref(),
            Some("# kept")
        );
        assert_eq!(comment_char(Some(";".into())), ';');
        assert_eq!(comment_char(Some("auto".into())), '#');
        assert_eq!(comment_char(None), '#');
    }

    #[test]
    fn reads_relative_template_from_repo_config() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let run = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        };
        run(&["init", "-q"]);
        std::fs::write(root.join(".gitmessage"), "# comment\nBody line\n").unwrap();
        run(&["config", "commit.template", ".gitmessage"]);
        let repo = gix::open(root).unwrap();
        assert_eq!(read(&repo, root).as_deref(), Some("Body line"));

        run(&["config", "commit.template", "missing.txt"]);
        let repo = gix::open(root).unwrap();
        assert_eq!(read(&repo, root), None);
    }
}
