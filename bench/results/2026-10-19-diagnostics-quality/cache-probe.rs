
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
mod before {

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
pub fn available_dependencies(source: &str, with_dev: bool) -> Option<Vec<String>> {
    // Every unit of a package asks for the same `pubspec.yaml`: the answer by
    // text, per thread.
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<(String, bool), Option<Vec<String>>>> = Default::default();
    }
    if let Some(hit) = CACHE.with(|c| c.borrow().get(&(source.to_string(), with_dev)).cloned()) {
        return hit;
    }
    let r = available_dependencies_uncached(source, with_dev);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 64 {
            c.clear();
        }
        c.insert((source.to_string(), with_dev), r.clone());
    });
    r
}

}
mod after {
type DependencyResults = std::collections::HashMap<String, Option<Vec<String>>>;

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
pub fn available_dependencies(source: &str, with_dev: bool) -> Option<Vec<String>> {
    // Every unit of a package asks for the same `pubspec.yaml`: the answer by
    // text, per thread.
    thread_local! {
        static CACHE: std::cell::RefCell<[DependencyResults; 2]> = Default::default();
    }
    let mode = usize::from(with_dev);
    if let Some(hit) = CACHE.with(|c| c.borrow()[mode].get(source).cloned()) {
        return hit;
    }
    let r = available_dependencies_uncached(source, with_dev);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.iter().map(DependencyResults::len).sum::<usize>() > 64 {
            for mode in c.iter_mut() {
                mode.clear();
            }
        }
        c[mode].insert(source.to_string(), r.clone());
    });
    r
}

}

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
