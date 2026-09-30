//! Changes-list filtering - GHD `ui/changes/filter-changes-logic.ts` plus the
//! fuzzy text match of `lib/fuzzy-find.ts` (fuzzaldrin-plus, approximated:
//! ordered subsequence with bonuses for consecutive and boundary hits).
//!
//! Deviation: [`hidden_by`] hides files matching the `272-changes-hide-globs`
//! patterns from the list (view only; they are still committed).

use corvane_models::{FileStatusKind, WorkingDirectoryFileChange};

use crate::state::{FileListFilter, FilterOption};

/// `0 < score <= 1` when every query char occurs in order in `text`.
pub fn fuzzy_score(query: &str, text: &str) -> Option<f32> {
    fuzzy_match(query, text).map(|(score, _)| score)
}

/// [`fuzzy_score`] plus the matched positions (char indices into `text`),
/// GHD `fuzzAldrin.match` for `HighlightText`. Empty positions for an empty
/// query.
pub fn fuzzy_match(query: &str, text: &str) -> Option<(f32, Vec<usize>)> {
    let query: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    if query.is_empty() {
        return Some((1.0, Vec::new()));
    }
    // one lower-cased char per original char, so positions map back to `text`
    let text: Vec<char> = text
        .chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();
    let mut score = 0.0f32;
    let mut qi = 0usize;
    let mut prev_hit: Option<usize> = None;
    let mut hits = Vec::with_capacity(query.len());
    for (ti, &c) in text.iter().enumerate() {
        if qi < query.len() && c == query[qi] {
            let boundary = ti == 0 || matches!(text[ti - 1], '/' | '.' | '-' | '_' | ' ');
            let consecutive = prev_hit.is_some_and(|p| p + 1 == ti);
            score += 1.0 + if consecutive { 1.0 } else { 0.0 } + if boundary { 0.5 } else { 0.0 };
            prev_hit = Some(ti);
            hits.push(ti);
            qi += 1;
        }
    }
    if qi < query.len() {
        return None;
    }
    // best case: every hit consecutive from a boundary
    let max = query.len() as f32 * 2.5;
    // prefer shorter paths for equal hits
    let score = (score / max)
        * (query.len() as f32 / text.len() as f32)
            .sqrt()
            .clamp(0.05, 1.0);
    Some((score, hits))
}

/// GHD `BranchAutocompletionProvider.getAutocompletionItems`: every branch
/// for an empty query, else the fuzzy matches best first (`match` sorts by
/// descending score; ties keep the input order). Returns names with the
/// matched char positions.
pub fn branch_matches(query: &str, names: &[String]) -> Vec<(String, Vec<usize>)> {
    if query.is_empty() {
        return names.iter().map(|n| (n.clone(), Vec::new())).collect();
    }
    let mut hits: Vec<(f32, &String, Vec<usize>)> = names
        .iter()
        .filter_map(|n| fuzzy_match(query, n).map(|(score, pos)| (score, n, pos)))
        .collect();
    hits.sort_by(|a, b| b.0.total_cmp(&a.0));
    hits.into_iter()
        .map(|(_, n, pos)| (n.clone(), pos))
        .collect()
}

/// `applyFilterOptions`: every active option must match.
pub fn matches_options(filter: &FileListFilter, file: &WorkingDirectoryFileChange) -> bool {
    if filter.count_active() == 0 {
        return true;
    }
    let included = file.selection.kind() != corvane_models::DiffSelectionType::None;
    if filter.included && !included {
        return false;
    }
    if filter.excluded && included {
        return false;
    }
    if filter.new_files
        && !matches!(
            file.status.kind,
            FileStatusKind::New | FileStatusKind::Untracked
        )
    {
        return false;
    }
    if filter.modified && file.status.kind != FileStatusKind::Modified {
        return false;
    }
    if filter.deleted && file.status.kind != FileStatusKind::Deleted {
        return false;
    }
    true
}

/// The `272-changes-hide-globs` flag text split into patterns: separated by
/// commas or whitespace, empty ones dropped.
pub fn hide_patterns(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .map(|p| p.trim_end_matches('/'))
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether `path` (repository-relative, `/`-separated) matches one of the
/// patterns. Gitignore-like: `*` and `?` stay inside one path component, `**`
/// spans components; a pattern without `/` matches any component (`*.lock`,
/// `node_modules`), one with `/` matches from the root (a leading `/` is
/// optional), and a pattern matching a directory hides everything below it.
pub fn hidden_by(patterns: &[String], path: &str) -> bool {
    patterns.iter().any(|pattern| {
        if pattern.contains('/') {
            let pattern = pattern.trim_start_matches('/');
            // the path itself or one of its parent directories
            glob_match(pattern, path)
                || path
                    .match_indices('/')
                    .any(|(i, _)| glob_match(pattern, &path[..i]))
        } else {
            path.split('/')
                .any(|component| glob_match(pattern, component))
        }
    })
}

/// `*` / `?` without crossing `/`, `**` across it.
fn glob_match(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') if p.get(1) == Some(&'*') => {
                let rest = &p[2..];
                // `**/` also matches no directory at all
                let rest_no_slash = rest.strip_prefix(&['/']).unwrap_or(rest);
                (0..=t.len()).any(|i| go(rest, &t[i..]) || go(rest_no_slash, &t[i..]))
            }
            Some('*') => {
                let rest = &p[1..];
                for i in 0..=t.len() {
                    if go(rest, &t[i..]) {
                        return true;
                    }
                    if t.get(i) == Some(&'/') {
                        break;
                    }
                }
                false
            }
            Some('?') => t.first().is_some_and(|c| *c != '/') && go(&p[1..], &t[1..]),
            Some(c) => t.first() == Some(c) && go(&p[1..], &t[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
}

/// Files that pass the option filters and fuzzy-match `text`, best match first
/// (original order when `text` is empty). Files matching a `hide` pattern
/// ([`hidden_by`]) are left out.
pub fn filtered_files<'a>(
    files: &'a [WorkingDirectoryFileChange],
    text: &str,
    filter: &FileListFilter,
    hide: &[String],
) -> Vec<&'a WorkingDirectoryFileChange> {
    let text = text.trim();
    let mut scored: Vec<(f32, &WorkingDirectoryFileChange)> = files
        .iter()
        .filter(|f| hide.is_empty() || !hidden_by(hide, &f.path))
        .filter(|f| matches_options(filter, f))
        .filter_map(|f| fuzzy_score(text, &f.path).map(|s| (s, f)))
        .collect();
    if !text.is_empty() {
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    }
    scored.into_iter().map(|(_, f)| f).collect()
}

