# Corvane

[![CI](https://github.com/example/corvane/actions/workflows/ci.yml/badge.svg)](https://github.com/example/corvane/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg "MIT license")](LICENSE)

A **native** Rust clone of _GitHub Desktop_, built on `gpui` and ``gitoxide``.
It aims for *pixel parity* with the original, with __strong__ opinions and ***both*** at once.
Some snake_case_words and 2*3*4 math should not turn into emphasis, nor should * lone stars *.

Setext Title
============

Another Section
---------------

## Table of contents

1. [Installation](#installation)
2. [Usage](#usage)
   1. [Cloning](#cloning)
   2. [Committing][commit-docs]
3. [FAQ][]
10) Parenthesised item

- Fast: reads with gitoxide 🚀
- Familiar: every dialog matches GHD 3.6.6
  - nested item with `inline code`
    - third level, with a [link](https://example.com "Title")
      continuation line that is lazy
* star bullet
+ plus bullet

- [ ] task not done
- [x] task done
- [X] task done loudly

### Installation ###

```sh
brew install corvane
cargo install --path crates/corvane # comment
```

~~~
plain tilde fence
  with indentation kept
~~~

````markdown
```nested
still inside the four-backtick fence
```
````

```html
<div class="banner" id=main>
  <p>Hello &amp; welcome</p>
</div>
```

```xml
<?xml version="1.0"?>
<config enabled="true"><!-- note --></config>
```

    indented code block
    	with a tab

> **Note**
> Corvane is still in development.
>> Nested quote with `code` and a [link][commit-docs].
lazy continuation of the quote

> Quote after a blank line
> - list inside a quote
> 1. ordered inside

---
***
_ _ _
- - -

#### Usage

Run `corvane .` in a repository, or open it from the menu bar.
Line with two trailing spaces for a hard break  
Next line after the break.\
Backslash break above; escapes: \*not emphasis\*, \_nor this\_, \# nor \[this\].

Inline HTML: <kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd>, a line<br/>break and <span style="color: red">red</span>.
Entities: &copy; 2026 &mdash; &#169; &#xA9; and a lone & ampersand.

<details>
<summary>Click to expand</summary>

Hidden *markdown* content.

</details>

<!-- a comment
spanning lines -->

<div markdown="1">
Markdown **inside** a div.
</div>

Autolinks: <https://github.com/example/corvane>, <ftp://files.example.com/a\>b> and <someone@example.com>.
Images: ![logo](docs/logo.png "Corvane logo") and ![ref image][logo] and ![broken] text.
Reference links: [GitHub Desktop][ghd], [collapsed][], [shortcut] and [nested [brackets]](url).
Link with parens: [wiki](https://en.wikipedia.org/wiki/Rust_(programming_language)) done.
Link with spaces: [space] [ghd] and [paren link] (not a link).

| Feature        | Status | Notes                  |
| -------------- | :----: | ---------------------: |
| Clone          | ✅     | `git clone` under hood |
| **Stash**      | 🚧     | _partial_              |

Emoji shortcodes :tada: :+1: :heavy_check_mark: and emoji characters 🎉👍🏽 in a line.
Strikethrough ~~old text~~ and ~single~ tilde.

Footnote reference[^1] and another[^note].

[^1]: The first footnote.
[^note]: A named footnote with `code`.

[commit-docs]: https://docs.example.com/commit "Committing"
[ghd]: <https://desktop.github.com> 'GitHub Desktop'
[logo]: docs/logo.png
  "Title on the next line"
[FAQ]: https://example.com/faq
(Paren title)

####### seven hashes is not a header
#hashtag is not a header either
  ## indented header ##
    # too indented, code

Paragraph right before a list:
- item one

	tab-indented line

Final paragraph with `unterminated code
and a trailing line.
