# Tree-sitter captures

Generated from `crates/corvane-highlight/src/treesitter/captures.rs` (`UPDATE_TS_CAPTURES_DOC=1 cargo test -p corvane-highlight captures_doc`).

How the tree-sitter highlighter (Settings › Appearance › Syntax highlighting, flag `105-tree-sitter-highlighting`) colours a capture: its longest dot-prefix below decides (`keyword.control.import` → `keyword`). "line colour" leaves the text unstyled like CodeMirror does; "enclosing" keeps the colour of the capture around it.

| Capture | Colour |
|---|---|
| `comment` | `--syntax-comment-color` |
| `keyword.directive` | `--syntax-comment-color` |
| `keyword.control.directive` | `--syntax-comment-color` |
| `preproc` | `--syntax-comment-color` |
| `tag.doctype` | `--syntax-comment-color` |
| `text.literal` | `--syntax-comment-color` |
| `markup.raw` | `--syntax-comment-color` |
| `punctuation.list_marker` | `--syntax-comment-color` |
| `markup.list.marker` | `--syntax-comment-color` |
| `string` | `--syntax-string-color` |
| `character` | `--syntax-string-color` |
| `escape` | `--syntax-string-color` |
| `constant.character` | `--syntax-string-color` |
| `text.uri` | `--syntax-string-color` |
| `link_uri` | `--syntax-string-color` |
| `markup.link.url` | `--syntax-string-color` |
| `keyword` | `--syntax-keyword-color` |
| `conditional` | `--syntax-keyword-color` |
| `repeat` | `--syntax-keyword-color` |
| `include` | `--syntax-keyword-color` |
| `exception` | `--syntax-keyword-color` |
| `storageclass` | `--syntax-keyword-color` |
| `type.qualifier` | `--syntax-keyword-color` |
| `variable.builtin` | `--syntax-keyword-color` |
| `variable.special` | `--syntax-keyword-color` |
| `variable.language` | `--syntax-keyword-color` |
| `boolean` | `--syntax-atom-color` |
| `constant` | `--syntax-atom-color` |
| `string.special.symbol` | `--syntax-atom-color` |
| `symbol` | `--syntax-atom-color` |
| `type` | `--syntax-type-color` |
| `constructor` | `--syntax-type-color` |
| `enum` | `--syntax-type-color` |
| `variant` | `--syntax-type-color` |
| `variable` | `--syntax-variable-color` |
| `function` | `--syntax-variable-color` |
| `method` | `--syntax-variable-color` |
| `macro` | `--syntax-variable-color` |
| `variable.parameter` | `--syntax-alt-variable-color` |
| `parameter` | `--syntax-alt-variable-color` |
| `variable.member` | `--syntax-alt-variable-color` |
| `variable.other.member` | `--syntax-alt-variable-color` |
| `property` | `--syntax-alt-variable-color` |
| `field` | `--syntax-alt-variable-color` |
| `label` | `--syntax-alt-variable-color` |
| `markup.list` | `--syntax-alt-variable-color` |
| `selector.pseudo` | `--syntax-alt-variable-color` |
| `module` | `--syntax-qualifier-color` |
| `namespace` | `--syntax-qualifier-color` |
| `tag` | `--syntax-tag-color` |
| `selector` | `--syntax-tag-color` |
| `attribute` | `--syntax-attribute-color` |
| `tag.attribute` | `--syntax-attribute-color` |
| `link_text` | `--syntax-link-color` |
| `markup.link` | `--syntax-link-color` |
| `text.reference` | `--syntax-link-color` |
| `title` | `--syntax-header-color` |
| `text.title` | `--syntax-header-color` |
| `markup.heading` | `--syntax-header-color` |
| `markup.quote` | `--syntax-quote-color` |
| `text.quote` | `--syntax-quote-color` |
| `number` | line colour |
| `float` | line colour |
| `constant.numeric` | line colour |
| `operator` | line colour |
| `punctuation` | line colour |
| `embedded` | line colour |
| `emphasis` | line colour |
| `markup.italic` | line colour |
| `markup.strong` | line colour |
| `markup.strikethrough` | line colour |
| `markup.underline` | line colour |
| `text.emphasis` | line colour |
| `text.strong` | line colour |
| `text.strike` | line colour |
| `diff` | line colour |
| `error` | line colour |
| `entity.name.type` | `--syntax-type-color` |
| `entity.name.function` | `--syntax-variable-color` |
| `entity.name` | `--syntax-variable-color` |
| `meta.attribute` | `--syntax-attribute-color` |
| `storage` | `--syntax-keyword-color` |
| `none` | enclosing |
| `spell` | enclosing |
| `nospell` | enclosing |
| `conceal` | enclosing |
| `text` | enclosing |
| `markup` | enclosing |
| `local` | enclosing |
| `injection` | enclosing |
| `hint` | enclosing |
| `predictive` | enclosing |
| `primary` | enclosing |
