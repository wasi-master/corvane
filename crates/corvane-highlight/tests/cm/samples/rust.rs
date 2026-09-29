//! Sample for the CodeMirror Rust mode golden test.
#![allow(dead_code)]

use std::collections::HashMap;

/* block comment
   spanning lines */
#[derive(Debug, Clone)]
pub struct Point<T> {
    x: T,
    y: T,
}

enum Shape { Circle(f64), Square { side: u32 } }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut map: HashMap<&str, i32> = HashMap::new();
    let raw = r"C:\path";
    let hashed = r#"a "quoted" \ string"#;
    let byte = b'a';
    let ch = '\n';
    let s = "escaped \"quote\" and {placeholder}";
    let n = 0x1F_u8 + 1_000 + 3.14e-2f64 as u8;
    if let Some(v) = map.get("key") { println!("{v}"); } else { unreachable!() }
    for (i, x) in (0..10).enumerate().filter(|&(i, _)| i % 2 == 0) {
        map.insert("k", x as i32 * i as i32);
    }
    let multi = "line one
line two";
    let emoji = "😀 wide"; let after = 1; // 😀 then text
    Ok(())
}
