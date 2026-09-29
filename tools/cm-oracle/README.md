# CodeMirror oracle

GitHub Desktop colours diffs with CodeMirror 5 modes run by its highlighter
worker (`app/src/highlighter/index.ts`). Corvane ports those modes to Rust
(`crates/corvane-highlight/src/cm`) and proves each port against GHD's own
JavaScript: this directory runs the original modes in Node on sample files
and records the tokens, and `tests/cm_golden.rs` requires the Rust port to
produce exactly the same tokens (boundaries and styles, `m-<mode>` class
included).

```bash
python3 tools/cm-oracle/extract.py           # CodeMirror + modes from the installed GHD → target/cm-oracle
python3 tools/cm-oracle/gen.py [sample…]      # samples/* → expected/*.tokens (needs node)
cargo test -p corvane-highlight --test cm_golden
```

Nothing from GHD is committed; `extract.py` rebuilds the JavaScript from the
source maps inside `/Applications/GitHub Desktop.app`. The mode sources you
port from end up in `target/cm-oracle/node_modules/codemirror/mode/<name>/<name>.js`
(plus `codemirror-mode-{elixir,luau,zig}`).

## Porting a mode

1. Read the JS mode and every mode it `require`s / `CodeMirror.getMode`s.
2. Write `crates/corvane-highlight/src/cm/modes/<name>.rs`: a type that
   implements `cm::Mode` (or a `cm::simple::SimpleMode` table for
   `defineSimpleMode` modes). Port line by line; keep the JS function names
   in comments so the two can be compared. Register it: `pub mod <name>;` and
   its MIME types in `modes::mode_for_mime` (with the MIME's option object,
   e.g. `text/typescript` → javascript with `typescript: true`).
3. Add `crates/corvane-highlight/tests/cm/samples/<name>.<ext>` (and more
   files for other MIME variants): realistic code that exercises every
   branch of the mode — strings of every kind, comments (line + multi-line
   state carried across lines), numbers, keywords, definitions, operators,
   nested constructs, unterminated constructs at end of line, blank lines,
   tabs, non-ASCII text. Several hundred tokens per sample.
4. `python3 tools/cm-oracle/gen.py <sample>` then
   `cargo test -p corvane-highlight --test cm_golden` until it passes.

### JS → Rust translation notes

- `StringStream` positions are UTF-16 units, like JS string indices
  (`stream.pos`, `stream.start`, `back_up(n)`); use `cm::js_len(s)` for a
  JS `.length`.
- `stream.match(/re/)` → `stream.match_re(re!(r"..."), consume)` (unanchored
  search whose match must start at `pos`, exactly like JS; returns the match
  text and groups). `stream.match("str", consume, ci)` → `match_str`.
  `eat("x")` → `eat('x')`, `eat(/re/)` → `eat_re(re!(..))` or `eat_if(|c| ..)`,
  `eatWhile` → `eat_while` / `eat_while_if` / `eat_while_re`.
- `re!` compiles once per call site with `fancy_regex` (lookahead,
  lookbehind and backreferences work like JS). JS flag `i` → `(?i)`.
- JS `\w`, `\d`, `\b` are ASCII; Rust's are Unicode. Write `[A-Za-z0-9_]`,
  `[0-9]`, and word boundaries as lookarounds
  (`(?<![A-Za-z0-9_])(?=[A-Za-z0-9_])` / `(?<=[A-Za-z0-9_])(?![A-Za-z0-9_])`)
  when non-ASCII text would behave differently: fancy-regex rejects
  `(?-u:…)`. JS `\s` is Unicode in both. In fancy-regex `\<` / `\>` are word
  boundaries, not literal `<` / `>`: drop the backslash. Case-insensitive
  `(?i)` also folds `ſ` / `K` (Kelvin) onto `s` / `k`, which JS does not.
  JS `.` excludes `\n`, `\r`, U+2028 and U+2029; Rust's only `\n`. `re!`
  and the simple-mode tables rewrite `.` (`cm::js_pattern`); code standing
  in for a JS regex (`match(/.*/)`) uses `skip_js_dots` /
  `is_js_line_terminator`, not `skip_to_end`. `fuzz.py` checks every port
  against GHD with those characters spliced into the samples.
- A JS state object becomes a `#[derive(Clone)] struct`; `state.tokenize`
  function pointers become an enum (or `fn` pointer) field. Nested modes keep
  the inner state as `Box<dyn ModeState>`.
- The style string is returned verbatim (`"string property"`, `"comment
  variable-2"`); the cascade in `cm::resolve` picks GHD's colour later.
- `Mode::name` must equal the JS `mode.name`, and nesting modes override
  `inner_mode_name` like `innerMode` so tokens get the right `m-` class.
- `blankLine` → `Mode::blank_line`. `indent`, `electricChars`, `fold` and
  other editor-only hooks are not needed.
