//! Fresh-list append removes only a proved redundant cast and retains fallback.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn node(js: &str) -> String {
    let mut child = Command::new("node")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("globalThis.self=globalThis;\n{js}").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

fn function<'a>(js: &'a str, name: &str) -> &'a str {
    let start = js
        .find(&format!("function {name}("))
        .expect("live function");
    let tail = &js[start..];
    &tail[..tail.find("\n};").expect("function end")]
}

#[test]
#[ignore = "requires Dart SDK and Node; child processes isolate experimental flags"]
fn fresh_list_append_differential() {
    for enabled in ["0", "1"] {
        for minify in ["0", "1"] {
            let child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "fresh_list_append_worker",
                    "--nocapture",
                ])
                .env("DARTFORGE_JSPROD_FRESH_LIST_APPEND", enabled)
                .env("DARTFORGE_FRESH_LIST_TEST_WORKER", minify)
                .output()
                .unwrap();
            assert!(
                child.status.success(),
                "enabled={enabled} minify={minify}\n{}\n{}",
                String::from_utf8_lossy(&child.stdout),
                String::from_utf8_lossy(&child.stderr)
            );
        }
    }
}

#[test]
#[ignore = "invoked by the isolated differential parent"]
fn fresh_list_append_worker() {
    let Ok(minify) = std::env::var("DARTFORGE_FRESH_LIST_TEST_WORKER") else {
        return;
    };
    let enabled = std::env::var("DARTFORGE_JSPROD_FRESH_LIST_APPEND").unwrap() == "1";
    let minify = minify == "1";
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/fresh_list_append.dart");
    let oracle = Command::new("dart")
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        oracle.status.success(),
        "{}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let expected = String::from_utf8(oracle.stdout)
        .unwrap()
        .replace("\r\n", "\n");
    let options = dartforge_emit_js_production::Options {
        minify,
        ..Default::default()
    };
    let production = dartforge_emit_js_production::compile_own_sdk(
        &source,
        None,
        None,
        options,
        &Default::default(),
    )
    .expect("production compilation");
    if let Some(directory) = std::env::var_os("DARTFORGE_FRESH_LIST_TEST_OUTPUT") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(format!("enabled-{enabled}-minify-{minify}.js")),
            &production.js,
        )
        .unwrap();
    }
    if !minify {
        for name in ["fresh", "subtype"] {
            assert_eq!(
                function(&production.js, name).contains(".append("),
                enabled,
                "actual fast site {name}"
            );
        }
        for name in [
            "unknownFactory",
            "escaped",
            "aliased",
            "captured",
            "rebound",
            "covariant",
            "fixed",
            "custom",
        ] {
            assert!(
                !function(&production.js, name).contains(".append("),
                "checked refusal {name}"
            );
        }
        if !enabled {
            assert!(!production.js.contains("t$freshListAppend"));
        }
    }
    assert_eq!(
        node(&production.js),
        expected,
        "enabled={enabled} minify={minify}"
    );
    if enabled && !minify {
        assert_eq!(
            node(&format!(
                "{}\n{}",
                production.js,
                mutation_probes(&production.js)
            )),
            expected
        );
    }
    #[cfg(unix)]
    if enabled && !minify {
        modified_sdk(&source, &expected);
    }
    if enabled && !minify {
        let foreign = source.with_file_name("fresh_list_append_foreign.dart");
        let compiled = dartforge_emit_js_production::compile_own_sdk(
            &foreign,
            None,
            None,
            dartforge_emit_js_production::Options {
                minify: false,
                ..Default::default()
            },
            &Default::default(),
        )
        .expect("foreign intrinsic compilation");
        assert!(
            !compiled.js.contains("t$freshListAppend"),
            "foreign SDK intrinsic must invalidate the premise"
        );
        assert_eq!(
            node(&compiled.js),
            "true\n",
            "callee getter exposes receiver before constructor mutates growability"
        );
    }
}

