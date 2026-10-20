#[allow(dead_code)]
#[path = "/tmp/forge-stream-verify/integrity_io.rs"]
mod integrity_io;
#[allow(dead_code)]
#[path = "/Volumes/ssd_dock/MyRustProjects/dartforge/crates/emit_native/src/pruning/allocation_inventory/publication/shape.rs"]
mod shape;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let text = std::fs::read(&args[2]).unwrap();
    let start = std::time::Instant::now();
    match args[1].as_str() {
        "old" => {
            let value: serde_json::Value = serde_json::from_slice(&text).unwrap();
            assert!(integrity_io::canonical_matches(&text, &value));
            assert_eq!(value.as_object().unwrap().len(), 7);
            assert_eq!(value["inventory"].as_object().unwrap().len(), 4);
            for key in ["functions", "globals", "modules"] {
                assert!(value["inventory"][key].is_array());
            }
            assert_eq!(value["schema"], 5);
            assert_eq!(value["production"], true);
            assert!(value["runtime"].is_object());
            assert!(value["artifacts"].is_array());
            assert!(!value["compiler"].as_str().unwrap().is_empty());
            assert!(!value["cache_key"].as_str().unwrap().is_empty());
        }
        "new" => {
            assert!(integrity_io::canonical_document_matches(&text));
            let value: shape::Envelope = serde_json::from_slice(&text).unwrap();
            let shape::InventoryShape = value.inventory;
            assert_eq!(value.schema, 5);
            assert!(value.production);
            assert!(value.runtime.is_object());
            assert!(value.artifacts.is_array());
            assert!(!value.compiler.is_empty());
            assert!(!value.cache_key.is_empty());
        }
        _ => panic!("unknown benchmark mode"),
    }
    println!("{} {}", args[1], start.elapsed().as_secs_f64());
}
