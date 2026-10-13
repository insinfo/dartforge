//! Minimal cases of the inference gap groups measured against the oracle
//! (`tools/oraculo_tipos`): each case pins the static type that
//! `package:analyzer` gives an expression. They use the real 3.6.2 SDK
//! (`SdkLayout::discover` / `DARTFORGE_SDK_LIB`); without it, the tests are
//! skipped.

use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_types::{infer_program_bodies, resolve_outline};
use std::path::PathBuf;

fn sdk() -> Option<SdkLayout> {
    let dir = SdkLayout::discover().or_else(|| {
        let p = PathBuf::from("C:/tools/dartsdk-3.6.2/lib");
        p.join("libraries.json").exists().then_some(p)
    })?;
    SdkLayout::load(&dir, "dartdevc").ok()
}

/// Types (in the analyzer's format) of the expressions whose text is exactly
/// `snippet`, in source order; and the warnings.
struct Result_ {
    tipos: Vec<(String, String)>,
    avisos: Vec<String>,
}

impl Result_ {
    fn tipo(&self, snippet: &str) -> &str {
        self.tipos
            .iter()
            .find(|(t, _)| t == snippet)
            .map(|(_, ty)| ty.as_str())
            .unwrap_or_else(|| panic!("expression `{snippet}` not found"))
    }
}

fn inferir(code: &str) -> Option<Result_> {
    sdk()?;
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.dart");
    std::fs::write(&main, code).unwrap();
    Some(infer_file(&main))
}

fn infer_file(main: &std::path::Path) -> Result_ {
    let sdk = sdk().unwrap();
    let main = main.to_path_buf();
    std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(move || {
            let mut interner = Interner::new();
            let (prog, _) = load_lenient(&main, &sdk, None, &mut interner);
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &prog, &interner);
            let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
            let (bodies, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
            let entry = prog.library(prog.entry.unwrap()).units[0];
            let unit = prog.unit(entry);
            let bt = &bodies.units[entry.0 as usize];
            let mut types = Vec::new();
            for (i, e) in unit.ast.exprs.iter().enumerate() {
                let text = unit.source[e.span.start..e.span.end].to_string();
                let t = bt.static_types[i];
                types.push((e.span.start, text, formatar(&table, t, &interner, &prog)));
            }
            types.sort_by_key(|(s, _, _)| *s);
            Result_ {
                tipos: types.into_iter().map(|(_, a, b)| (a, b)).collect(),
                avisos: diags.iter().map(|d| d.message.clone()).collect(),
            }
        })
        .unwrap()
        .join()
        .unwrap()
}

