//! Primer Octicons (16 px) bundled in `assets/octicons/`.

use std::time::Duration;

use gpui_kit::{
    Animation, AnimationExt, AnyElement, ElementId, Hsla, IntoElement, Styled, Svg, Transformation,
    percentage, svg,
};

use crate::theme::sizes::ICON_SIZE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Octicon {
    Repo,
    RepoForked,
    RepoClone,
    RepoPush,
    RepoPull,
    GitBranch,
    GitCommit,
    GitMerge,
    GitCompare,
    GitPullRequest,
    Sync,
    /// GHD `syncClockwise`: `sync` flipped horizontally, used by `Loading`.
    SyncClockwise,
    ArrowUp,
    ArrowDown,
    Upload,
    TriangleDown,
    ChevronDown,
    ChevronRight,
    Filter,
    Search,
    Check,
    CheckCircleFill,
    XCircleFill,
    DiffAdded,
    DiffModified,
    DiffRemoved,
    DiffRenamed,
    DiffIgnored,
    FileDiff,
    Gear,
    PersonAdd,
    X,
    Plus,
    FileDirectory,
    Lock,
    DeviceDesktop,
    Copy,
    Fold,
    FoldUp,
    FoldDown,
    Unfold,
    DotFill,
    KebabHorizontal,
    Question,
    Bell,
    Paintbrush,
    Plug,
    Home,
    Accessibility,
    Tag,
    History,
    Alert,
    Info,
    IssueOpened,
    LinkExternal,
    Terminal,
    FileCode,
    Pencil,
    Trash,
    Undo,
    CloudOffline,
    Eye,
    DesktopDownload,
    FileSymlinkFile,
    FileSubmodule,
    FileBinary,
    Image,
    /// GHD custom 12 px check/dash for the diff gutter (`ui/octicons/diff.ts`).
    DiffCheck,
    DiffDash,
    /// GHD custom stash icon (`filter-changes-list.tsx` `StashIcon`).
    Stash,
    Server,
    File,
    Person,
    GitPullRequestDraft,
    Stop,
    Skip,
    SquareFill,
    IssueReopened,
    ChevronUp,
    ListUnordered,
    MortarBoard,
    Telescope,
}

