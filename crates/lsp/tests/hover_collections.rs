//! Hover of variables initialized with a list or set literal that contains a
//! map entry (`[1, 'a': 3]`, `{1, 'a': 3}`), code that appears while typing.
//!
//! The expected types are those of `dart language-server` 3.6.2 (and of the
//! analyzer 6.11 `staticType`), checked on 27/10/2026 for every case below:
//! the parser's recovery splits `'a': 3` in a list into two elements, so the
//! list is `List<Object>`, the same for a top-level variable, a field and a
//! local; a map entry next to an expression in braces is an ambiguous
//! literal, typed `dynamic`; inside an `if`/`for` element the entry
//! contributes only its key type.

mod common;

use common::Project;
use serde_json::json;

#[test]
fn list_and_set_literals_with_map_entries_match_the_analyzer() {
    let mut p = Project::new("hover-colecoes");
    let text = "\
var a = [1, 'a': 3];
var b = {1, 'a': 3};
var c = [if (true) 'a': 3];
var d = [1, if (true) 'a': 3];
var e = {1, if (true) 'a': 3};
var f = <int>[1, 'a': 3];
var g = [1, 2, 'a': 3, 'b'];
class C { var f = [1, 'a': 3]; var g = {1, 'a': 3}; }
void main() { var l = [1, 'a': 3]; var s = {1, 'a': 3}; print(l); print(s); }
";
    p.open("lib/a.dart", text);
    let cases = [
        ((0, 4), "List<Object> a"),
        ((1, 4), "dynamic b"),
        ((2, 4), "List<String> c"),
        ((3, 4), "List<Object> d"),
        ((4, 4), "dynamic e"),
        ((5, 4), "List<int> f"),
        ((6, 4), "List<Object> g"),
        ((7, 14), "List<Object> f"),
        ((7, 35), "dynamic g"),
        ((8, 18), "List<Object> l"),
        ((8, 39), "dynamic s"),
    ];
    for ((l, c), expected) in cases {
        let h = p.in_position("textDocument/hover", "lib/a.dart", l, c, json!({}));
        let contents = h["result"]["contents"].as_str().unwrap_or_default();
        assert_eq!(
            contents.lines().next(),
            Some(expected),
            "{l}:{c}: {}",
            h["result"]
        );
    }
}