/// Count per option, as the popover labels show them (`getFilterCounts`).
pub fn option_count(option: FilterOption, files: &[WorkingDirectoryFileChange]) -> usize {
    let mut only = FileListFilter::default();
    only.set(option, true);
    files.iter().filter(|f| matches_options(&only, f)).count()
}

/// `getNoResultsMessage`
pub fn no_results_message(text: &str, filter: &FileListFilter) -> Option<String> {
    let mut active: Vec<String> = Vec::new();
    if !text.trim().is_empty() {
        active.push(format!("\"{}\"", text.trim()));
    }
    for (flag, label) in [
        (filter.included, "Included in commit"),
        (filter.excluded, "Excluded from commit"),
        (filter.new_files, "New files"),
        (filter.modified, "Modified files"),
        (filter.deleted, "Deleted files"),
    ] {
        if flag {
            active.push(label.to_string());
        }
    }
    if active.is_empty() {
        return None;
    }
    let list = match active.len() {
        1 => active[0].clone(),
        2 => format!("{} and {}", active[0], active[1]),
        n => format!("{}, and {}", active[..n - 1].join(", "), active[n - 1]),
    };
    Some(format!(
        "Sorry, I can't find any changed files matching the following filters: {list}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_matches_rank_and_highlight() {
        let names: Vec<String> = ["main", "feature/login", "origin/main", "fix-lint"]
            .map(String::from)
            .to_vec();
        assert_eq!(branch_matches("", &names).len(), 4);
        let hits = branch_matches("main", &names);
        assert_eq!(hits[0].0, "main");
        assert_eq!(hits[0].1, vec![0, 1, 2, 3]);
        assert_eq!(hits[1].0, "origin/main");
        assert_eq!(hits.len(), 2);
        let hits = branch_matches("FL", &names);
        assert!(
            hits.iter()
                .any(|(n, pos)| n == "fix-lint" && pos == &vec![0, 4])
        );
    }

    #[test]
    fn fuzzy_matches_in_order() {
        assert!(fuzzy_score("mrs", "src/main.rs").is_some());
        assert!(fuzzy_score("n", "notes.txt").is_some());
        assert!(fuzzy_score("no", "notes.txt").is_some());
        assert!(fuzzy_score("xyz", "src/main.rs").is_none());
        assert!(fuzzy_score("", "anything").is_some());
        let exact = fuzzy_score("main", "src/main.rs").unwrap();
        let scattered = fuzzy_score("main", "m/a/i/n/x.rs").unwrap();
        assert!(exact > scattered);
    }

    #[test]
    fn hide_globs() {
        let p = hide_patterns("*.lock, node_modules  dist/,/docs/*.md ,src/**/gen");
        assert_eq!(
            p,
            ["*.lock", "node_modules", "dist", "/docs/*.md", "src/**/gen"]
        );
        assert!(hidden_by(&p, "Cargo.lock"));
        assert!(hidden_by(&p, "web/yarn.lock"));
        assert!(!hidden_by(&p, "lockfile.rs"));
        assert!(hidden_by(&p, "node_modules/a/b.js"));
        assert!(hidden_by(&p, "web/node_modules/a.js"));
        assert!(hidden_by(&p, "dist/app.js"));
        assert!(!hidden_by(&p, "distribution/app.js"));
        assert!(hidden_by(&p, "docs/readme.md"));
        assert!(!hidden_by(&p, "docs/sub/readme.md"));
        assert!(!hidden_by(&p, "other/docs/readme.md"));
        assert!(hidden_by(&p, "src/gen/x.rs"));
        assert!(hidden_by(&p, "src/a/b/gen/x.rs"));
        assert!(!hidden_by(&p, "src/general.rs"));
        let q = hide_patterns("fil?.txt");
        assert!(hidden_by(&q, "a/file.txt"));
        assert!(!hidden_by(&q, "a/fi/e.txt"));
        assert!(hide_patterns("  ,  ").is_empty());
    }

    #[test]
    fn no_results_grammar() {
        let mut f = FileListFilter::default();
        assert!(no_results_message("", &f).is_none());
        f.included = true;
        f.deleted = true;
        assert_eq!(
            no_results_message("abc", &f).unwrap(),
            "Sorry, I can't find any changed files matching the following filters: \"abc\", Included in commit, and Deleted files"
        );
    }
}
