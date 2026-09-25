//! Casos mínimos dos grupos de lacunas de inferência medidos contra o
//! oráculo (`tools/oraculo_tipos`): cada caso fixa o tipo estático que o
//! `package:analyzer` dá a uma expressão. Usam o SDK 3.6.2 real
//! (`SdkLayout::discover` / `DARTFORGE_SDK_LIB`); sem ele, os testes pulam.

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

/// Tipos (no formato do analyzer) das expressões cujo texto é exatamente
/// `trecho`, na ordem do código-fonte; e os avisos.
struct Resultado {
    tipos: Vec<(String, String)>,
    avisos: Vec<String>,
}

impl Resultado {
    fn tipo(&self, trecho: &str) -> &str {
        self.tipos
            .iter()
            .find(|(t, _)| t == trecho)
            .map(|(_, ty)| ty.as_str())
            .unwrap_or_else(|| panic!("expressão `{trecho}` não encontrada"))
    }
}

fn inferir(codigo: &str) -> Option<Resultado> {
    sdk()?;
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.dart");
    std::fs::write(&main, codigo).unwrap();
    Some(inferir_arquivo(&main))
}

fn inferir_arquivo(main: &std::path::Path) -> Resultado {
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
            let entrada = prog.library(prog.entry.unwrap()).units[0];
            let unit = prog.unit(entrada);
            let bt = &bodies.units[entrada.0 as usize];
            let mut tipos = Vec::new();
            for (i, e) in unit.ast.exprs.iter().enumerate() {
                let texto = unit.source[e.span.start..e.span.end].to_string();
                let t = bt.static_types[i];
                tipos.push((e.span.start, texto, formatar(&table, t, &interner, &prog)));
            }
            tipos.sort_by_key(|(s, _, _)| *s);
            Resultado {
                tipos: tipos.into_iter().map(|(_, a, b)| (a, b)).collect(),
                avisos: diags.iter().map(|d| d.message.clone()).collect(),
            }
        })
        .unwrap()
        .join()
        .unwrap()
}

/// Como `DartType.getDisplayString()`.
fn formatar(t: &TypeTable, ty: TypeId, i: &Interner, p: &dartforge_elements::model::Program) -> String {
    let q = |n: bool| if n { "?" } else { "" };
    match t.get(ty) {
        Type::Intersection { param, bound } => format!("{} & {}", i.resolve(t.param(*param).name), formatar(t, *bound, i, p)),
        Type::Dynamic => "dynamic".into(),
        Type::Void => "void".into(),
        Type::Never => "Never".into(),
        Type::Null => "Null".into(),
        Type::Interface { class, args, nullable } | Type::ExtensionType { decl: class, args, nullable } => {
            let nome = i.resolve(p.class(*class).name);
            if args.is_empty() {
                format!("{nome}{}", q(*nullable))
            } else {
                let a: Vec<String> = args.iter().map(|&x| formatar(t, x, i, p)).collect();
                format!("{nome}<{}>{}", a.join(", "), q(*nullable))
            }
        }
        Type::FutureOr { arg, nullable } => format!("FutureOr<{}>{}", formatar(t, *arg, i, p), q(*nullable)),
        Type::TypeParameter { param, nullable } => format!("{}{}", i.resolve(t.param(*param).name), q(*nullable)),
        Type::Record { positional, named, nullable } => {
            let mut partes: Vec<String> = positional.iter().map(|&x| formatar(t, x, i, p)).collect();
            let mut n: Vec<(String, String)> = named.iter().map(|(s, x)| (i.resolve(*s).to_string(), formatar(t, *x, i, p))).collect();
            n.sort();
            if !n.is_empty() {
                partes.push(format!("{{{}}}", n.iter().map(|(s, x)| format!("{x} {s}")).collect::<Vec<_>>().join(", ")));
            } else if positional.len() == 1 {
                return format!("({},){}", partes[0], q(*nullable));
            }
            format!("({}){}", partes.join(", "), q(*nullable))
        }
        Type::Function { type_params, ret, positional, optional, named, nullable } => {
            let tps = if type_params.is_empty() {
                String::new()
            } else {
                let v: Vec<String> = type_params
                    .iter()
                    .map(|&tp| {
                        let d = t.param(tp);
                        let nome = i.resolve(d.name).to_string();
                        match t.get(d.bound) {
                            Type::Interface { nullable: true, class, .. } if i.resolve(p.class(*class).name) == "Object" => nome,
                            Type::Dynamic => nome,
                            _ => format!("{nome} extends {}", formatar(t, d.bound, i, p)),
                        }
                    })
                    .collect();
                format!("<{}>", v.join(", "))
            };
            let mut partes: Vec<String> = positional.iter().map(|&x| formatar(t, x, i, p)).collect();
            if !optional.is_empty() {
                partes.push(format!("[{}]", optional.iter().map(|&x| formatar(t, x, i, p)).collect::<Vec<_>>().join(", ")));
            }
            if !named.is_empty() {
                let mut n: Vec<(String, String)> = named
                    .iter()
                    .map(|(s, x, r)| (i.resolve(*s).to_string(), format!("{}{} {}", if *r { "required " } else { "" }, formatar(t, *x, i, p), i.resolve(*s))))
                    .collect();
                n.sort();
                partes.push(format!("{{{}}}", n.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(", ")));
            }
            format!("{} Function{tps}({}){}", formatar(t, *ret, i, p), partes.join(", "), q(*nullable))
        }
    }
}

