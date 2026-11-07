//! Dead-loop compression preserves lexical initialization, throws and scopes.
use std::process::Command;

#[test]
#[ignore = "requires Node"]
fn lexical_for_initializers_survive_false_condition_compression() {
    let source = r#"
const events = [];
function fail(label) { events.push(label); throw new Error(label); }
try { for (let x = fail("let-init"); false;) { events.push("body"); } } catch { events.push("caught-let"); }
for (const x = events.push("const-init"); false;) {}
try { let no = false; for (let x = fail("folded-test"); no;) {} } catch { events.push("caught-folded"); }
let outer = 42;
try { for (let outer = (() => outer)(); false;) {} } catch (error) { events.push(error instanceof ReferenceError ? "tdz" : "wrong-error"); }
for (let {value} = {get value() { events.push("destructure"); return 7; }}; false;) {}
label: for (let x = events.push("labeled-init"); false;) { continue label; }
function hoist() { for (let x = events.push("hoisted-init"); false;) { var never; } return never; }
if (hoist() !== undefined) throw new Error("hoisting");
console.log(JSON.stringify(events));
"#;
    let compressed =
        dartforge_emit_js_production::minify::minify_with_properties(source, &Default::default())
            .unwrap();
    for program in [source, &compressed] {
        let output = Command::new("node")
            .arg("-e")
            .arg(program)
            .output()
            .unwrap();
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            r#"["let-init","caught-let","const-init","folded-test","caught-folded","tdz","destructure","labeled-init","hoisted-init"]"#
        );
    }
}

#[test]
#[ignore = "requires Node"]
fn compression_keeps_lone_surrogates_distinct_from_replacement_characters() {
    let source = r#"
const a = "\ud800", b = "\udc00", replacement = "\ufffd";
const object = { [a]: 7, [b]: 9, [replacement]: 11 };
const repeated = [a, a, a, b, b, replacement, replacement];
if (object[a] !== 7 || object[b] !== 9 || object[replacement] !== 11) throw Error('keys');
if (new Set(repeated).size !== 3) throw Error('distinct strings');
if (`\ud800`.charCodeAt(0) !== 0xd800) throw Error('template');
if ((a + b).length !== 2 || (a + b).codePointAt(0) !== 0x10000) throw Error('pair');
console.log('surrogates ok');
"#;
    let compressed =
        dartforge_emit_js_production::minify::minify_with_properties(source, &Default::default())
            .unwrap();
    for program in [source, &compressed] {
        let output = Command::new("node")
            .arg("-e")
            .arg(program)
            .output()
            .unwrap();
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "surrogates ok"
        );
    }
}
