# Bundled templates

Used by "Create a New Repository" (Git Ignore and License selects), exactly
like GitHub Desktop's `app/static/gitignore` and `app/static/choosealicense.com`.

- `gitignore/*.gitignore` — the root templates of
  <https://github.com/github/gitignore> (CC0-1.0).
- `licenses/*.txt` — `_licenses/*.txt` of
  <https://github.com/github/choosealicense.com> (CC-BY-3.0 for the site
  content; the license texts themselves are public). Front matter (`title`,
  `featured`, `hidden`) drives the select; `[year]`/`[fullname]`/`[project]`
  style tokens are filled in when the file is written.

Refresh with `packaging/fetch-templates.sh`.
