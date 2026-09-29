Edge cases for the markdown state machine
=

- [ ] open task with *emphasis*
- [x] closed task with ~~strike~~
- [X] closed upper-case task
- [ ]not a task (no space after)
- [y] not a task
1. [ ] ordered task
* [ ]
  - [ ] nested task

Strike: ~~one~~ ~~~three~~~ ~ ~~ spaced ~~ and a~~b~~c and ~~ unclosed
tildes ~~at end~~
Emoji: :smile: :+1: :-1: :heavy-minus: :Upper: :123: :a: ::double:: :not closed

> one
>> two
>>> three is past the max depth
>>>> four
> > > spaced three

- item
  ```
  null-mode fence in a list
dedented text leaves the list and the fence
  ```

- item two
  ```html
  <b>html fence in a list</b>
text at column zero stays in the fence
  ```
after

````markdown
nested markdown with <div class="x">
html inside the nested fence
</div>
and ```html
<i>doubly nested</i>
```
````

Link title after footnote-style definitions:
[a]: http://a.example
"dq title"
[b]: http://b.example
(paren (nested) title)
[c]: http://c.example
$dollar$ tail
[d]: http://d.example
 'indented single' trailing
[e]: http://e.example
plain text is not a title
[f]: http://f.example "same line" trailing words
[g]: http://g.example   
next line after trailing spaces

Emphasis next to emoji: 😀*a*😀 😀_b_😀 🎉**c**🎉 *😀* _🎉_ **🚀**
Emphasis next to CJK punctuation: 「*強調*」 （_下線_） 、**太字**。
Emphasis with nbsp: * nbsp * a _b_ c 　*ideographic space*　	*tab*	_tab_
Deep: ***strong em*** ___strong em___ **_mixed_** __*mixed*__ *__mixed__* _**mixed**_
Over-long runs: ****four**** _____five_____ and *****
Line separator: [x](a b) * em* end
Close without open: foo* bar_ baz** qux__

<div markdown="1">
**markdown inside** the div > with a gt
</div>

<span markdown='1'>single-quoted markdown attr</span> after
<p markdown=1>unquoted</p>

<table><tr><td>one-line table</td></tr></table>
after table

<div>
unclosed div

text after a blank line inside an unclosed div
</div>

<a href="x"
title="multi-line tag">

blank above while the tag is closed but context open
</a>

Setext edge: the next line has four spaces
    ===
Setext edge: tab before
	===
Setext edge: trailing spaces
===   
Setext edge: single dash is not setext
-
Setext edge: link def line
[x]: http://x.example
===

# Header with closing hashes ###
## Header with trailing # hash
### Header with `code` and *em* and [link](u) ###
#
# 
#5 not header

  > indented quote
   > three spaces
    > four spaces is code

 1. one space ordered
    - nested under ordered
        - deeper
            - even deeper
                - and deeper
- back to top
    code-ish continuation

* list
***
- list then hr
- - -

Trailing:  
 two spaces above at the end  
   
spaces-only line above
