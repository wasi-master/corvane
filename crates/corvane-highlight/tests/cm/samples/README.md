# Corvane

A **native** Rust clone of _GitHub Desktop_, built on GPUI.  
Second line after a hard break (two trailing spaces above).

[![CI](https://github.com/example/corvane/actions/workflows/ci.yml/badge.svg)](https://github.com/example/corvane/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg "MIT license")](LICENSE)

## Table of contents

1. [Install](#install)
2. [Usage](#usage)
   1. [Cloning](#cloning)
   2. [Committing](#committing)
3. [FAQ](#faq)
10) Parenthesised ordered item

Setext header, level one
========================

Setext header, level two
------------------------

  Indented setext title
  ===

### Features ###

* Fast diffs with *syntax highlighting* and **bold** claims
* Works offline — no telemetry, ever
  * nested item with `inline code`
    * third level with __strong__ and _em_
      * fourth level cycles the colours
  * back to level two
* back to level one
- dash list
+ plus list

- [ ] task list syntax is plain text here
- [x] done item

Paragraph with *emphasis*, **strong emphasis**, ***both at once***, and _underscore em_,
__underscore strong__, ___underscore both___, snake_case_words stay plain,
2 * 3 * 4 is arithmetic, a * b surrounded by spaces, and **unterminated bold
that carries to the next line** within a paragraph.
Punctuation flanking: *"quoted"* and **(parenthesised)**, x*y*z, x_y_z, «*fr*».
Emphasis *with trailing space * and * leading space* and `code *not em*`.

This line has ~~strikethrough~~ which is off by default, and :emoji: too.

Inline `code`, double ``code with ` backtick``, triple ```x```, and an unterminated `span
that continues here` ends. Mismatched ``two then one` stays open`` closed.

> A blockquote with *emphasis* and a [link](https://example.com).
> > Nested quote level two
> > > Level three with `code`
continued lazily without marker

> Quote again
>
> After an empty quote line

Escapes: \*not em\*, \_not em\_, \`not code\`, \\ backslash, \[not a link\], \# not a header.

Links: [inline](https://example.com/path?q=1&r=2 "Title"), [ref link][ref], [collapsed][],
[nested [brackets] inside](http://example.com/a(b)c), [spaced] [ref], and [dangling] text.
Autolinks <https://github.com/desktop/desktop> and <ftp://files.example.com/pub>,
email <someone@example.com>, and not-a-link <notalink>.
Images: ![alt text](images/logo.png "Logo"), ![ref image][logo], ![](empty.png).
Link with a title across [lines](http://example.com/very/long/url
continued) here.

[ref]: https://example.com/reference "Reference title"
[logo]: ./images/logo.svg 'Single quoted'
[paren]: <https://example.com/angle> (Parenthesised title)
[next-line]: https://example.com/next-line
  "Title on the next line"
[^1]: A footnote definition with *em*.
 [indented]: https://example.com/indented

Footnote reference[^1] in text.

***
---
___
* * *
- - -
_ _ _ _

Some text
***

    indented code block line one
    indented code block line two
	tab-indented code

Text right after code.

```rust
fn main() {
    println!("Hello, *world*!");
}
```

~~~python
def greet(name):
    return f"hi {name}"
~~~

````
Four-backtick fence with ``` inside
````

```html
<div class="note">
  <p>Hello <b>world</b></p>
</div>
```

```xml
<?xml version="1.0"?>
<root attr="1"><child/></root>
```

``` markdown
# Nested heading
* nested *list*
```

```XHTML
<br/>
```

```text/html
<span>mime fence</span>
```

```image/svg+xml
<svg xmlns="http://www.w3.org/2000/svg"></svg>
```

```js
const x = `template ${literal}`;
```

```
plain fence without language
```

<div align="center">
  <img src="logo.png" alt="Logo" width="200">
</div>

<details>
<summary>Click to expand</summary>

Hidden *markdown* content.
  
</details>

<!-- An HTML comment
spanning lines -->

<br>
Inline <kbd>Ctrl</kbd>+<kbd>C</kbd> and <span style="color: red">red</span> text.
<div markdown="1">*md inside*</div>
Closing tag alone </span> here.

Unicode: café, naïve, 日本語のテキスト, emoji 🎉 *強調* and **太字**, Ω≈ç√.

Trailing spaces at end of line   
Line with trailing tab	
####### seven hashes is not a header
#Not a header without space
#

| Column A | Column B |
|----------|:--------:|
| `a`      | **b**    |

1. First
2. Second

   Paragraph inside item.

       code inside list item
3. Third
Final paragraph without newline at end with a [link](http://example.com