macro_rules! ou_pula {
    ($e:expr) => {
        match $e {
            Some(r) => r,
            None => {
                eprintln!("SDK 3.6.2 indisponível: pulando");
                return;
            }
        }
    };
}

/// Grupo base: literais, closures (corpo inferido), genéricos de coleção,
/// índices, operadores numéricos, `super`, construtores genéricos.
#[test]
fn base_do_motor_novo() {
    let r = ou_pula!(inferir(
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
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// Tipo cru numa anotação do outline é instanciado para os limites
/// (`Map` → `Map<dynamic, dynamic>`, `C` com `T extends num` → `C<num>`).
#[test]
fn tipo_cru_instanciado_para_os_limites() {
    let r = ou_pula!(inferir(
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

/// Construtores nomeados, `$this`, campos de extensão, `null.hashCode`,
/// tear-off genérico como alvo de chamada (tipo não instanciado).
#[test]
fn construtores_nomeados_e_referencias() {
    let r = ou_pula!(inferir(
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
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// `late final x;` sem inicializador tem setter (atribuição única): a
/// atribuição tipa o valor pelo campo, sem aviso de setter indefinido.
#[test]
fn setter_de_late_final() {
    let r = ou_pula!(inferir(
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
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// Classe do SDK cujo patch vem numa *parte* de um arquivo de patch
/// (`core_patch.dart` → `part 'bigint_patch.dart'`): uma classe só, com os
/// membros da declaração original (`BigInt.operator <<`).
#[test]
fn patch_em_parte_de_patch() {
    let r = ou_pula!(inferir(
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
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// Parâmetro-função da forma antiga com `?` (`int f(int x)?`): o tipo do
/// parâmetro é anulável (`jsonEncode`, `List.sort`, `Stream.listen`).
#[test]
fn parametro_funcao_antigo_anulavel() {
    let r = ou_pula!(inferir(
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

/// Inferência de sobreposição pela posição (o nome do parâmetro pode
/// mudar), extensão genérica usada de dentro dela mesma, `-1` com contexto
/// `double`, e closure que não herda promoção de variável escrita no corpo.
#[test]
fn sobreposicao_extensao_e_closure() {
    let r = ou_pula!(inferir(
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
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// Conflito de import entre `dart:` e um pacote: o do sistema fica oculto
/// (`dart:html` também exporta `NotificationEvent`).
#[test]
fn import_do_sistema_perde_o_conflito() {
    let Some(sdk) = sdk() else { return };
    let _ = sdk;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lib.dart"), "class Duration { final int n; Duration(this.n); }").unwrap();
    let main = dir.path().join("main.dart");
    std::fs::write(&main, "import 'dart:core';
import 'lib.dart';
void main() { var d = Duration(3); var n = d.n; }").unwrap();
    let r = inferir_arquivo(&main);
    assert_eq!(r.tipo("d.n"), "int");
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// Promoção de variável de tipo: interseção `X & B` (`T?` testado contra
/// `null` vira `T & Object`; `is int` vira `T & int`).
#[test]
fn promocao_de_variavel_de_tipo() {
    let r = ou_pula!(inferir(
        r#"
T f<T>(T? a, T b, Map<String, T> m) {
  if (a != null) { var x = a; }
  if (b is int) { var y = b; }
  var z = m['k']!;
  return b;
}
"#
    ));
    let tipos: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "a").map(|(_, ty)| ty.as_str()).collect();
    assert_eq!(tipos, ["T?", "T & Object"]);
    let tb: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "b").map(|(_, ty)| ty.as_str()).collect();
    assert_eq!(tb, ["T", "T & int", "T"]);
    assert_eq!(r.tipo("m['k']"), "T?");
    assert_eq!(r.tipo("m['k']!"), "T & Object");
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// Promoção de campo privado final (Dart 3.2), por `this`, implícito e por
/// local; um homônimo não final na biblioteca a impede.
#[test]
fn promocao_de_campo_privado() {
    let r = ou_pula!(inferir(
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
    let tipos = |t: &str| r.tipos.iter().filter(|(x, _)| x == t).map(|(_, y)| y.as_str()).collect::<Vec<_>>();
    assert_eq!(tipos("_x"), ["int?", "int"]);
    assert_eq!(tipos("this._o"), ["Object", "String"]);
    assert_eq!(tipos("_y"), ["int?", "int?"]);
    assert_eq!(tipos("a._x"), ["int?", "int"]);
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// `?.` promove o receptor só dentro da cadeia (argumentos inclusive);
/// `clamp` com contexto `double`; dentro de função local, variável escrita
/// fora dela (mesmo depois) se promove, e escrita dentro de outra função
/// local não (`conservativeJoin(anywhere.written, anywhere.captured)`;
/// sondado no analyzer 3.6.2 e 3.13.4: só o segundo caso dá
/// `unchecked_use_of_nullable_value`).
#[test]
fn cadeia_clamp_e_funcao_local() {
    let r = ou_pula!(inferir(
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
    let tipos = |t: &str| r.tipos.iter().filter(|(x, _)| x == t).map(|(_, y)| y.as_str()).collect::<Vec<_>>();
    assert_eq!(tipos("s"), ["S?", "S"]);
    assert_eq!(tipos("1"), ["double"]);
    assert_eq!(tipos("buf")[..2], ["StringBuffer?", "StringBuffer"]);
    // `cap = null` (escrita), `cap != null`, `cap.write`: capturada, não promove.
    assert_eq!(tipos("cap")[1..3], ["StringBuffer?", "StringBuffer?"]);
}

/// A escrita de uma variável homônima declarada noutra closure (o `main` de
/// um arquivo de teste com vários `test(…)`) não captura a variável desta:
/// as escritas são resolvidas pela declaração, no escopo léxico
/// (`limitless_ui`, `li_datatable_component_test.dart`).
#[test]
fn escrita_homonima_noutra_closure_nao_captura() {
    let r = ou_pula!(inferir(
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
    let tipos = |t: &str| r.tipos.iter().filter(|(x, _)| x == t).map(|(_, y)| y.as_str()).collect::<Vec<_>>();
    // `botao!`, depois `botao.write` dentro da closure: promovido.
    assert_eq!(tipos("botao")[..2], ["StringBuffer?", "StringBuffer"]);
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// Casos dos corpos do SDK compilado da fonte: limite de parâmetro de
/// extensão, typedef que renomeia classe genérica, parâmetros `super.x`
/// (tipo substituído e argumentos implícitos), tipo testado no ramo falso de
/// `is!`, parâmetro de função local que sombreia o de fora, subtipo entre
/// funções genéricas, construtor encaminhado de aplicação de mixin e
/// constantes referidas.
#[test]
fn corpos_do_sdk() {
    let r = ou_pula!(inferir(
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
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}

/// Promoção de campo privado final só a partir da versão de linguagem 3.2
/// (R-FLU-12): em biblioteca `// @dart = 3.1` o `!` é necessário e o campo
/// fica anulável.
#[test]
fn promocao_de_campo_depende_da_versao() {
    let r = ou_pula!(inferir(
        r#"// @dart = 3.1
class A {
  final int? _x;
  A(this._x);
  int f() => _x != null ? _x! : 0;
}
"#
    ));
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
    let tipos: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "_x").map(|(_, y)| y.as_str()).collect();
    assert_eq!(tipos, ["int?", "int?"]);
}

/// Variável de condição (§7.10): `final bool v = d != null; if (v) d` promove
/// `d`; escrever `d` depois invalida.
#[test]
fn variavel_de_condicao() {
    let r = ou_pula!(inferir(
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
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
    let tipos: Vec<&str> = r.tipos.iter().filter(|(t, _)| t == "e").map(|(_, y)| y.as_str()).collect();
    // Escrita depois da condição: a variável não restaura a promoção.
    assert_eq!(tipos.last(), Some(&"double?"));
}

/// Extension type que implementa outro extension type (`Element implements
/// JSObject`, `JSString implements JSAny` no package:web).
#[test]
fn extension_type_implementa_extension_type() {
    let r = ou_pula!(inferir(
        r#"
extension type A._(Object _) {}
extension type B._(Object _) implements A {}
extension type C._(Object _) implements B { C(B b) : _ = b; }
A f(C c) => c;
"#
    ));
    assert!(r.avisos.is_empty(), "avisos: {:?}", r.avisos);
}