fn mutation_probes(js: &str) -> String {
    let helper = js
        .split("_interceptors.t$freshListAppend")
        .nth(1)
        .expect("live guard")
        .split(" =")
        .next()
        .unwrap();
    format!(
        r#"
const appendGuard = _interceptors.t$freshListAppend{helper};
const itemList = L$fresh_list_append.fresh(1);
const stringList = L$fresh_list_append.strings();
const rtiKey = Object.getOwnPropertySymbols(itemList)[0];
const itemRti = itemList[rtiKey], stringRti = stringList[Object.getOwnPropertySymbols(stringList)[0]];
const addKey = Object.getOwnPropertySymbols(Array.prototype).find(s => s.description === 'dartx.add');
const originalAddDescriptor = Object.getOwnPropertyDescriptor(Array.prototype, addKey);
const originalAdd = originalAddDescriptor.value;
const token = appendGuard.create(_interceptors.JSArray.of, _interceptors.JSArray, itemRti, []);
const before = appendGuard.ready(token, token.list);
if (!before) throw Error('fresh canonical token must be eligible');
const callee = token.list[addKey];
token.list[token.key] = stringRti;
let failed = false;
try {{ appendGuard.append(token, token.list, callee, before, itemList[0]); }} catch (_) {{ failed = true; }}
if (!failed || token.list.length !== 0) throw Error('RTI mutation must use checked original append');
const token2 = appendGuard.create(_interceptors.JSArray.of, _interceptors.JSArray, itemRti, []);
const effects = [];
let leaked;
Object.defineProperty(Array.prototype, addKey, {{configurable:true, get() {{effects.push('lookup'); leaked=this; return originalAdd;}}}});
try {{
  const wasReady = appendGuard.ready(token2, token2.list);
  if (wasReady || effects.length !== 0) throw Error('descriptor guard invoked a getter');
  const method = token2.list[addKey];
  const value = (() => {{effects.push('argument'); leaked[token2.key] = stringRti; return itemList[0];}})();
  failed = false;
  try {{ appendGuard.append(token2, token2.list, method, wasReady, value); }} catch (_) {{ failed = true; }}
  if (!failed || effects.join(',') !== 'lookup,argument') throw Error('lookup/argument/check fallback order');
}} finally {{ Object.defineProperty(Array.prototype, addKey, originalAddDescriptor); }}
const token3 = appendGuard.create(_interceptors.JSArray.of, _interceptors.JSArray, itemRti, []);
const ready3 = appendGuard.ready(token3, token3.list);
token3.list.fixed$length = Array;
failed = false;
try {{ appendGuard.append(token3, token3.list, originalAdd, ready3, itemList[0]); }} catch (_) {{ failed=true; }}
if (!failed || token3.list.length !== 0) throw Error('growability check was skipped');
"#
    )
}

