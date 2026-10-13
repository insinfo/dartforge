//! Support for the LSP semantic tests (completion, rename, actions): a
//! minimal SDK on disk, a temporary project and the server speaking
//! JSON-RPC.
#![allow(dead_code)]

use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{SemanticAnalyzer, Server};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// A reduced `dart:core`: enough for inherited members, generics, enums and
/// extensions, without depending on an installed SDK.
const CORE: &str = r#"library dart.core;
class Object {
  const Object();
  bool operator ==(Object other) => true;
  external int get hashCode;
  external String toString();
  external dynamic noSuchMethod(Invocation invocation);
  external Type get runtimeType;
}
class Invocation {}
class Type {}
class Null {}
class bool {}
abstract class Comparable<T> { int compareTo(T other); }
abstract class num implements Comparable<num> {
  num operator +(num other);
  num abs();
}
abstract class int extends num {
  int operator +(num other);
  bool get isEven;
}
abstract class double extends num {}
abstract class String implements Comparable<String> {
  int get length;
  String substring(int start, [int? end]);
  String toUpperCase();
}
abstract class Iterable<E> {
  E get first;
  int get length;
}
abstract class List<E> implements Iterable<E> {
  void add(E value);
  E removeLast();
}
class MapEntry<K, V> {
  final K key;
  final V value;
  const MapEntry._(this.key, this.value);
}
abstract class Map<K, V> {
  external factory Map();
  Iterable<MapEntry<K, V>> get entries;
  external factory Map.fromIterable(Iterable iterable, {K Function(dynamic element)? key, V Function(dynamic element)? value});
}
abstract class Set<E> implements Iterable<E> {
  external factory Set();
  external factory Set.of(Iterable<E> elements);
}
abstract class Function {}
abstract class Record {}
abstract class Enum {
  int get index;
}
abstract class Symbol {}
abstract class Pattern {}
class Deprecated { const Deprecated(String message); }
const Object override = _Override();
class _Override { const _Override(); }
void print(Object? object) {}
"#;

/// Temporary project with a minimal SDK and a semantic server.
pub struct Project {
    pub root: PathBuf,
    pub server: Server<SemanticAnalyzer>,
    next_id: i64,
    /// The server's `initialize` response (the server rejects a second
    /// `initialize`, like Dart's: `serverAlreadyInitialized`).
    #[allow(dead_code)]
    pub initialization: Value,
    /// The open documents, for [`Project::restart`].
    open: Vec<(String, String)>,
}

impl Project {
    /// Creates `target/tmp-agent/<name>-<pid>` with `pubspec.yaml` and an SDK.
    pub fn new(name: &str) -> Self {
        Self::with_capabilities(name, json!({}))
    }

