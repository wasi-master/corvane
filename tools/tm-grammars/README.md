# TextMate grammars in the default build

The default build highlights languages no other engine covers with TextMate
grammars from Sublime Text, VS Code, Atom and TextMate bundles, as collected
(and license-checked) by GitHub Linguist. They are converted to
sublime-syntax, the format syntect reads, and compiled with syntect's own
default set into `crates/corvane-highlight/assets/syntaxes.packdump`, which
`corvane_highlight::syntaxes` loads instead of syntect's built-in set.

| File | Written by |
|---|---|
| `syntaxes/<scope>.sublime-syntax` (converted grammars, each naming its source and license) | `sync.py` |
| `THIRD_PARTY.md` (language, scope, license, source) | `sync.py` |
| `rejected.txt` (grammars that failed their samples) | the `tm-build` example |
| `crates/corvane-highlight/assets/syntaxes.packdump` | the `tm-build` example |

```bash
python3 tools/tm-grammars/sync.py
cargo run -p corvane-highlight --features pack-builder --example tm-build -- \
    tools/tm-grammars/syntaxes target/tm/samples.json \
    crates/corvane-highlight/assets/syntaxes.packdump tools/tm-grammars/rejected.txt
```

## What is taken

`sync.py` reads Linguist at `LINGUIST_TAG`: the release's
`linguist-grammars.tar.gz` (every vendored grammar compiled to TextMate JSON;
pinned by sha256) and `languages.yml` (each language's scope, extensions,
file names, interpreters). A language is taken when none of its file types
is covered already by a tree-sitter grammar (`tools/ts-queries/languages.toml`),
a ported CodeMirror mode, syntect's defaults or two-face (the `list-syntaxes`
example), and its grammar's license is one Linguist records as permissive
(`other` / unknown are skipped). Scopes a taken grammar includes come along as
hidden syntaxes when the set lacks them.

## Conversion

TextMate → sublime-syntax, rule by rule: `match` rules keep their scope and
captures; `begin`/`end` become a push whose context carries `name` as
`meta_scope`, `contentName` as `meta_content_scope` and the `end` match (first,
or last with `applyEndPatternLast`); `begin`/`while` pops at the first line the
`while` pattern does not match; repository entries become `repo_<name>`
contexts; `$self`/`$base` include `main`, other scopes `scope:<name>`. Dropped:
captures' nested patterns, `$1` substitutions in scope names, injection
grammars; `\G` is removed and `\h` spelled out (fancy-regex has neither).

`tm-build` then parses each language's Linguist samples with its converted
grammar and rejects the grammar on any error or after 20 s, until every
remaining one passes.

## How the sets stack

Default build: the core dump alone. With the `syntax-extended` pack or the
full build, two-face's set comes first and the core dump second
(`syntaxes::sets`), so the TextMate additions two-face lacks still apply.
