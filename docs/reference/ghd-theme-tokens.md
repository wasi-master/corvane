# GitHub Desktop theme tokens → `corvane_ui::theme`

Extracted 2026-09-28 from `app/styles/_variables.scss` (light) and `app/styles/themes/_dark.scss` (dark) at `desktop/desktop@development` (matches 3.6.6). Verbatim SCSS copies live in `ghd-src/`.

## Primer palette (`primer-support` color-system, as vendored by GHD)

Verify against `node_modules/primer-support/lib/variables/color-system.scss` when porting; values below are the v4-era scale GHD uses.

| Scale | 000 | 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 |
|---|---|---|---|---|---|---|---|---|---|---|
| gray | #fafbfc | #f6f8fa | #e1e4e8 | #d1d5da | #959da5 | #6a737d | #586069 | #444d56 | #2f363d | #24292e |
| blue | #f1f8ff | #dbedff | #c8e1ff | #79b8ff | #2188ff | #0366d6 | #005cc5 | #044289 | #032f62 | #05264c |
| green | #f0fff4 | #dcffe4 | #bef5cb | #85e89d | #34d058 | #28a745 | #22863a | #176f2c | #165c26 | #144620 |
| red | #ffeef0 | #ffdce0 | #fdaeb7 | #f97583 | #ea4a5a | #d73a49 | #cb2431 | #b31d28 | #9e1c23 | #86181d |
| yellow | #fffdef | #fffbdd | #fff5b1 | #ffea7f | #ffdf5d | #ffd33d | #f9c513 | #dbab09 | #b08800 | #735c0f |
| orange | #fff8f2 | #ffebda | #ffd1ac | #ffab70 | #fb8532 | #f66a0a | #e36209 | #d15704 | #c24e00 | #a04100 |

Aliases: `$blue` = blue-500, `$green` = green-500, `$red` = red-500, `$yellow` = yellow-500, `$orange` = orange-500, `$white` = #fff, `$black` = #1b1f23, `$link-color` = `$blue`. `darken()/lighten()` are Sass HSL operations; precompute when porting.

## Token table (light | dark)