/// Like `DartType.getDisplayString()`.
fn formatar(t: &TypeTable, ty: TypeId, i: &Interner, p: &dartforge_elements::model::Program) -> String {
    let q = |n: bool| if n { "?" } else { "" };
    match t.get(ty) {
        Type::Intersection { param, bound } => format!("{} & {}", i.resolve(t.param(*param).name), formatar(t, *bound, i, p)),
        Type::Dynamic => "dynamic".into(),
        Type::Void => "void".into(),
        Type::Never => "Never".into(),
        Type::Null => "Null".into(),
        Type::Interface { class, args, nullable } | Type::ExtensionType { decl: class, args, nullable } => {
            let name = i.resolve(p.class(*class).name);
            if args.is_empty() {
                format!("{name}{}", q(*nullable))
            } else {
                let a: Vec<String> = args.iter().map(|&x| formatar(t, x, i, p)).collect();
                format!("{name}<{}>{}", a.join(", "), q(*nullable))
            }
        }
        Type::FutureOr { arg, nullable } => format!("FutureOr<{}>{}", formatar(t, *arg, i, p), q(*nullable)),
        Type::TypeParameter { param, nullable } => format!("{}{}", i.resolve(t.param(*param).name), q(*nullable)),
        Type::Record { positional, named, nullable } => {
            let mut parts: Vec<String> = positional.iter().map(|&x| formatar(t, x, i, p)).collect();
            let mut n: Vec<(String, String)> = named.iter().map(|(s, x)| (i.resolve(*s).to_string(), formatar(t, *x, i, p))).collect();
            n.sort();
            if !n.is_empty() {
                parts.push(format!("{{{}}}", n.iter().map(|(s, x)| format!("{x} {s}")).collect::<Vec<_>>().join(", ")));
            } else if positional.len() == 1 {
                return format!("({},){}", parts[0], q(*nullable));
            }
            format!("({}){}", parts.join(", "), q(*nullable))
        }
        Type::Function { type_params, ret, positional, optional, named, nullable } => {
            let tps = if type_params.is_empty() {
                String::new()
            } else {
                let v: Vec<String> = type_params
                    .iter()
                    .map(|&tp| {
                        let d = t.param(tp);
                        let name = i.resolve(d.name).to_string();
                        match t.get(d.bound) {
                            Type::Interface { nullable: true, class, .. } if i.resolve(p.class(*class).name) == "Object" => name,
                            Type::Dynamic => name,
                            _ => format!("{name} extends {}", formatar(t, d.bound, i, p)),
                        }
                    })
                    .collect();
                format!("<{}>", v.join(", "))
            };
            let mut parts: Vec<String> = positional.iter().map(|&x| formatar(t, x, i, p)).collect();
            if !optional.is_empty() {
                parts.push(format!("[{}]", optional.iter().map(|&x| formatar(t, x, i, p)).collect::<Vec<_>>().join(", ")));
            }
            if !named.is_empty() {
                let mut n: Vec<(String, String)> = named
                    .iter()
                    .map(|(s, x, r)| (i.resolve(*s).to_string(), format!("{}{} {}", if *r { "required " } else { "" }, formatar(t, *x, i, p), i.resolve(*s))))
                    .collect();
                n.sort();
                parts.push(format!("{{{}}}", n.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(", ")));
            }
            format!("{} Function{tps}({}){}", formatar(t, *ret, i, p), parts.join(", "), q(*nullable))
        }
    }
}

macro_rules! or_skip {
    ($e:expr) => {
        match $e {
            Some(r) => r,
            None => {
                eprintln!("SDK 3.6.2 unavailable: skipping");
                return;
            }
        }
    };
}

