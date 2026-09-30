//! Keyboard range selection for multi-select lists - GHD
//! `ui/lib/list/list.tsx` (`addSelection`, `handleKeyDown` with `shiftKey`)
//! and `ui/lib/list/selection.ts` (`createSelectionBetween`).
//!
//! GHD keeps the selected rows ordered from the selection origin
//! (`selectedRows[0]`) to the row that moves (`selectedRows.at(-1)`). Corvane
//! stores paths in the same order; the origin is the repository state's
//! `selected_file` (the ⇧-click anchor), the moving end is the last path.
//!
//! Deviation: [`extend_keeping`] keeps ⌘-clicked rows on ⇧-click
//! (`707-shift-click-keeps-selection`).

/// `createSelectionBetween`: the rows from `from` to `to` inclusive, in the
/// direction of travel (so `from` comes first).
pub fn selection_between(order: &[String], from: usize, to: usize) -> Vec<String> {
    if order.is_empty() {
        return Vec::new();
    }
    let last = order.len() - 1;
    let (from, to) = (from.min(last), to.min(last));
    if from <= to {
        order[from..=to].to_vec()
    } else {
        order[to..=from].iter().rev().cloned().collect()
    }
}

/// `addSelection`: move the end of the selection one row up (`delta < 0`) or
/// down and select everything between the origin and the new end. `None` when
/// nothing changes (the end is already at the edge, or `anchor` is not
/// visible). Without a wrap: GHD passes `wrap: false`.
pub fn extend_selection(
    order: &[String],
    anchor: &str,
    selected: &[String],
    delta: isize,
) -> Option<Vec<String>> {
    let origin = order.iter().position(|p| p == anchor)?;
    let end = selected
        .last()
        .and_then(|last| order.iter().position(|p| p == last))
        .unwrap_or(origin);
    let next = end.checked_add_signed(delta)?;
    if next >= order.len() {
        return None;
    }
    Some(selection_between(order, origin, next))
}

/// ⇧-click that keeps ⌘-clicked rows outside the range (Corvane
/// `707-shift-click-keeps-selection`, like Finder): the previous range from
/// `anchor` to the moving end (the last selected path) is replaced by the one
/// from `anchor` to `to`; other selected rows stay, before the range so the
/// moving end is still last. `None` when `anchor` or `to` is not visible.
pub fn extend_keeping(
    order: &[String],
    anchor: &str,
    selected: &[String],
    to: &str,
) -> Option<Vec<String>> {
    let origin = order.iter().position(|p| p == anchor)?;
    let target = order.iter().position(|p| p == to)?;
    let end = selected
        .last()
        .and_then(|last| order.iter().position(|p| p == last))
        .unwrap_or(origin);
    let previous = selection_between(order, origin, end);
    let range = selection_between(order, origin, target);
    let mut out: Vec<String> = selected
        .iter()
        .filter(|p| !previous.contains(p) && !range.contains(p))
        .cloned()
        .collect();
    out.extend(range);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> Vec<String> {
        ["a", "b", "c", "d", "e"].map(String::from).to_vec()
    }

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn between_keeps_direction() {
        assert_eq!(selection_between(&order(), 1, 3), v(&["b", "c", "d"]));
        assert_eq!(selection_between(&order(), 3, 1), v(&["d", "c", "b"]));
        assert_eq!(selection_between(&order(), 2, 2), v(&["c"]));
    }

    #[test]
    fn extend_down_then_back_up() {
        let o = order();
        let s = extend_selection(&o, "b", &v(&["b"]), 1).unwrap();
        assert_eq!(s, v(&["b", "c"]));
        let s = extend_selection(&o, "b", &s, 1).unwrap();
        assert_eq!(s, v(&["b", "c", "d"]));
        let s = extend_selection(&o, "b", &s, -1).unwrap();
        assert_eq!(s, v(&["b", "c"]));
        let s = extend_selection(&o, "b", &s, -1).unwrap();
        assert_eq!(s, v(&["b"]));
        // past the origin the range grows the other way
        let s = extend_selection(&o, "b", &s, -1).unwrap();
        assert_eq!(s, v(&["b", "a"]));
    }

    #[test]
    fn extend_stops_at_edges() {
        let o = order();
        assert_eq!(extend_selection(&o, "a", &v(&["a"]), -1), None);
        assert_eq!(extend_selection(&o, "e", &v(&["e"]), 1), None);
    }

    #[test]
    fn extend_from_toggled_selection_uses_last_path_as_end() {
        // ⌘-click selections are unordered; the last toggled path is the end
        let o = order();
        let s = extend_selection(&o, "d", &v(&["a", "d"]), 1).unwrap();
        assert_eq!(s, v(&["d", "e"]));
    }

    #[test]
    fn shift_click_keeps_toggled_rows() {
        let o = order();
        // click a, ⌘-click c, ⇧-click e: a stays, c..e selected
        let s = extend_keeping(&o, "c", &v(&["a", "c"]), "e").unwrap();
        assert_eq!(s, v(&["a", "c", "d", "e"]));
        // ⇧-click d instead: the previous range c..e shrinks to c..d
        let s = extend_keeping(&o, "c", &s, "d").unwrap();
        assert_eq!(s, v(&["a", "c", "d"]));
        // ⇧-click above the anchor: the range flips, a is part of it
        let s = extend_keeping(&o, "c", &s, "a").unwrap();
        assert_eq!(s, v(&["c", "b", "a"]));
        assert_eq!(extend_keeping(&o, "z", &s, "a"), None);
    }

    #[test]
    fn hidden_anchor_is_ignored() {
        assert_eq!(extend_selection(&order(), "z", &v(&["z"]), 1), None);
    }
}
