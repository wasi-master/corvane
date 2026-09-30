//! Paths Windows cannot check out (`284-windows-invalid-names-warning`, a
//! Corvane addition; GHD 3.6.6 commits them silently): reserved device names,
//! characters NTFS rejects, and names ending in a space or a dot.

/// Why Windows rejects `path` (repository-relative, `/`-separated), or `None`
/// when every component is valid there.
pub fn windows_invalid_reason(path: &str) -> Option<&'static str> {
    path.split('/').find_map(component_reason)
}

fn component_reason(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return None;
    }
    if name
        .chars()
        .any(|c| c < ' ' || matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*'))
    {
        return Some("contains a character Windows does not allow");
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Some("ends with a space or a dot");
    }
    // `CON`, `nul.txt`, `COM1.tar.gz`: the part before the first dot, spaces
    // trimmed, is what Windows compares
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    reserved.then_some("is a name Windows reserves")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_names() {
        assert_eq!(windows_invalid_reason("src/main.rs"), None);
        assert_eq!(windows_invalid_reason(".github/workflows/ci.yml"), None);
        assert_eq!(windows_invalid_reason("docs/console.md"), None);
        assert_eq!(windows_invalid_reason("COM10"), None);
        assert_eq!(windows_invalid_reason("com0"), None);
        assert!(windows_invalid_reason("notes/a:b.txt").is_some());
        assert!(windows_invalid_reason("what?.md").is_some());
        assert!(windows_invalid_reason("folder /file.txt").is_some());
        assert!(windows_invalid_reason("file.").is_some());
        assert!(windows_invalid_reason("lib/nul.txt").is_some());
        assert!(windows_invalid_reason("Con").is_some());
        assert!(windows_invalid_reason("LPT1.tar.gz").is_some());
    }
}
