==============================
 Corvane — guide de démarrage
==============================

.. contents:: Table of Contents
   :depth: 2

Introduction
------------

This is **bold text**, *emphasis*, and ``inline literal`` in a paragraph.
Numbers like 42, 3.14 and 1,000 get styled; so do +5 and -7 offsets.
Visit https://example.org/docs/index.html?x=1#top or HTTP://EXAMPLE.COM.
**not closed and *also not closed and ``no end
Mixed **bold**text and *em*x and ``lit``y without boundary.

Roles and references
~~~~~~~~~~~~~~~~~~~~

Use :emphasis:`role text` and `suffix text`:strong: and plain :role: here.
Inline math :math:`\alpha + \beta^{2}` and :latex:`\frac{a}{b}` roles.
See the |substitution| and |sub ref|_ and |anon|__ usages.
Footnotes [1]_, [#]_, [#note]_, [*]_ and citations [CIT2002]_.
Links like Python_ and `Corvane docs`_ and anonymous__ references.
A trailing_word_ref_ followed by x and ref_x word.
Weird `unterminated backtick and :bad: role:done.

.. _Python: https://www.python.org/
.. __: https://anonymous.example.org/
.. _`Corvane docs`: https://example.org/corvane

.. |substitution| replace:: some replacement text
.. |logo| image:: images/logo.png
   :alt: Logo ✓

.. [1] A numbered footnote.
.. [#note] An auto-numbered, labelled footnote.
.. [CIT2002] A citation.

.. note::
   This is a note directive body.

.. code-block:: python

    def greet(name):
        return f"Hello, {name}!"

.. python::
   import os
   print(os.getcwd())

.. math::

   E = mc^{2} \quad \text{energy}

.. latex::
   \begin{equation} x^2 \end{equation}

.. This is a comment
   that continues on an indented line.

Not a comment anymore.

Example session::

    $ corvane --version
    corvane 0.4.2

Back to normal text.

    >>> print("doctest")
    doctest
    >>> 1 + 2
    3

    In [1]: x = 5

    >>> s = """multi-line
    ... docstring"""
    >>> x = [i ** 2 for i in range(10)]  # comment

Math role spanning lines :math:`\sum_{i=1}^{n}
x_i` continues **here**.
Role at end :math:
Suffix `text`:role:

.. |eq| math:: x + y
.. |py| python:: print(1)
.. python:: print("same line")
.. image:: pic.png
.. [#]
.. [2] second
..  |bad sub|  replace:: spaced
.. _trailing-link:
.. __ not a link

----

Unicode: ünïcödé ✓ 😀 and tabs	here.
Ünïcödé_ link and ``lit`` and `x`_ and `y`__ and |z| end.
..
Final paragraph with **strong** end
