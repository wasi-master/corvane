//! `ui/branches/ci-status.tsx` (`styles/ui/_ci-status.scss`): the little
//! check icon and its colour for a status / check run / job step.

use corvane_core::{CheckConclusion, CheckStatus};
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::{c, primer};

/// `getSymbolForCheck`
pub fn symbol_for(conclusion: Option<CheckConclusion>) -> Octicon {
    match conclusion {
        Some(CheckConclusion::TimedOut) | Some(CheckConclusion::Failure) => Octicon::X,
        Some(CheckConclusion::Neutral) | Some(CheckConclusion::Unknown) => Octicon::SquareFill,
        Some(CheckConclusion::Success) => Octicon::Check,
        Some(CheckConclusion::Cancelled) => Octicon::Stop,
        Some(CheckConclusion::ActionRequired) => Octicon::Alert,
        Some(CheckConclusion::Skipped) => Octicon::Skip,
        Some(CheckConclusion::Stale) => Octicon::IssueReopened,
        None => Octicon::DotFill,
    }
}

/// `.ci-status-<class>` colours: pending yellow, failures red, the rest grey,
/// success green.
pub fn color_for(conclusion: Option<CheckConclusion>) -> Hsla {
    match conclusion {
        None => c(primer::YELLOW_700),
        Some(CheckConclusion::TimedOut)
        | Some(CheckConclusion::ActionRequired)
        | Some(CheckConclusion::Failure) => c(primer::RED_500),
        Some(CheckConclusion::Success) => c(primer::GREEN_500),
        Some(_) => c(primer::GRAY_400),
    }
}

/// A conclusion only counts once the check completed (`getClassNameForCheck`).
pub fn effective_conclusion(
    status: CheckStatus,
    conclusion: Option<CheckConclusion>,
) -> Option<CheckConclusion> {
    conclusion.filter(|_| status == CheckStatus::Completed)
}

/// `CIStatus`: the 16 px icon.
pub fn ci_status(status: CheckStatus, conclusion: Option<CheckConclusion>) -> Svg {
    let conclusion = effective_conclusion(status, conclusion);
    octicon(symbol_for(conclusion), color_for(conclusion))
}

/// `getSymbolForLogStep`: job steps use the filled circles.
pub fn symbol_for_log_step(status: CheckStatus, conclusion: Option<CheckConclusion>) -> Octicon {
    match effective_conclusion(status, conclusion) {
        Some(CheckConclusion::Success) => Octicon::CheckCircleFill,
        Some(CheckConclusion::Failure) => Octicon::XCircleFill,
        other => symbol_for(other),
    }
}
