Contributor Guide
=================
  Indented underline makes level two
  ===

Intro paragraph that runs
over two lines before a setext underline
---

* * *

1. First step: fork the repository.
2. Second step, with a nested list:
   - clone your fork
   - create a branch named `feature/my-thing`

     A paragraph inside the list item, after a blank line.

         indented code inside a list item

   - push it
3. Third step with a fenced block inside the item:

   ```js
   const x = 1; // not highlighted: javascript is not loaded
   ```

   ```html
   <em>html in a list</em>
 dedented line still in the fence
   ```

4. Fourth step
continues lazily here.

Back to a paragraph.

- a
 - b
  - c
   - d
    - e
     - f

-
- empty item above
1.no space is not a list
+	tab after plus

> # Header in a quote
> ```
> code fence in a quote
> ```
> > nested *emphasis
> > still emphasised* and **bold**
> back to one level with ``double `tick` code``

Emphasis matrix: *a* _b_ **c** __d__ ***e*** ___f___ *__g__* _**h**_
Intraword: foo*bar*baz foo_bar_baz foo**bar**baz foo__bar__baz
Punctuation: *"quoted"* _(paren)_ **[bracket]** "*left*" (*x*) *.*
Unicode punctuation: *«guillemets»* _—dash—_ **¡hola!** *日本語* _über_
Spaces: a * b * c and a _ b _ c and * and _ alone
Unclosed *emphasis runs
until the end of the paragraph.

Unclosed **strong

resets after a blank line.

Code spans: `a` ``b`` ```c``` `` ` `` `mismatched`` and `x` done.
Escaped backtick \` and backslash \\ and \
end.

HTML block:
<table>
  <tr>
    <td>cell</td>

    <td>after blank inside the table</td>
  </tr>
</table>
Text after the table.

<p
  class="multi-line">attrs</p>

<img src="a.png"
alt="unfinished

text after unfinished tag

< not a tag > and a <3 heart and 1 < 2 > 0.
</closing> stray closing tag and </ spaced>.
<!DOCTYPE html>
<![CDATA[ raw ]]>
<?php echo 1; ?>

Link titles: [a](url "double") [b](url 'single') [c](url (paren)) [d](<url with spaces>)
[e](url "unterminated
next line) and [f]( spaced ) and [g][] and [h] [i] and [j]
Image in link: [![alt text](img.png)](https://example.com) end.
Empty link: []() and [](url) and [text]() done.
Escaped: \[not a link\](nope) and [link\]with\]escapes](url\)with\)parens).
Nested parens: [n](a(b(c)d)e) [m](a\(b) [o](a(b\)c)d) done.
Brackets href: [p][ref [inner] x] [q][a\]b] done.

[ref]: https://example.com/ref
[title1]: https://example.com "Title"
[title2]: https://example.com 'Title'
[title3]: https://example.com (Title)
[title4]: https://example.com
'Next-line single'
[title5]: https://example.com
$dollar title$
[title6]: https://example.com
"unterminated next line title
[with\]escape]: https://example.com/esc

Text with trailing spaces
and a trailing single space
and trailing tab
last.

	Tab-indented code after a blank line
    Space-indented code
  	mixed spaces and tab

Title
Setext after a paragraph line
===

- list before setext
Not a setext
---

```
unterminated fence at the end
with `backticks` and <b>tags</b> inside
