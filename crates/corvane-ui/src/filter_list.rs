//! Keyboard selection shared by the foldouts' filter lists (GHD
//! `ui/lib/filter-list.tsx` `onFilterKeyDown`): ↓ / ↑ in the filter box move
//! a highlighted row through the list, Enter picks it.
//!
//! Deviation: GHD moves the focus into the list on ↓ / ↑; Corvane keeps the
//! caret in the filter box and highlights the row, so typing keeps
//! filtering.

use gpui_kit::{Pixels, ScrollHandle};

/// GHD `ui/lib/filter-list.tsx` `onFilterKeyDown` (ArrowDown / ArrowUp): the
/// next highlighted row of `count` selectable rows. ↓ from the filter starts
/// at the first row, ↑ at the last; further moves wrap around like the
/// list's own ([`wrap_step`]).
pub fn step(current: Option<usize>, delta: isize, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match current {
        Some(ix) => wrap_step(ix, delta, count),
        None if delta < 0 => count - 1,
        None => 0,
    })
}

/// GHD `List.moveSelection` (`findNextSelectableRow`, `wrap` defaults to
/// true): ↓ on the last row goes to the first, ↑ on the first to the last.
/// Extending a range (`addSelection`, `wrap: false`) clamps instead.
pub fn wrap_step(ix: usize, delta: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    (ix as isize + delta).rem_euclid(count as isize) as usize
}

/// The top of selectable row `ix` in a list of groups of `group_sizes`
/// rows, each group preceded by a `header` tall header (group headers are
/// not selectable, `canSelectRow`).
pub fn row_top(group_sizes: &[usize], ix: usize, header: Pixels, row: Pixels) -> Pixels {
    let mut headers = 0;
    let mut before = 0;
    for &size in group_sizes {
        if size == 0 {
            continue;
        }
        headers += 1;
        if ix < before + size {
            break;
        }
        before += size;
    }
    header * headers as f32 + row * ix as f32
}

/// Scroll `handle` so the content span `top..top + height` is visible.
pub fn scroll_into_view(handle: &ScrollHandle, top: Pixels, height: Pixels) {
    let bottom = top + height;
    let viewport = handle.bounds().size.height;
    let mut offset = handle.offset();
    if top < -offset.y {
        offset.y = -top;
    } else if bottom > -offset.y + viewport {
        offset.y = viewport - bottom;
    }
    handle.set_offset(offset);
}

#[cfg(test)]
mod tests {
    use super::{row_top, step};
    use gpui_kit::px;

    #[test]
    fn step_starts_at_the_ends_and_wraps() {
        assert_eq!(step(None, 1, 0), None);
        assert_eq!(step(None, 1, 3), Some(0));
        assert_eq!(step(None, -1, 3), Some(2));
        assert_eq!(step(Some(2), 1, 3), Some(0));
        assert_eq!(step(Some(0), -1, 3), Some(2));
        assert_eq!(step(Some(1), 1, 3), Some(2));
    }

    #[test]
    fn row_top_counts_the_headers_above() {
        let (h, r) = (px(10.), px(30.));
        assert_eq!(row_top(&[1, 2, 3], 0, h, r), px(10.));
        assert_eq!(row_top(&[1, 2, 3], 1, h, r), px(20.) + px(30.));
        assert_eq!(row_top(&[1, 2, 3], 3, h, r), px(30.) + px(90.));
        // an empty group has no header
        assert_eq!(row_top(&[0, 2], 1, h, r), px(10.) + px(30.));
    }
}
