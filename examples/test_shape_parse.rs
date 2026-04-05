//! Quick test: parse scout_bot.ron and print what we found.
//! Run with: cargo run --example test_shape_parse

#[path = "shared/shape.rs"]
mod shape;

fn main() {
    let paths = [
        "data/shapes/scout_bot.ron".to_string(),
        format!("{}/data/shapes/scout_bot.ron", env!("CARGO_MANIFEST_DIR")),
    ];

    for path in &paths {
        println!("Trying: {path}");
        match std::fs::read_to_string(path) {
            Ok(ron_str) => {
                println!("  Read {} bytes", ron_str.len());
                match shape::load_shape(&ron_str) {
                    Ok(f) => {
                        println!("  Parsed OK!");
                        println!("  Templates: {:?}", f.templates.keys().collect::<Vec<_>>());
                        println!("  Root name: {:?}", f.root.name);
                        println!("  Root children: {}", f.root.children.len());
                        for (i, c) in f.root.children.iter().enumerate() {
                            println!("    [{}] name={:?} shape={} mirror={:?} template={:?} children={}",
                                i, c.name, c.shape.is_some(), c.mirror, c.template, c.children.len());
                        }
                    }
                    Err(e) => println!("  PARSE ERROR: {e}"),
                }
                return;
            }
            Err(e) => println!("  Not found: {e}"),
        }
    }
}
