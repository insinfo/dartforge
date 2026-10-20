use std::io::Write;
#[path = "/Volumes/ssd_dock/MyRustProjects/dartforge/crates/emit_native/src/pruning/allocation_inventory/integrity_io.rs"]
mod integrity_io;
fn main() {
 let args:Vec<_>=std::env::args().collect();
 let text=std::fs::read(&args[2]).unwrap();
 let value:serde_json::Value=serde_json::from_slice(&text).unwrap();
 let start=std::time::Instant::now();
 match args[1].as_str() {
  "old-compare"=>assert_eq!(text,format!("{}\n",serde_json::to_string(&value).unwrap()).as_bytes()),
  "new-compare"=>assert!(integrity_io::canonical_matches(&text,&value)),
  "old-write"=>{let output=format!("{}\n",serde_json::to_string(&value).unwrap());std::io::sink().write_all(output.as_bytes()).unwrap();assert_eq!(blake3::hash(output.as_bytes()),blake3::hash(&text));},
  "new-write"=>{let digest=integrity_io::write_json(std::io::sink(),&value).unwrap();assert_eq!(digest,blake3::hash(&text));},
  "hash"=>assert_eq!(integrity_io::digest_file(std::path::Path::new(&args[2])).unwrap(),blake3::hash(&text).to_hex().as_str()),
  _=>panic!("unknown benchmark mode")
 }
 println!("{} {}",args[1],start.elapsed().as_secs_f64());
}