/// Base group: literals, closures (inferred body), collection generics,
/// indices, numeric operators, `super`, generic constructors.
#[test]
fn new_engine_base() {
    let r = or_skip!(inferir(
        r#"
class A { int f() => 1; }
class B extends A { void g() { var s = super.f(); } }
void main() {
  var d = 1.5;
  var q = 10 / 3;
  var lista = [1, 2, 3];
  var m = <String, dynamic>{};
  var v = m['x'];
  var dobro = lista.map((x) => x * 2).toList();
  var soma = lista.fold(0, (a, b) => a + b);
  var n = 3 + 1.0;
  var s = {'a': 1, 'b': 2.0};
  var vazio = {};
  List<double> ld = [1, 2];
  var e = lista.isEmpty ? null : lista.first;
}
"#
    ));
    assert_eq!(r.tipo("1.5"), "double");
    assert_eq!(r.tipo("10 / 3"), "double");
    assert_eq!(r.tipo("[1, 2, 3]"), "List<int>");
    assert_eq!(r.tipo("m['x']"), "dynamic");
    assert_eq!(r.tipo("lista.map((x) => x * 2).toList()"), "List<int>");
    assert_eq!(r.tipo("(x) => x * 2"), "int Function(int)");
    assert_eq!(r.tipo("lista.fold(0, (a, b) => a + b)"), "int");
    assert_eq!(r.tipo("3 + 1.0"), "double");
    assert_eq!(r.tipo("{'a': 1, 'b': 2.0}"), "Map<String, num>");
    assert_eq!(r.tipo("{}"), "Map<dynamic, dynamic>");
    assert_eq!(r.tipo("[1, 2]"), "List<double>");
    assert_eq!(r.tipo("lista.isEmpty ? null : lista.first"), "int?");
    assert_eq!(r.tipo("super.f()"), "int");
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// A raw type in an outline annotation is instantiated to bounds (`Map` →
/// `Map<dynamic, dynamic>`, `C` with `T extends num` → `C<num>`).
#[test]
fn raw_type_instantiated_to_bounds() {
    let r = or_skip!(inferir(
        r#"
class C<T extends num> { T? v; }
Map dados = {};
C c = C();
void main() {
  var d = dados;
  var v = c.v;
}
"#
    ));
    assert_eq!(r.tipo("dados"), "Map<dynamic, dynamic>");
    assert_eq!(r.tipo("c.v"), "num?");
}

/// Named constructors, `$this`, extension fields, `null.hashCode`, a generic
/// tear-off as a call target (uninstantiated type).
#[test]
fn named_constructors_and_references() {
    let r = or_skip!(inferir(
        r#"
typedef Cb = dynamic Function(List<String>, {String rawValue});
class E {
  final int v;
  const E.vazio() : v = 0;
  factory E.deJson(Map m) => E.vazio();
  Cb? callback;
}
T id<T>(T x) => x;
extension X on num {
  static const nomes = 'abc';
  String f() => '$this ${nomes.length}';
}
void main() {
  var a = const E.vazio();
  var b = new E.vazio();
  var c = E.deJson({});
  var h = null.hashCode;
  var i = id(3);
  var cb = E.vazio().callback;
}
"#
    ));
    assert_eq!(r.tipo("const E.vazio()"), "E");
    assert_eq!(r.tipo("new E.vazio()"), "E");
    assert_eq!(r.tipo("E.deJson({})"), "E");
    assert_eq!(r.tipo("null.hashCode"), "int");
    assert_eq!(r.tipo("id"), "T Function<T>(T)");
    assert_eq!(r.tipo("id(3)"), "int");
    assert_eq!(r.tipo("nomes.length"), "int");
    assert_eq!(r.tipo("E.vazio().callback"), "dynamic Function(List<String>, {String rawValue})?");
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// `late final x;` without an initializer has a setter (single assignment):
/// the assignment types the value through the field, without an undefined
/// setter warning.
#[test]
fn late_final_setter() {
    let r = or_skip!(inferir(
        r#"
T cast<T>(Object? o) => o as T;
class V {
  late final String nome;
  void iniciar(Object o) {
    nome = cast(o);
    this.nome = cast(o);
  }
}
"#
    ));
    assert_eq!(r.tipo("nome = cast(o)"), "String");
    assert_eq!(r.tipo("this.nome = cast(o)"), "String");
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// An SDK class whose patch comes in a *part* of a patch file
/// (`core_patch.dart` → `part 'bigint_patch.dart'`): a single class, with the
/// members of the original declaration (`BigInt.operator <<`).
#[test]
fn patch_in_part_of_patch() {
    let r = or_skip!(inferir(
        r#"
void main() {
  var n = BigInt.parse('1');
  var s = n << 8;
  var b = n.bitLength;
}
"#
    ));
    assert_eq!(r.tipo("BigInt.parse('1')"), "BigInt");
    assert_eq!(r.tipo("n << 8"), "BigInt");
    assert_eq!(r.tipo("n.bitLength"), "int");
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// An old-style function parameter with `?` (`int f(int x)?`): the
/// parameter's type is nullable (`jsonEncode`, `List.sort`, `Stream.listen`).
#[test]
fn old_nullable_function_parameter() {
    let r = or_skip!(inferir(
        r#"
void g([int comparar(String a, String b)?]) {}
void main() {
  var x = g;
  var l = <String>[];
  var s = l.sort;
}
"#
    ));
    assert_eq!(r.tipo("g"), "void Function([int Function(String, String)?])");
    assert_eq!(r.tipo("l.sort"), "void Function([int Function(String, String)?])");
}

/// Override inference by position (the parameter's name may change), a
/// generic extension used from inside itself, `-1` with a `double` context,
/// and a closure that does not inherit the promotion of a variable written
/// in the body.
#[test]
fn extension_override_and_closure() {
    let r = or_skip!(inferir(
        r#"
typedef F<T> = void Function(T v);
abstract class Base<T> { void registrar(F<T> f); }
class Impl implements Base<String> {
  @override
  void registrar(callback) { callback('x'); }
}
extension Divide<T> on Iterable<T> {
  Iterable<List<T>> partes() => [toList()];
  Iterable<List<T>> duas() => partes();
}
void main() {
  double d = -1;
  int? w;
  w ??= 3;
  var f = () => w;
}
"#
    ));
    assert_eq!(r.tipo("callback"), "void Function(String)");
    assert_eq!(r.tipo("partes()"), "Iterable<List<T>>");
    assert_eq!(r.tipo("-1"), "double");
    assert_eq!(r.tipo("() => w"), "int? Function()");
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// Import conflict between `dart:` and a package: the system one stays
/// hidden (`dart:html` also exports `NotificationEvent`).
#[test]
fn system_import_loses_conflict() {
    let Some(sdk) = sdk() else { return };
    let _ = sdk;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lib.dart"), "class Duration { final int n; Duration(this.n); }").unwrap();
    let main = dir.path().join("main.dart");
    std::fs::write(&main, "import 'dart:core';
import 'lib.dart';
void main() { var d = Duration(3); var n = d.n; }").unwrap();
    let r = infer_file(&main);
    assert_eq!(r.tipo("d.n"), "int");
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// Type variable promotion: the intersection `X & B` (`T?` tested against
/// `null` becomes `T & Object`; `is int` becomes `T & int`).
#[test]
fn type_variable_promotion() {
    let r = or_skip!(inferir(
        r#"
T f<T>(T? a, T b, Map<String, T> m) {
  if (a != null) { var x = a; }
  if (b is int) { var y = b; }
  var z = m['k']!;
  return b;
}
"#
    ));
    let types: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "a").map(|(_, ty)| ty.as_str()).collect();
    assert_eq!(types, ["T?", "T & Object"]);
    let tb: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "b").map(|(_, ty)| ty.as_str()).collect();
    assert_eq!(tb, ["T", "T & int", "T"]);
    assert_eq!(r.tipo("m['k']"), "T?");
    assert_eq!(r.tipo("m['k']!"), "T & Object");
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// Promotion of a private final field (Dart 3.2), through `this`, implicit
/// and through a local; a non-final same-named one in the library prevents
/// it.
#[test]
fn private_field_promotion() {
    let r = or_skip!(inferir(
        r#"
class A {
  final int? _x;
  final Object _o;
  int? _y;
  A(this._x, this._o, this._y);
  int f() {
    if (_x != null) { var a = _x; }
    if (this._o is String) { var b = this._o; }
    if (_y != null) { var c = _y; }
    return 0;
  }
}
int g(A a) {
  if (a._x == null) return 0;
  return a._x;
}
"#
    ));
    let types = |t: &str| r.tipos.iter().filter(|(x, _)| x == t).map(|(_, y)| y.as_str()).collect::<Vec<_>>();
    assert_eq!(types("_x"), ["int?", "int"]);
    assert_eq!(types("this._o"), ["Object", "String"]);
    assert_eq!(types("_y"), ["int?", "int?"]);
    assert_eq!(types("a._x"), ["int?", "int"]);
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// `?.` promotes the receiver only inside the chain (arguments included);
/// `clamp` with a `double` context; inside a local function, a variable
/// written outside it (even afterwards) is promoted, and one written inside
/// another local function is not
/// (`conservativeJoin(anywhere.written, anywhere.captured)`; probed on
/// analyzer 3.6.2 and 3.13.4: only the second case gives
/// `unchecked_use_of_nullable_value`).
#[test]
fn chain_clamp_and_local_function() {
    let r = or_skip!(inferir(
        r#"
class S { int? d; S copia(int e) => this; }
void f(S? s, double? h, double ph) {
  var c = s?.copia(s.d ?? 0);
  final double y = h != null ? h.clamp(1, ph) : 50.0;
  StringBuffer? buf;
  void fecha() {
    if (buf != null) { buf.write('x'); }
  }
  buf = StringBuffer();
}
void g() {
  StringBuffer? cap;
  void abre() { cap = null; }
  void usa() {
    if (cap != null) { cap.write('x'); }
  }
}
"#
    ));
    let types = |t: &str| r.tipos.iter().filter(|(x, _)| x == t).map(|(_, y)| y.as_str()).collect::<Vec<_>>();
    assert_eq!(types("s"), ["S?", "S"]);
    assert_eq!(types("1"), ["double"]);
    assert_eq!(types("buf")[..2], ["StringBuffer?", "StringBuffer"]);
    // `cap = null` (write), `cap != null`, `cap.write`: captured, does not promote.
    assert_eq!(types("cap")[1..3], ["StringBuffer?", "StringBuffer?"]);
}

/// A write to a same-named variable declared in another closure (the `main`
/// of a test file with several `test(…)`) does not capture this one's
/// variable: writes are resolved by declaration, in the lexical scope
/// (`limitless_ui`, `li_datatable_component_test.dart`).
#[test]
fn same_name_write_in_other_closure_does_not_capture() {
    let r = or_skip!(inferir(
        r#"
void rodar(void Function() f) => f();
void main() {
  rodar(() {
    final StringBuffer? botao = StringBuffer();
    botao!.write('a');
    rodar(() { botao.write('b'); });
  });
  rodar(() {
    StringBuffer? botao = StringBuffer();
    botao = null;
  });
}
"#
    ));
    let types = |t: &str| r.tipos.iter().filter(|(x, _)| x == t).map(|(_, y)| y.as_str()).collect::<Vec<_>>();
    // `botao!`, then `botao.write` inside the closure: promoted.
    assert_eq!(types("botao")[..2], ["StringBuffer?", "StringBuffer"]);
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// Cases from the bodies of the SDK compiled from source: an extension
/// parameter bound, a typedef that renames a generic class, `super.x`
/// parameters (substituted type and implicit arguments), the tested type in
/// the false branch of `is!`, a local function parameter shadowing the outer
/// one, subtyping between generic functions, a forwarded constructor of a
/// mixin application and referenced constants.
#[test]
fn sdk_bodies() {
    let r = or_skip!(inferir(
        r#"
abstract class E { String get _name; }
extension X<T extends E> on Iterable<T> {
  String a(T v) => v._name;
}
class _M<K, V> { _M(); }
typedef DM<K, V> = _M<K, V>;
_M<K, V> f<K, V>() => DM<K, V>();
class IB<T> { final Iterable<T> _source; final int _start; IB._(this._source, this._start); }
class EB<T> extends IB<T> { EB(super._source, super._start) : super._(); }
EB<T> g<T>(Iterable<T> s) => EB<T>(s, 0);
void h(Object x) {
  if (x is! int) { x = 3; }
  int k = x;
}
class N { N? next; }
N? copia(N? node) {
  if (node == null) return null;
  void filhos(N node) { node = node.next!; }
  filhos(node);
  return node;
}
class C<S> {
  final Set<R> Function<R>()? _vazio;
  C(this._vazio);
  C<R> cast<R>() => C<R>(_vazio);
}
class B<T> { B(T t); }
mixin M<T> {}
class A<T> = B<T> with M<T>;
A<int> mk() => A<int>(1);
const int base = 4;
const int deslocado = base << 5;
const lista = [base, deslocado, 'x$base'];
"#
    ));
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
}

/// Promotion of a private final field only from language version 3.2
/// (R-FLU-12): in a `// @dart = 3.1` library the `!` is needed and the field
/// stays nullable.
#[test]
fn field_promotion_depends_on_version() {
    let r = or_skip!(inferir(
        r#"// @dart = 3.1
class A {
  final int? _x;
  A(this._x);
  int f() => _x != null ? _x! : 0;
}
"#
    ));
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
    let types: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "_x").map(|(_, y)| y.as_str()).collect();
    assert_eq!(types, ["int?", "int?"]);
}

/// Condition variable (§7.10): `final bool v = d != null; if (v) d` promotes
/// `d`; writing `d` afterwards invalidates it.
#[test]
fn condition_variable() {
    let r = or_skip!(inferir(
        r#"
double f(double? d, double? e) {
  var soma = 0.0;
  final bool v = d != null;
  if (v) soma += d;
  var w = e != null && soma > 0;
  e = null;
  if (w) print(e);
  return soma;
}
"#
    ));
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
    let types: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "e").map(|(_, y)| y.as_str()).collect();
    // A write after the condition: the variable does not restore the promotion.
    assert_eq!(types.last(), Some(&"double?"));
}

/// An extension type that implements another extension type
/// (`Element implements JSObject`, `JSString implements JSAny` in
/// package:web).
#[test]
fn extension_type_implements_extension_type() {
    let r = or_skip!(inferir(
        r#"
extension type A._(Object _) {}
extension type B._(Object _) implements A {}
extension type C._(Object _) implements B { C(B b) : _ = b; }
A f(C c) => c;
"#
    ));
    // `C <: A` through `implements`; `B` is not a subtype of `Object` (no
    // superinterface leads to a class), and analyzer 3.6.2 reports the
    // initializer `_ = b` (checked against `dart analyze`).
    assert_eq!(r.avisos, vec!["The initializer type 'B' can't be assigned to the field type 'Object'.".to_string()], "warnings: {:?}", r.avisos);
}

/// An object with `call` passed to a function-typed parameter of a generic
/// call: the `call` coercion comes before the constraint, and `T` comes from
/// the return of `call` (the analyzer gives `Iterable<int>`).
#[test]
fn implicit_call_restricts_generic_invocation() {
    let r = or_skip!(inferir(
        r#"
class Somador {
  final int base;
  Somador(this.base);
  int call(int x) => base + x;
}
void main() {
  final s = Somador(5);
  var l = [1, 2, 3].map(s);
  print(l);
}
"#
    ));
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
    assert_eq!(r.tipo("[1, 2, 3].map(s)"), "Iterable<int>");
}

/// `Never` as a value is a type literal, like `dynamic` (the analyzer gives
/// `Type`); there is no declaration of it in `dart:core`.
#[test]
fn never_as_type_literal() {
    let r = or_skip!(inferir(
        r#"
void main() {
  var t = Never;
  print([Never, Null, dynamic]);
}
"#
    ));
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
    assert_eq!(r.tipo("Never"), "Type");
    assert_eq!(r.tipo("[Never, Null, dynamic]"), "List<Type>");
}

/// The variable of a pattern that shadows the scrutinee (`Neg(:final e)` in
/// `switch (e)`) has the field's type; the case's promotion applies to the
/// scrutinee, not to it (the analyzer gives `Expr` to the body's `e`).
#[test]
fn pattern_variable_shadowing_scrutinee() {
    let r = or_skip!(inferir(
        r#"
sealed class Expr {}
class Num extends Expr { final int valor; Num(this.valor); }
class Neg extends Expr { final Expr e; Neg(this.e); }
Object mostra(Expr e) => switch (e) {
      Num() => e,
      Neg(:final e) => e,
    };
Object comando(Expr e) {
  switch (e) {
    case Neg(:final e):
      return e;
    case Num():
      return e;
  }
}
"#
    ));
    assert!(r.avisos.is_empty(), "warnings: {:?}", r.avisos);
    let types: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "e").map(|(_, y)| y.as_str()).collect();
    assert_eq!(types, ["Expr", "Num", "Expr", "Expr", "Expr", "Num"]);
}