| Token | Light | Dark |
|---|---|---|
| color-new | green-600 | green |
| color-deleted | red-600 | red |
| color-modified | darken(yellow-700, 10%) | yellow-700 |
| color-renamed | blue | blue |
| color-conflicted | orange-800 | orange |
| text-color | gray-900 | gray-100 |
| text-secondary-color | gray-500 | gray-400 |
| text-secondary-color-muted | lighten(gray-500, 30%) | darken(gray-500, 10%) |
| background-color | white | gray-900 |
| button-background | blue | blue |
| button-hover-background | lighten(blue, 5%) | lighten(blue, 5%) |
| button-text-color | white | white |
| button-focus-border-color | blue-100 | blue-600 |
| link-button-color | lighten(blue, 5%) | link-color |
| link-button-hover-color | blue-600 | lighten(link-color, 3%) |
| secondary-button-background | gray-100 | gray-800 |
| secondary-button-border-color | box-border-contrast-color | box-border-contrast-color |
| secondary-button-hover-background | white | (same as bg) |
| secondary-button-hover-border-color | box-border-contrast-color | gray-300 |
| secondary-button-focus-shadow-color | rgba(gray-200,.75) | rgba(gray-200,.75) |
| badge-icon-color | white | gray-300 |
| input-icon-warning-color | yellow-800 | yellow-600 |
| input-icon-error-color | red-600 | red-400 |
| input-icon-hover-background-color | gray-100 | gray-800 |
| scroll-bar-thumb-background-color | rgba(0,0,0,.2) | rgba(255,255,255,.2) |
| scroll-bar-thumb-background-color-active | rgba(0,0,0,.5) | rgba(255,255,255,.5) |
| box-background-color | background-color | darken(gray-900, 3%) |
| box-alt-background-color | gray-100 | lighten(gray-900, 3%) |
| box-skeleton-background-color | gray-200 | gray-700 |
| box-border-color | gray-200 | #141414 |
| box-border-contrast-color | darken(gray-400, 5%) | lighten(gray-500, 3%) |
| box-border-accent-color | blue | blue |
| box-selected-background-color | #ebeef1 | gray-700 |
| box-selected-text-color | gray-900 | text-color |
| box-selected-border-color | gray-400 | gray-400 |
| box-selected-active-background-color | blue | blue |
| box-selected-active-text-color | white | white |
| box-selected-active-border-color | gray-400 | gray-400 |
| box-hover-background-color | gray-100 | gray-800 |
| box-hover-text-color | gray-900 | text-color |
| box-placeholder-color | gray-500 | gray-400 |
| co-author-tag-background-color | blue-000 | blue-800 |
| co-author-tag-border-color | blue-200 | blue-700 |
| commit-warning-badge-background-color | gray-000 | gray-900 |
| commit-warning-badge-border-color | gray-300 | gray-700 |
| shadow-color | rgba(71,83,95,.19) | rgba(0,0,0,.5) |
| base-box-shadow | 0 2px 7px shadow-color | same |
| toolbar-background-color | gray-900 | darken(gray-900, 3%) |
| toolbar-border-color | gray-900 | box-border-color |
| toolbar-text-color | white | text-color |
| toolbar-text-secondary-color | gray-300 | text-secondary-color |
| toolbar-button-border-color | black | box-border-color |
| toolbar-button-hover-color | white | white |
| toolbar-button-hover-background-color | gray-800 | gray-800 |
| toolbar-button-focus-background-color | gray-800 | gray-800 |
| toolbar-button-active-color | text-color | text-color |
| toolbar-button-active-background-color | background-color | background-color |
| toolbar-button-progress-color | gray-800 | gray-800 |
| toolbar-button-hover-progress-color | gray-700 | gray-700 |
| toolbar-dropdown-open-progress-color | gray-200 | gray-200 |
| toolbar-dropdown-text-warning-color | orange-800 | orange-600 |
| toolbar-tooltip-background-color | gray-800 | gray-800 |
| toolbar-badge-background-color | gray-600 | gray-700 |
| toolbar-badge-active-background-color | gray-200 | gray-700 |
| tab-bar-active-color | blue | blue |
| tab-bar-background-color | white | box-background-color |
| tab-bar-hover-background-color | gray-100 | gray-800 |
| tab-bar-count-color | text-color | text-color |
| tab-bar-count-background-color | gray-200 | gray-700 |
| list-item-badge-color | gray-800 | text-color |
| list-item-badge-background-color | gray-200 | gray-600 |
| list-item-selected-badge-color | gray-900 | white |
| list-item-selected-badge-background-color | gray-300 | gray-500 |
| list-item-selected-active-badge-color | gray-900 | gray-900 |
| list-item-selected-active-badge-background-color | white | white |
| list-item-hover-background-color | gray-100 | gray-800 |
| toast-notification-background-color | (rgba black .8) | rgba(0,0,0,.8) |
| focus-color | blue | blue |
| accent-color | blue-400 | blue-200 |
| diff-linenumber-focus-color | blue-600 | blue-200 |
| text-field-focus-shadow-color | rgba(blue,.25) | rgba(blue,.25) |
| primary-suggested-action-background | blue-000 | blue-900 |
| primary-suggested-action-border-color | blue-200 | blue-700 |
| suggested-action-icon-color | (blue-400) | blue-400 |
| diff-text-color | gray-900 | text-color |
| diff-alt-text-color | gray-700 | darken(gray-100, 20%) |
| diff-border-color | gray-200 | gray-800 |
| diff-gutter-color | gray-200 | (see ghd-src) |
| diff-gutter-background-color | background-color | (see ghd-src) |
| diff-line-number-color | gray-700 | (see ghd-src) |
| diff-selected-background-color | blue-400 | (see ghd-src) |
| diff-selected-border-color / gutter-color | blue-600 | (see ghd-src) |
| diff-selected-text-color | background-color | (see ghd-src) |
| diff-add-background-color | darken(green-000, 2%) | darken(green-900, 3%) |
| diff-add-border-color / gutter-color | green-300 | (see ghd-src) |
| diff-add-gutter-background-color | darken(green-100, 3%) | (see ghd-src) |
| diff-add-inner-background-color | #acf2bd | (see ghd-src) |
| diff-delete-background-color | red-000 | darken(red-900, 15%) |
| diff-delete-border-color / gutter-color | red-200 | (see ghd-src) |
| diff-delete-gutter-background-color | red-100 | (see ghd-src) |
| diff-delete-inner-background-color | #fdb8c0 | (see ghd-src) |
| diff-hunk-background-color | blue-000 | (see ghd-src) |
| diff-hunk-border-color | blue-200 | (see ghd-src) |
| diff-hunk-gutter-color | darken(blue-200, 5%) | (see ghd-src) |
| diff-hunk-gutter-background-color | blue-100 | (see ghd-src) |
| diff-hunk-text-color | gray-600 | (see ghd-src) |
| diff-hover-background-color | blue-300 | (see ghd-src) |
| diff-hover-border-color / gutter-color | blue-400 | (see ghd-src) |
| diff-add-hover-background-color | green-300 | (see ghd-src) |
| diff-add-hover-border-color / gutter-color | green-400 | (see ghd-src) |
| diff-delete-hover-background-color | red-200 | (see ghd-src) |
| diff-delete-hover-border-color / gutter-color | red-300 | (see ghd-src) |
| diff-empty-row-background-color | gray-000 | (see ghd-src) |
| diff-empty-hunk-handle | gray-300 | (see ghd-src) |
| error-color | red | (see ghd-src) |
| form-error-background / border / text | red-100 / red-200 / red-800 | (see ghd-src) |
| dialog-banner-success-background / border / text | green-100 / green-300 / green-800 | (see ghd-src) |
| dialog-warning-color | yellow-800 | (see ghd-src) |
| dialog-information-color | blue-400 | (see ghd-src) |
| dialog-error-color | red | (see ghd-src) |
| dialog-progress-background | green | (see ghd-src) |
| status-pending-color | yellow-700 | (see ghd-src) |
| status-error-color | red-500 | (see ghd-src) |
| status-success-color | #1a7f37 | (see ghd-src) |
| overlay-background-color | rgba(0,0,0,.4) | rgba(0,0,0,.5) |

Sizes (theme-independent): toolbar-height 50, tab-bar-height 29, button-height 25, text-field-height 25, border-radius 6, spacing 10, diff-line-number-column-width 50, diff-line-padding-y 2, font-size 12/11/14/28/32/42/9, z-index side-panel 14 < drag 15 < nudge 16 < foldout 17 < popup-overlay 18 < popup 19 < tooltip 20.

Rows marked "(see ghd-src)" appear in `_dark.scss` beyond the first 300 lines; read `ghd-src/_dark.scss` directly when porting `ghd-dark.json`.