#[cfg(unix)]
fn modified_sdk(source: &Path, expected: &str) {
    let sdk = std::path::PathBuf::from(
        std::env::var_os("DARTFORGE_SDK_LIB").expect("selected SDK library path"),
    );
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/js-fresh-list-sdk");
    std::fs::create_dir_all(&target).unwrap();
    let overlay = target.join(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let changed = Path::new("_internal/js_dev_runtime/private/js_array.dart");
    let library = Path::new("_internal/js_dev_runtime/private/interceptors.dart");
    fn copy(from: &Path, to: &Path, relative: &Path, changed: &Path, library: &Path) {
        if relative == changed {
            let source = std::fs::read_to_string(from).unwrap();
            let rewritten = source.replacen("checkGrowable('add');", "checkGrowable('add');\n    JS('void', r'globalThis.__sdkAppendCalls = (globalThis.__sdkAppendCalls || 0) + 1');", 1);
            assert_ne!(rewritten, source);
            std::fs::write(to, rewritten).unwrap();
        } else if relative == library {
            std::fs::copy(from, to).unwrap();
        } else if changed.starts_with(relative) || library.starts_with(relative) {
            std::fs::create_dir(to).unwrap();
            for e in std::fs::read_dir(from).unwrap() {
                let e = e.unwrap();
                copy(
                    &e.path(),
                    &to.join(e.file_name()),
                    &relative.join(e.file_name()),
                    changed,
                    library,
                );
            }
        } else {
            std::os::unix::fs::symlink(from, to).unwrap();
        }
    }
    copy(&sdk, &overlay, Path::new(""), changed, library);
    let result = dartforge_emit_js_production::compile_own_sdk(
        source,
        Some(&overlay),
        None,
        dartforge_emit_js_production::Options {
            minify: false,
            ..Default::default()
        },
        &Default::default(),
    );
    std::fs::remove_dir_all(&overlay).unwrap();
    let compiled = result.expect("modified selected SDK compilation");
    assert!(
        !compiled.js.contains("t$freshListAppend"),
        "modified SDK must retain checked dispatch"
    );
    assert_eq!(
        node(&format!(
            "{}\nif (!(globalThis.__sdkAppendCalls > 0)) throw Error('modified SDK body did not execute');",
            compiled.js
        )),
        expected
    );
}

#[test]
#[ignore = "requires Dart SDK and Node; isolated experimental closed-window flags"]
fn fresh_list_append_closed_differential() {
    for enabled in ["0", "1"] {
        for minify in ["0", "1"] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "fresh_list_append_closed_worker",
                    "--nocapture",
                ])
                .env("DARTFORGE_JSPROD_FRESH_LIST_APPEND", "0")
                .env("DARTFORGE_JSPROD_FRESH_LIST_APPEND_CLOSED", enabled)
                .env("DARTFORGE_FRESH_CLOSED_TEST_WORKER", minify)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "enabled={enabled} minify={minify}\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
#[ignore = "invoked by isolated closed-window differential parent"]
fn fresh_list_append_closed_worker() {
    let Ok(minify) = std::env::var("DARTFORGE_FRESH_CLOSED_TEST_WORKER") else {
        return;
    };
    let minify = minify == "1";
    let enabled = std::env::var("DARTFORGE_JSPROD_FRESH_LIST_APPEND_CLOSED").unwrap() == "1";
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/fresh_list_append_closed.dart");
    let oracle = Command::new("dart")
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        oracle.status.success(),
        "{}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let expected = String::from_utf8(oracle.stdout)
        .unwrap()
        .replace("\r\n", "\n");
    let compiled = dartforge_emit_js_production::compile_own_sdk(
        &source,
        None,
        None,
        dartforge_emit_js_production::Options {
            minify,
            ..Default::default()
        },
        &Default::default(),
    )
    .expect("closed-window compilation");
    if !minify {
        assert_eq!(
            function(&compiled.js, "pure").contains(".appendClosed("),
            enabled,
            "actual pure-field constructor window"
        );
        for name in [
            "constructorBody",
            "initializerCall",
            "implicitField",
            "inheritedBody",
            "globalGetter",
            "customOperator",
            "callback",
            "globalEscape",
            "reservedMetadata",
            "shadowedCounter",
            "shadowedParameter",
            "alternateWrite",
            "unknownCounter",
            "dynamicOverflow",
        ] {
            let body = function(&compiled.js, name);
            assert!(!body.contains(".appendClosed("), "refusal {name}");
            assert!(
                !body.contains(".create("),
                "refused window must retain original allocation {name}"
            );
        }
    }
    assert_eq!(
        node(&compiled.js),
        expected,
        "closed={enabled} minify={minify}"
    );
    if enabled && !minify {
        let foreign = source.with_file_name("fresh_list_append_foreign.dart");
        let compiled = dartforge_emit_js_production::compile_own_sdk(
            &foreign,
            None,
            None,
            dartforge_emit_js_production::Options {
                minify: false,
                ..Default::default()
            },
            &Default::default(),
        )
        .expect("foreign closed-window compilation");
        assert!(
            !compiled.js.contains("t$freshListAppend"),
            "interop invalidates closed window"
        );
        assert_eq!(node(&compiled.js), "true\n");
    }
}
