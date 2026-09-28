//! Changes-list filtering - GHD `ui/changes/filter-changes-logic.ts` plus the
//! fuzzy text match of `lib/fuzzy-find.ts` (fuzzaldrin-plus, approximated:
//! ordered subsequence with bonuses for consecutive and boundary hits).

use corvane_models::{FileStatusKind, WorkingDirectoryFileChange};

use crate::state::{FileListFilter, FilterOption};

/// `0 < score <= 1` when every query char occurs in order in `text`.
pub fn fuzzy_score(query: &str, text: &str) -> Option<f32> {
    let query: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    if query.is_empty() {
        return Some(1.0);
    }
    let text: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut score = 0.0f32;
    let mut qi = 0usize;
    let mut prev_hit: Option<usize> = None;
    for (ti, &c) in text.iter().enumerate() {
        if qi < query.len() && c == query[qi] {
            let boundary = ti == 0 || matches!(text[ti - 1], '/' | '.' | '-' | '_' | ' ');
            let consecutive = prev_hit.is_some_and(|p| p + 1 == ti);
            score += 1.0 + if consecutive { 1.0 } else { 0.0 } + if boundary { 0.5 } else { 0.0 };
            prev_hit = Some(ti);
            qi += 1;
        }
    }
    if qi < query.len() {
        return None;
    }
    // best case: every hit consecutive from a boundary
    let max = query.len() as f32 * 2.5;
    // prefer shorter paths for equal hits
    Some(
        (score / max)
            * (query.len() as f32 / text.len() as f32)
                .sqrt()
                .clamp(0.05, 1.0),
    )
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

/// Files that pass the option filters and fuzzy-match `text`, best match first
/// (original order when `text` is empty).
pub fn filtered_files<'a>(
    files: &'a [WorkingDirectoryFileChange],
    text: &str,
    filter: &FileListFilter,
) -> Vec<&'a WorkingDirectoryFileChange> {
    let text = text.trim();
    let mut scored: Vec<(f32, &WorkingDirectoryFileChange)> = files
        .iter()
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
