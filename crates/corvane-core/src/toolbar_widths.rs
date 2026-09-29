//! Resizable toolbar buttons - GHD `app-store.ts#updateResizableConstraints`
//! (the worktree / branch part) with `enableResizingToolbarButtons`: the
//! worktree dropdown is allotted first, then the branch dropdown, and neither
//! may push the push/pull button (230 px) off the window. Widths are
//! persisted in the settings (`branch-dropdown-width`,
//! `worktree-dropdown-width`); resetting removes them.

/// `defaultBranchDropdownWidth`, `defaultWorktreeDropdownWidth`,
/// `defaultPushPullButtonWidth`
pub const DEFAULT_DROPDOWN_WIDTH: f32 = 230.;

/// GHD `IConstrainedValue`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstrainedWidth {
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

impl ConstrainedWidth {
    /// GHD `clamp(IConstrainedValue)`: below `min` is `min`, above `max` is
    /// `max` (so `min` wins when the window is too narrow for both).
    pub fn clamped(&self) -> f32 {
        clamp(self.value, self.min, self.max)
    }

    /// A dragged width, kept within the constraints (`Resizable.clampWidth`).
    pub fn clamp(&self, width: f32) -> f32 {
        clamp(width, self.min, self.max)
    }
}

fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// The toolbar dropdown widths for a window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolbarWidths {
    pub worktree: ConstrainedWidth,
    pub branch: ConstrainedWidth,
}

/// `updateResizableConstraints` for a `window_width` wide window whose
/// sidebar (and repository button) is `sidebar_width` wide.
pub fn toolbar_widths(
    window_width: f32,
    sidebar_width: f32,
    worktree_shown: bool,
    worktree_width: Option<f32>,
    branch_width: Option<f32>,
) -> ToolbarWidths {
    let buttons = if worktree_shown { 3. } else { 2. };
    let available = window_width - sidebar_width;
    let worktree = ConstrainedWidth {
        value: worktree_width.unwrap_or(DEFAULT_DROPDOWN_WIDTH),
        min: (available / buttons - 10.).min(170.),
        max: available - DEFAULT_DROPDOWN_WIDTH - DEFAULT_DROPDOWN_WIDTH,
    };
    let branch_max = available
        - if worktree_shown {
            worktree.clamped()
        } else {
            0.
        }
        - DEFAULT_DROPDOWN_WIDTH;
    let branch_min = if DEFAULT_DROPDOWN_WIDTH > available / buttons {
        available / buttons - 10.
    } else {
        DEFAULT_DROPDOWN_WIDTH
    };
    ToolbarWidths {
        worktree,
        branch: ConstrainedWidth {
            value: branch_width.unwrap_or(DEFAULT_DROPDOWN_WIDTH),
            min: branch_min,
            max: branch_max,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fit_the_reference_window() {
        let w = toolbar_widths(1367., 250., false, None, None);
        assert_eq!(w.branch.clamped(), 230.);
        assert_eq!(w.branch.min, 230.);
        // 1117 left: the branch may grow until push/pull is 230 px from the edge
        assert_eq!(w.branch.max, 1117. - 230.);
    }

    #[test]
    fn worktree_is_allotted_before_the_branch() {
        let w = toolbar_widths(1367., 250., true, Some(500.), Some(900.));
        assert_eq!(w.worktree.min, 170.);
        assert_eq!(w.worktree.clamped(), 500.);
        // 1117 - 500 (worktree) - 230 (push/pull)
        assert_eq!(w.branch.max, 387.);
        assert_eq!(w.branch.clamped(), 387.);
    }

    #[test]
    fn narrow_windows_shrink_the_minimums() {
        // 960 px window, 300 px sidebar: 660 px for three buttons
        let w = toolbar_widths(960., 300., true, None, None);
        assert_eq!(w.worktree.min, 170.);
        assert_eq!(w.worktree.max, 660. - 460.);
        assert_eq!(w.worktree.clamped(), 200.);
        // 230 > 660 / 3, so the branch minimum drops below the default
        assert_eq!(w.branch.min, 210.);
        assert_eq!(w.branch.max, 660. - 200. - 230.);
        // a drag clamps into the range
        assert_eq!(w.worktree.clamp(1000.), 200.);
        assert_eq!(w.worktree.clamp(10.), 170.);
    }
}
