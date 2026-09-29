Release notes
=
# H1
## H2 ##
### H3 with `code` and *em*
#### H4 #####
##### H5
###### H6 closing ######
   # indented three spaces is still a header
    # four spaces is code
#	tab after hash

Paragraph
---
A list follows
- item one
- item two
---

- item
Setext after list line
===

* level one
	* tab-indented level two
		* two tabs level three
* back

1. Ordered
   ```html
   <em>in a list</em>
   ```
2. Next item
   ```
   plain code in list
   ```
3. Fence that exits the list
   ```
   still code
text at column zero exits
after exit

- list item with fence
  ~~~~
  inner tilde code
  ~~~
  still inside because three is short
  ~~~~~
- after tilde fence

> quote with list:
> - quoted item
> - another *em* item
> ```
> code in quote
> ```
> `a` ``b`` in quote

>not spaced quote
>>double
 > indented quote marker
text > not a quote

[title-next]: https://example.com/a
"Title in double quotes"
[title-single]: https://example.com/b
'Title in single quotes'
[title-paren]: https://example.com/c
(Title in parens)
[title-none]: https://example.com/d
Plain text then "quoted" later
[title-dollar]: https://example.com/e
word $cash$ end
[title-space]: https://example.com/f
two words here
[title-escaped]: https://example.com/g
"Escaped \" quote"

[href-bracket]: /relative/path
[a]: <x> "t"
[esc\]aped]: http://example.com/escaped

A [reference][href-bracket] and a [shortcut] and an ![image] [ref].
Link [text](http://example.com/multi
line/href) done and [text2][multi
line ref] done.
A [broken]x link and [another](  spaced ) href.
A [link with **bold** inside](http://x.y) and [link with `code`](http://x.y).
Image ![alt *with em*](a.png) and ![nested [alt]](b.png) and ![ref] [img].
Bang without image! and ![not an image.
[x] [y] brackets with space.

<https://example.com/a\>b> escaped greater-than in autolink.
<mailto:user@example.com> and <user.name+tag@sub.example.co.uk>.
<http://unterminated

<table>
  <tr>
    <td>cell</td>

    <td>after blank inside table</td>
  </tr>
</table>
After the table.
   
	
*em after whitespace-only lines*

<p>
Paragraph html</p> trailing *text*

<img src="a.png"
     alt="multi-line tag">
Following line.

<section markdown="1">
*emphasis inside md html*
</section>

<!DOCTYPE html>
<?php echo "processing"; ?>
<![CDATA[ raw < data ]]>
<not-closed attr=unquoted
still in tag>
<Custom-Element data-x='1'>content</Custom-Element>
<br/>after self close
< not a tag
a <b>bold</b> inline and <i>italic
across</i> lines.

```markdown
Nested markdown fence
<div>html inside nested markdown</div>
~~~xml
<deep/>
~~~
```

```
unterminated fence runs to the end
# not a header
*not em*
