//! `.gitignore` edits - GHD `lib/git/gitignore.ts` (`appendIgnoreRule`,
//! `appendIgnoreFile`, `escapeGitSpecialCharacters`).

use std::path::Path;

use crate::error::Result;

/// Escape the characters git treats specially in a pattern: `[ ] ! * # ?`.
pub fn escape_gitignore_pattern(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, '[' | ']' | '!' | '*' | '#' | '?') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Append raw patterns to the root `.gitignore`, creating it if needed. Keeps
/// the file's existing line endings (GHD consults `core.autocrlf`).
pub fn append_ignore_rules(workdir: &Path, patterns: &[String]) -> Result<()> {
    let path = workdir.join(".gitignore");
    let mut text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if !text.is_empty() && !text.ends_with('\n') {
        text.push_str(eol);
    }
    for pattern in patterns {
        text.push_str(pattern);
        text.push_str(eol);
    }
    std::fs::write(&path, text)?;
    Ok(())
}

/// The root `.gitignore` text, `None` when the file does not exist
/// (GHD `readGitIgnoreAtRoot`).
pub fn read_gitignore(workdir: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(workdir.join(".gitignore")) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// GHD `saveGitIgnore`: empty text deletes the file; otherwise the text is
/// written with a trailing newline, using CRLF when `core.autocrlf` is on.
pub fn save_gitignore(workdir: &Path, text: &str, autocrlf: bool) -> Result<()> {
    let path = workdir.join(".gitignore");
    if text.is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        };
    }
    let eol = if autocrlf { "\r\n" } else { "\n" };
    let mut out = String::with_capacity(text.len() + 2);
    for line in text.replace("\r\n", "\n").split('\n') {
        out.push_str(line);
        out.push_str(eol);
    }
    // `split` yields a trailing empty piece when the text ends with a newline.
    while out.ends_with(&format!("{eol}{eol}")) {
        out.truncate(out.len() - eol.len());
    }
    std::fs::write(&path, out)?;
    Ok(())
}

/// Ignore file paths (escaped first), as the "Ignore File" menu items do.
pub fn append_ignore_files(workdir: &Path, paths: &[String]) -> Result<()> {
    let patterns: Vec<String> = paths.iter().map(|p| escape_gitignore_pattern(p)).collect();
    append_ignore_rules(workdir, &patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_special_characters() {
        assert_eq!(
            escape_gitignore_pattern("a[1]!*#?.txt"),
            "a\\[1\\]\\!\\*\\#\\?.txt"
        );
        assert_eq!(escape_gitignore_pattern("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn appends_with_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        append_ignore_rules(dir.path(), &["*.log".into()]).unwrap();
        std::fs::write(dir.path().join(".gitignore"), "*.log\nbuild").unwrap();
        append_ignore_files(dir.path(), &["dist".into(), "we!rd".into()]).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text, "*.log\nbuild\ndist\nwe\\!rd\n");
    }

    #[test]
    fn save_normalises_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_gitignore(dir.path()).unwrap(), None);
        save_gitignore(dir.path(), "a\nb", false).unwrap();
        assert_eq!(
            read_gitignore(dir.path()).unwrap().as_deref(),
            Some("a\nb\n")
        );
        save_gitignore(dir.path(), "a\r\nb\n", true).unwrap();
        assert_eq!(
            read_gitignore(dir.path()).unwrap().as_deref(),
            Some("a\r\nb\r\n")
        );
        save_gitignore(dir.path(), "", false).unwrap();
        assert_eq!(read_gitignore(dir.path()).unwrap(), None);
    }

    #[test]
    fn keeps_crlf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "a\r\n").unwrap();
        append_ignore_rules(dir.path(), &["b".into()]).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text, "a\r\nb\r\n");
    }
}
