//! Typed-loop guards inspect public accessor identities without invoking callbacks.
use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn typed_guards_preserve_public_accessor_effects() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/typed_public_accessors/main.dart");
    let directory =
        std::env::temp_dir().join(format!("dartforge-typed-accessors-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let vm = Command::new("dart")
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        vm.status.success(),
        "{}",
        String::from_utf8_lossy(&vm.stderr)
    );
    let expected = String::from_utf8(vm.stdout).unwrap().replace("\r\n", "\n");
    let mut checked_length = None;
    for profile in ["O2", "off", "raw", "min"] {
        let artifact = directory.join(format!("{profile}.js"));
        let mut compile = if profile == "O2" {
            let mut c = Command::new("dart");
            c.args(["compile", "js", "-O2"]);
            c
        } else {
            let mut c = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"));
            if profile == "raw" || profile == "off" {
                c.arg("--no-minify");
            }
            if profile == "off" {
                c.env("DARTFORGE_JSPROD_TYPED_LOOPS", "0");
            }
            c
        };
        let result = compile
            .arg(&source)
            .arg("-o")
            .arg(&artifact)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{profile}: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        for mode in ["buffer", "byteOffset", "byteLength", "baseBuffer", "resizable", "descriptor", "isView", "consoleByteLength"] {
            let host = directory.join("host.js");
            std::fs::write(&host, HOST).unwrap();
            let result = Command::new("node")
                .arg(&host).arg(&artifact).arg(mode).output().unwrap();
            assert!(result.status.success() && result.stderr.is_empty(),
                "{profile}/{mode}: {}", String::from_utf8_lossy(&result.stderr));
            assert_eq!(String::from_utf8(result.stdout).unwrap().replace("\r\n", "\n"),
                expected, "{profile}/{mode}");
        }
        // dart2js hoists the native length read; compare this callback trace
        // with our checked fallback, rather than treating backend purity as equal.
        let result = Command::new("node").arg(directory.join("host.js"))
            .arg(&artifact).arg("length").output().unwrap();
        assert!(result.status.success() && result.stderr.is_empty());
        let output = String::from_utf8(result.stdout).unwrap().replace("\r\n", "\n");
        assert!(output.starts_with("6\n"));
        if profile == "off" {
            checked_length = Some(output);
        } else if profile != "O2" {
            assert_eq!(Some(output), checked_length, "{profile}/length");
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

const HOST: &str = r#"
globalThis.self = globalThis;
let reads = 0;
const mode = process.argv[3];
globalThis.installBufferProbe = function() {
  const p = Uint8Array.prototype;
  const t = Object.getPrototypeOf(p);
  const get = Object.getOwnPropertyDescriptor;
  const apply = Reflect.apply;
  if (mode === 'buffer' || mode === 'byteOffset' || mode === 'byteLength' || mode === 'length' || mode === 'consoleByteLength') {
    const property = mode === 'consoleByteLength' ? 'byteLength' : mode;
    const d = get(t, property);
    Object.defineProperty(p, property, {configurable: true, get() { reads++; return apply(d.get, this, []); }});
  } else if (mode === 'baseBuffer') {
    const d = get(t, 'buffer');
    Object.defineProperty(t, 'buffer', {...d, get() { reads++; return apply(d.get, this, []); }});
  } else if (mode === 'resizable') {
    const d = get(ArrayBuffer.prototype, 'resizable');
    if (d) Object.defineProperty(ArrayBuffer.prototype, 'resizable', {...d, get() { reads++; return apply(d.get, this, []); }});
  } else if (mode === 'descriptor') {
    Object.getOwnPropertyDescriptor = function(...args) { reads++; return apply(get, Object, args); };
  } else if (mode === 'isView') {
    const f = ArrayBuffer.isView;
    ArrayBuffer.isView = function(...args) { reads++; return apply(f, ArrayBuffer, args); };
  }
  if (mode === 'consoleByteLength') {
    const log = console.log;
    console.log = function(...args) {
      // Model console transports that inspect typed arrays while printing.
      const bytes = new Uint8Array(1);
      void bytes.byteLength;
      return apply(log, console, args);
    };
  }
};
globalThis.bufferReads = () => reads;
require(process.argv[2]);
"#;
