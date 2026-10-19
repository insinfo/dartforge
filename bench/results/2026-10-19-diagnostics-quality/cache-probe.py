"""Measure allocations in warm dependency-cache lookups, not YAML parsing."""
from pathlib import Path
import subprocess

out = Path(__file__).resolve().parent
source = 'crates/analysis/src/lints/pubspec.rs'
before = subprocess.check_output(['git', 'show', 'd63e7ae3:' + source], text=True)
after = Path(source).read_text()

def cache_function(text):
    return text[text.index('pub fn available_dependencies('):text.index('fn available_dependencies_uncached(')]

code = r'''
use std::{alloc::{GlobalAlloc, Layout, System}, sync::atomic::{AtomicUsize, Ordering}, hint::black_box};
struct Allocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator] static ALLOCATOR: Allocator = Allocator;
'''
for name, text in [('before', before), ('after', after)]:
    code += 'mod ' + name + ' {\n'
    if name == 'after':
        code += 'type DependencyResults = std::collections::HashMap<String, Option<Vec<String>>>;\n'
    code += r'''
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PARSES: AtomicUsize = AtomicUsize::new(0);
    // Only the real cache function is under measurement. This stub primes it;
    // the counter proves that parsing is never reached during measured hits.
    fn available_dependencies_uncached(source: &str, with_dev: bool) -> Option<Vec<String>> {
        PARSES.fetch_add(1, Ordering::Relaxed);
        if source.ends_with("INVALID") { return None; }
        let mut result = vec!["demo".to_string(), "runtime_dep".to_string()];
        if with_dev { result.push("test_dep".to_string()); }
        Some(result)
    }
    pub fn parses() -> usize { PARSES.load(Ordering::Relaxed) }
'''+cache_function(text)+'}\n'
code += r'''
fn main() {
    let valid = format!("#{}\nname: demo\ndependencies:\n  runtime_dep: any\ndev_dependencies:\n  test_dep: any\n", "x".repeat(16_384));
    let invalid = format!("#{}\nINVALID", "x".repeat(16_384));
    let count = 10_000;
    println!("version,result,with_dev,source_bytes,lookups,allocations,allocated_bytes");
    for (version, lookup, parses) in [
        ("before", before::available_dependencies as fn(&str, bool)->Option<Vec<String>>, before::parses as fn()->usize),
        ("after", after::available_dependencies as fn(&str, bool)->Option<Vec<String>>, after::parses as fn()->usize),
    ] {
        let mut expected_parses = 0;
        for (result, source) in [("some", &valid), ("none", &invalid)] {
            for with_dev in [false, true] {
                let expected = lookup(source, with_dev);
                expected_parses += 1;
                assert_eq!(lookup(source, with_dev), expected);
                let allocations = ALLOCATIONS.load(Ordering::Relaxed);
                let bytes = BYTES.load(Ordering::Relaxed);
                for _ in 0..count { black_box(lookup(black_box(source), black_box(with_dev))); }
                let used_allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations;
                let used_bytes = BYTES.load(Ordering::Relaxed) - bytes;
                assert_eq!(parses(), expected_parses);
                println!("{version},{result},{with_dev},{},{count},{used_allocations},{used_bytes}",source.len());
            }
        }
    }
}
'''
rs = out / 'cache-probe.rs'
rs.write_text(code)
exe = Path('target/diagnostics-cache-probe').resolve()
subprocess.run(['rustc', '--edition=2024', '-O', str(rs), '-o', str(exe)], check=True)
with (out / 'cache-allocations.csv').open('w') as log:
    subprocess.run([str(exe)], stdout=log, check=True)
