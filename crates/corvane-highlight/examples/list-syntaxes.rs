//! JSON list of the syntect grammars Corvane already has (syntect's default
//! set and two-face's collection): tools/tm-grammars/sync.py skips languages
//! they cover.
//!
//! `cargo run -q -p corvane-highlight --features pack-builder --example list-syntaxes`

fn main() {
    let mut out = Vec::new();
    for (set, syntaxes) in [
        (
            "syntect",
            syntect::parsing::SyntaxSet::load_defaults_newlines(),
        ),
        ("two-face", two_face::syntax::extra_newlines()),
    ] {
        for s in syntaxes.syntaxes() {
            out.push(serde_json::json!({
                "set": set,
                "name": s.name,
                "scope": s.scope.to_string(),
                "extensions": s.file_extensions,
            }));
        }
    }
    println!("{}", serde_json::Value::Array(out));
}