    /// [`Project::new`] with literal `CodeAction` support (without it, the Dart
    /// server offers neither fixes nor assists).
    #[allow(dead_code)]
    pub fn with_literals(name: &str) -> Self {
        Self::with_capabilities(
            name,
            json!({"textDocument": {"codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}}}),
        )
    }

    /// [`Project::new`] with the client capabilities given in `initialize`.
    #[allow(dead_code)]
    pub fn with_capabilities(name: &str, capabilities: Value) -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/lsp-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("projeto/lib")).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let lib = root.join("sdk/lib");
        fs::create_dir_all(lib.join("core")).unwrap();
        fs::write(
            lib.join("libraries.json"),
            r#"{"dartdevc":{"libraries":{
                "core":{"uri":"core/core.dart","patches":[]},
                "math":{"uri":"math/math.dart","patches":[]},
                "_interna":{"uri":"interna/interna.dart","patches":[]}
            }}}"#,
        )
        .unwrap();
        fs::write(lib.join("core/core.dart"), CORE).unwrap();
        fs::create_dir_all(lib.join("math")).unwrap();
        fs::write(
            lib.join("math/math.dart"),
            "library dart.math;\npart 'aleatorio.dart';\nconst double pi = 3.14;\n",
        )
        .unwrap();
        fs::write(
            lib.join("math/aleatorio.dart"),
            "part of dart.math;\nclass Random { int nextInt(int max) => 0; }\n",
        )
        .unwrap();
        fs::create_dir_all(lib.join("interna")).unwrap();
        fs::write(
            lib.join("interna/interna.dart"),
            "library dart._interna;\nclass Random {}\n",
        )
        .unwrap();
        let sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
        fs::write(root.join("projeto/pubspec.yaml"), "name: projeto\n").unwrap();
        fs::create_dir_all(root.join("projeto/.dart_tool")).unwrap();
        fs::write(
            root.join("projeto/.dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"projeto","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
        )
        .unwrap();
        let server = Server::with_analyzer(SemanticAnalyzer::new(Some(sdk)));
        let mut p = Self {
            root,
            server,
            next_id: 1,
            initialization: Value::Null,
            open: Vec::new(),
        };
        p.initialization = p.initialize(json!({"capabilities": capabilities}));
        p
    }

    /// A reduced `flutter` package next to the project (the classes the Flutter
    /// assists recognize by file URI), in `package_config.json`.
    pub fn install_flutter(&self) {
        let lib = self.root.join("flutter/lib");
        let files: &[(&str, &str)] = &[
            ("foundation.dart", "export 'src/foundation/diagnostics.dart';\n"),
            (
                "widgets.dart",
                "export 'src/widgets/framework.dart';\nexport 'src/widgets/basic.dart';\nexport 'src/widgets/container.dart';\nexport 'src/widgets/async.dart';\nexport 'src/widgets/text.dart';\nexport 'src/painting/edge_insets.dart';\nexport 'src/foundation/diagnostics.dart';\n",
            ),
            (
                "src/widgets/framework.dart",
                "import '../foundation/diagnostics.dart';\nabstract class Key {}\nabstract class BuildContext {}\nabstract class Widget with Diagnosticable {\n  final Key? key;\n  const Widget({this.key});\n}\nabstract class StatelessWidget extends Widget {\n  const StatelessWidget({super.key});\n  Widget build(BuildContext context);\n}\nabstract class StatefulWidget extends Widget {\n  const StatefulWidget({super.key});\n  State createState();\n}\nabstract class State<T extends StatefulWidget> with Diagnosticable {\n  T get widget => throw 0;\n  BuildContext get context => throw 0;\n  void initState() {}\n  void setState(void Function() fn) {}\n  Widget build(BuildContext context);\n}\n",
            ),
            (
                "src/widgets/basic.dart",
                "import 'framework.dart';\nimport '../painting/edge_insets.dart';\nclass Center extends StatelessWidget {\n  const Center({super.key, this.child});\n  final Widget? child;\n  @override\n  Widget build(BuildContext context) => this;\n}\nclass Align extends StatelessWidget {\n  const Align({super.key, this.child});\n  final Widget? child;\n  @override\n  Widget build(BuildContext context) => this;\n}\nclass Padding extends StatelessWidget {\n  const Padding({super.key, required this.padding, this.child});\n  final EdgeInsetsGeometry padding;\n  final Widget? child;\n  @override\n  Widget build(BuildContext context) => this;\n}\nclass SizedBox extends StatelessWidget {\n  const SizedBox({super.key, this.width, this.height, this.child});\n  final double? width;\n  final double? height;\n  final Widget? child;\n  @override\n  Widget build(BuildContext context) => this;\n}\nclass Column extends StatelessWidget {\n  const Column({super.key, this.children = const []});\n  final List<Widget> children;\n  @override\n  Widget build(BuildContext context) => this;\n}\nclass Row extends StatelessWidget {\n  const Row({super.key, this.children = const []});\n  final List<Widget> children;\n  @override\n  Widget build(BuildContext context) => this;\n}\nclass Builder extends StatelessWidget {\n  const Builder({super.key, required this.builder});\n  final Widget Function(BuildContext context) builder;\n  @override\n  Widget build(BuildContext context) => this;\n}\n",
            ),
            (
                "src/widgets/container.dart",
                "import 'framework.dart';\nclass Container extends StatelessWidget {\n  Container({super.key, this.child});\n  final Widget? child;\n  @override\n  Widget build(BuildContext context) => this;\n}\n",
            ),
            (
                "src/widgets/async.dart",
                "import 'framework.dart';\nclass StreamBuilder<T> extends StatefulWidget {\n  const StreamBuilder({super.key, this.stream, required this.builder});\n  final Object? stream;\n  final Widget Function(BuildContext context, Object? snapshot) builder;\n  @override\n  State createState() => throw 0;\n}\n",
            ),
            (
                "src/widgets/text.dart",
                "import 'framework.dart';\nclass Text extends StatelessWidget {\n  const Text(this.data, {super.key});\n  final String data;\n  @override\n  Widget build(BuildContext context) => this;\n}\n",
            ),
            (
                "src/painting/edge_insets.dart",
                "abstract class EdgeInsetsGeometry {\n  const EdgeInsetsGeometry();\n}\nclass EdgeInsets extends EdgeInsetsGeometry {\n  const EdgeInsets.all(double value);\n}\n",
            ),
            (
                "src/foundation/diagnostics.dart",
                "class DiagnosticPropertiesBuilder {\n  void add(DiagnosticsNode property) {}\n}\nabstract class DiagnosticsNode {}\nclass DiagnosticsProperty<T> extends DiagnosticsNode {\n  DiagnosticsProperty(String name, T? value);\n}\nclass IntProperty extends DiagnosticsNode {\n  IntProperty(String name, int? value);\n}\nclass DoubleProperty extends DiagnosticsNode {\n  DoubleProperty(String name, double? value);\n}\nclass StringProperty extends DiagnosticsNode {\n  StringProperty(String name, String? value);\n}\nclass FlagProperty extends DiagnosticsNode {\n  FlagProperty(String name, {required bool? value});\n}\nmixin Diagnosticable {\n  void debugFillProperties(DiagnosticPropertiesBuilder properties) {}\n}\n",
            ),
        ];
        for (rel, text) in files {
            let c = lib.join(rel);
            fs::create_dir_all(c.parent().unwrap()).unwrap();
            fs::write(c, text).unwrap();
        }
        fs::write(
            self.root.join("projeto/.dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"projeto","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"},{"name":"flutter","rootUri":"../../flutter","packageUri":"lib/","languageVersion":"3.6"}]}"#,
        )
        .unwrap();
    }

    /// `initialize` (with the project's `rootUri` added to `params`).
    fn initialize(&mut self, mut params: Value) -> Value {
        params["rootUri"] = json!(url::Url::from_file_path(self.root.join("projeto")).unwrap().to_string());
        self.server.receive(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":params}));
        self.server.pump().into_iter().find(|m| m["id"] == json!(0)).unwrap_or(Value::Null)
    }

    /// A new server, initialized with `params`, with the documents that were
    /// open reopened; returns the `initialize` response.
    #[allow(dead_code)]
    pub fn restart(&mut self, params: Value) -> Value {
        let sdk = SdkLayout::load(&self.root.join("sdk/lib"), "dartdevc").unwrap();
        self.server = Server::with_analyzer(SemanticAnalyzer::new(Some(sdk)));
        self.initialization = self.initialize(params);
        for (rel, text) in std::mem::take(&mut self.open) {
            self.open(&rel, &text);
        }
        self.initialization.clone()
    }

    /// Path of a project file.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join("projeto").join(relative)
    }

    /// `file:` URI of a project file.
    pub fn uri(&self, relative: &str) -> String {
        url::Url::from_file_path(self.path(relative))
            .unwrap()
            .to_string()
    }

    /// Writes to disk (without opening in the editor).
    pub fn write(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Writes to disk and opens in the editor; returns the published
    /// diagnostics. Without a publication (like the Dart server, no empty list
    /// goes out for a file that had no published errors), the empty list.
    pub fn open(&mut self, relative: &str, text: &str) -> Value {
        self.write(relative, text);
        self.open.retain(|(r, _)| r != relative);
        self.open.push((relative.to_string(), text.to_string()));
        let uri = self.uri(relative);
        self.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
        }}));
        self.server
            .pump()
            .into_iter()
            .rev()
            .find(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri)
            .unwrap_or_else(|| json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics", "params": {"uri": uri, "diagnostics": []}}))
    }

    /// Replaces the text of the open document (new version), without writing.
    pub fn change(&mut self, relative: &str, version: i64, text: &str) {
        self.server.receive(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":self.uri(relative),"version":version},
                "contentChanges":[{"text":text}]
            }}),
        );
        self.server.pump();
    }

    /// Sends a request and returns the whole response.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.server
            .receive(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let outputs = self.server.pump();
        outputs
            .into_iter()
            .find(|m| m["id"] == json!(id))
            .expect("response")
    }

    /// Answers the server's `workspace/configuration` requests with the given
    /// `dart` section (the server asks for them on `initialized` and on each
    /// `workspace/didChangeConfiguration`; the client must have announced
    /// `workspace.configuration`).
    #[allow(dead_code)]
    pub fn configure(&mut self, dart: Value) {
        self.server
            .receive(json!({"jsonrpc":"2.0","method":"workspace/didChangeConfiguration","params":{"settings":{}}}));
        let mut outputs = self.server.pump();
        while let Some(request) = outputs.iter().find(|m| m["method"] == "workspace/configuration").cloned() {
            let n = request["params"]["items"].as_array().map_or(1, |a| a.len());
            let result: Vec<Value> = (0..n).map(|_| dart.clone()).collect();
            self.server
                .receive(json!({"jsonrpc":"2.0","id":request["id"].clone(),"result":result}));
            outputs = self.server.pump();
        }
    }

    /// Request with `textDocument` + `position` at the position of the `▮`
    /// marker in `text` (the marker is not part of the file).
    pub fn in_position(
        &mut self,
        method: &str,
        relative: &str,
        line: u32,
        column: u32,
        extra: Value,
    ) -> Value {
        let mut params = json!({
            "textDocument": {"uri": self.uri(relative)},
            "position": {"line": line, "character": column},
        });
        if let Value::Object(mais) = extra {
            for (k, v) in mais {
                params[k] = v;
            }
        }
        self.request(method, params)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        // The typed analysis writes the cache of the minimal SDK (one per project,
        // by path): removes it, so it does not accumulate across runs.
        if let Ok(sdk) = SdkLayout::load(&self.root.join("sdk/lib"), "dartdevc") {
            let _ = fs::remove_file(dartforge_elements::SdkCache::caminho(&sdk, "dartdevc"));
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Separates the `▮` marker from the text: returns the text without it and
/// the LSP position (line, UTF-16 column) where it was.
pub fn mark(text: &str) -> (String, u32, u32) {
    let i = text.find('▮').expect("marker");
    let before = &text[..i];
    let line = before.matches('\n').count() as u32;
    let column = before.rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    (text.replacen('▮', "", 1), line, column)
}

/// Labels of the items of a completion response, in order.
pub fn labels(response: &Value) -> Vec<String> {
    response["result"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("no items: {response}"))
        .iter()
        .map(|i| i["label"].as_str().unwrap().to_string())
        .collect()
}

/// The item labeled `label`.
pub fn item<'a>(response: &'a Value, label: &str) -> &'a Value {
    response["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["label"] == label)
        .unwrap_or_else(|| panic!("no {label} in {:?}", labels(response)))
}

/// Byte offset of an LSP position (line, UTF-16 column).
pub fn offset_of(text: &str, position: &Value) -> usize {
    let line = position["line"].as_u64().unwrap() as usize;
    let column = position["character"].as_u64().unwrap() as usize;
    let start: usize = text.split_inclusive('\n').take(line).map(str::len).sum();
    let mut units = 0;
    for (i, c) in text[start..].char_indices() {
        if units >= column || c == '\n' {
            return start + i;
        }
        units += c.len_utf16();
    }
    text.len()
}

/// Applies the edits (`TextEdit[]`) for `uri` of a `WorkspaceEdit` to `text`.
pub fn apply(edit: &Value, uri: &str, text: &str) -> String {
    let Some(list) = edit["changes"][uri].as_array() else {
        return text.to_string();
    };
    let mut swaps: Vec<(usize, usize, &str)> = list
        .iter()
        .map(|e| {
            (
                offset_of(text, &e["range"]["start"]),
                offset_of(text, &e["range"]["end"]),
                e["newText"].as_str().unwrap(),
            )
        })
        .collect();
    swaps.sort_by_key(|t| std::cmp::Reverse(t.0));
    let mut output = text.to_string();
    for (de, until, new) in swaps {
        output.replace_range(de..until, new);
    }
    output
}