impl Octicon {
    pub fn path(self) -> &'static str {
        match self {
            Octicon::Repo => "octicons/repo-16.svg",
            Octicon::RepoForked => "octicons/repo-forked-16.svg",
            Octicon::RepoClone => "octicons/repo-clone-16.svg",
            Octicon::RepoPush => "octicons/repo-push-16.svg",
            Octicon::RepoPull => "octicons/repo-pull-16.svg",
            Octicon::GitBranch => "octicons/git-branch-16.svg",
            Octicon::GitCommit => "octicons/git-commit-16.svg",
            Octicon::GitMerge => "octicons/git-merge-16.svg",
            Octicon::GitCompare => "octicons/git-compare-16.svg",
            Octicon::GitPullRequest => "octicons/git-pull-request-16.svg",
            Octicon::Sync => "octicons/sync-16.svg",
            Octicon::SyncClockwise => "octicons/sync-clockwise-16.svg",
            Octicon::ArrowUp => "octicons/arrow-up-16.svg",
            Octicon::ArrowDown => "octicons/arrow-down-16.svg",
            Octicon::Upload => "octicons/upload-16.svg",
            Octicon::TriangleDown => "octicons/triangle-down-16.svg",
            Octicon::ChevronDown => "octicons/chevron-down-16.svg",
            Octicon::ChevronRight => "octicons/chevron-right-16.svg",
            Octicon::Filter => "octicons/filter-16.svg",
            Octicon::Search => "octicons/search-16.svg",
            Octicon::Check => "octicons/check-16.svg",
            Octicon::CheckCircleFill => "octicons/check-circle-fill-16.svg",
            Octicon::XCircleFill => "octicons/x-circle-fill-16.svg",
            Octicon::DiffAdded => "octicons/diff-added-16.svg",
            Octicon::DiffModified => "octicons/diff-modified-16.svg",
            Octicon::DiffRemoved => "octicons/diff-removed-16.svg",
            Octicon::DiffRenamed => "octicons/diff-renamed-16.svg",
            Octicon::DiffIgnored => "octicons/diff-ignored-16.svg",
            Octicon::FileDiff => "octicons/file-diff-16.svg",
            Octicon::Gear => "octicons/gear-16.svg",
            Octicon::PersonAdd => "octicons/person-add-16.svg",
            Octicon::X => "octicons/x-16.svg",
            Octicon::Plus => "octicons/plus-16.svg",
            Octicon::FileDirectory => "octicons/file-directory-16.svg",
            Octicon::Lock => "octicons/lock-16.svg",
            Octicon::DeviceDesktop => "octicons/device-desktop-16.svg",
            Octicon::Copy => "octicons/copy-16.svg",
            Octicon::Fold => "octicons/fold-16.svg",
            Octicon::FoldUp => "octicons/fold-up-16.svg",
            Octicon::FoldDown => "octicons/fold-down-16.svg",
            Octicon::Unfold => "octicons/unfold-16.svg",
            Octicon::DotFill => "octicons/dot-fill-16.svg",
            Octicon::KebabHorizontal => "octicons/kebab-horizontal-16.svg",
            Octicon::Question => "octicons/question-16.svg",
            Octicon::Bell => "octicons/bell-16.svg",
            Octicon::Paintbrush => "octicons/paintbrush-16.svg",
            Octicon::Plug => "octicons/plug-16.svg",
            Octicon::Home => "octicons/home-16.svg",
            Octicon::Accessibility => "octicons/accessibility-16.svg",
            Octicon::Tag => "octicons/tag-16.svg",
            Octicon::History => "octicons/history-16.svg",
            Octicon::Alert => "octicons/alert-16.svg",
            Octicon::DiffCheck => "octicons/diff-check-12.svg",
            Octicon::DiffDash => "octicons/diff-dash-12.svg",
            Octicon::Stash => "octicons/stash-16.svg",
            Octicon::Server => "octicons/server-16.svg",
            Octicon::File => "octicons/file-16.svg",
            Octicon::Person => "octicons/person-16.svg",
            Octicon::GitPullRequestDraft => "octicons/git-pull-request-draft-16.svg",
            Octicon::Stop => "octicons/stop-16.svg",
            Octicon::Skip => "octicons/skip-16.svg",
            Octicon::SquareFill => "octicons/square-fill-16.svg",
            Octicon::IssueReopened => "octicons/issue-reopened-16.svg",
            Octicon::ChevronUp => "octicons/chevron-up-16.svg",
            Octicon::Info => "octicons/info-16.svg",
            Octicon::IssueOpened => "octicons/issue-opened-16.svg",
            Octicon::LinkExternal => "octicons/link-external-16.svg",
            Octicon::ListUnordered => "octicons/list-unordered-16.svg",
            Octicon::MortarBoard => "octicons/mortar-board-16.svg",
            Octicon::Telescope => "octicons/telescope-16.svg",
            Octicon::Terminal => "octicons/terminal-16.svg",
            Octicon::FileCode => "octicons/file-code-16.svg",
            Octicon::Pencil => "octicons/pencil-16.svg",
            Octicon::Trash => "octicons/trash-16.svg",
            Octicon::Undo => "octicons/undo-16.svg",
            Octicon::CloudOffline => "octicons/cloud-offline-16.svg",
            Octicon::Eye => "octicons/eye-16.svg",
            Octicon::DesktopDownload => "octicons/desktop-download-16.svg",
            Octicon::FileSymlinkFile => "octicons/file-symlink-file-16.svg",
            Octicon::FileSubmodule => "octicons/file-submodule-16.svg",
            Octicon::FileBinary => "octicons/file-binary-16.svg",
            Octicon::Image => "octicons/image-16.svg",
        }
    }
}

/// A 16 px octicon filled with `color` (`svg.octicon { fill: currentColor }`).
/// GPUI's `svg()` does not inherit the parent's text colour, so it is explicit.
pub fn octicon(icon: Octicon, color: Hsla) -> Svg {
    svg()
        .path(icon.path())
        .size(ICON_SIZE())
        .flex_none()
        .text_color(color)
}

/// Spins `icon` like GHD's `.spin` class (`animation: spin 1s linear
/// infinite`, `ui/toolbar/_toolbar.scss`). `id` keys the animation state and
/// must be unique among its siblings.
pub fn spin(icon: Svg, id: impl Into<ElementId>) -> AnyElement {
    icon.with_animation(
        id,
        Animation::new(Duration::from_secs(1)).repeat(),
        |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
    )
    .into_any_element()
}

/// GHD `Loading` (`ui/lib/loading.tsx`): a spinning `syncClockwise`.
pub fn loading(id: impl Into<ElementId>, color: Hsla) -> AnyElement {
    spin(octicon(Octicon::SyncClockwise, color), id)
}
