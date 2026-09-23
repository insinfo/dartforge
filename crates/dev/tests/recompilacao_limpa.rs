//! Invariante da sessão residente: a saída de uma recompilação incremental é
//! **byte a byte** igual à de uma compilação limpa do mesmo fonte final.
//!
//! A sessão guarda o `Interner` entre edições, então os nomes que as edições
//! trazem são internados na ordem em que chegam — não na ordem da fonte, como
//! numa compilação do zero. Todo percurso ordenado por `SymbolId` (ou por
//! outro id que dependa da ordem de chegada) que alcance o texto emitido faz as
//! duas saídas divergirem. As edições abaixo internam os nomes dos
//! construtores na ordem **inversa** da declaração final (`gama`, depois
//! `beta`, depois `alfa`), que é o que expunha a iteração de
//! `ClassElement::constructors` (um `BTreeMap<SymbolId, _>`) nos tearoffs e
//! nos construtores encaminhadores das aplicações de mixin. Nomeados de tipos
//! de função e de records, membros privados e encaminhadores de
//! `noSuchMethod` também chegam com os nomes internados em ordem trocada.

use dartforge_dev::Sessao;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// SDK simulado: `dart:core` com o mínimo para tipar o projeto do teste.
fn sdk_simulado(dir: &Path) -> PathBuf {
    let lib = dir.join("sdk/lib");
    std::fs::create_dir_all(lib.join("core")).unwrap();
    std::fs::write(
        lib.join("libraries.json"),
        r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
    )
    .unwrap();
    std::fs::write(
        lib.join("core/core.dart"),
        r#"
        library dart.core;
        class Object { const Object(); String toString() => ""; }
        class num extends Object {}
        class int extends num { int operator +(int o) => this; }
        class double extends num {}
        class String extends Object { int get length => 0; }
        class bool extends Object {}
        class Null extends Object {}
        class Function extends Object {}
        class Record extends Object {}
        class Iterable<E> extends Object {}
        class List<E> extends Object implements Iterable<E> {}
        class Map<K, V> extends Object {}
        class Set<E> extends Object implements Iterable<E> {}
        class Type extends Object {}
        class Symbol extends Object {}
        class Enum extends Object {}
        class StackTrace extends Object {}
        class Invocation extends Object {}
        class Comparable<T> extends Object {}
        void print(Object? o) {}
        "#,
    )
    .unwrap();
    lib
}

/// As três revisões do projeto. A final declara `alfa`, `beta`, `gama` nessa
/// ordem; as anteriores trazem os nomes na ordem inversa.
const APOIO: [&str; 3] = [
    "int usa() => 1;\nvoid nomes({int bravo = 0, int alfa = 0}) {}\nint _zulu = 0;\nint _yankee = 0;\n",
    "int omega() => 3;\nint usa() => omega();\n",
    "int delta() => 4;\nint omega() => 3;\nint usa() => omega() + delta();\n",
];

const MAIN: [&str; 3] = [
    r#"import 'apoio.dart';
mixin Marca {}
class Base { final int v; Base.gama(this.v); }
class Filho extends Base with Marca { Filho.gama(int v) : super.gama(v); }
class Ponto { final int x; Ponto.gama(this.x); }
void main() {
  print(Ponto.gama(1).x);
  print(Filho.gama(2).v);
  print(usa());
}
"#,
    r#"import 'apoio.dart';
mixin Marca {}
class Base { final int v; Base.beta(this.v); Base.gama(this.v); }
class Filho extends Base with Marca { Filho.beta(int v) : super.beta(v); Filho.gama(int v) : super.gama(v); }
class Ponto { final int x; Ponto.beta(this.x); Ponto.gama(this.x); }
void main() {
  print(Ponto.beta(1).x);
  print(Filho.gama(2).v);
  print(usa());
}
"#,
    r#"import 'apoio.dart';
mixin Marca {}
class Base { final int v; Base.alfa(this.v); Base.beta(this.v); Base.gama(this.v); }
class Filho extends Base with Marca {
  Filho.alfa(int v) : super.alfa(v);
  Filho.beta(int v) : super.beta(v);
  Filho.gama(int v) : super.gama(v);
}
class Ponto { final int x; Ponto.alfa(this.x); Ponto.beta(this.x); Ponto.gama(this.x); }
typedef Assinatura = int Function({int alfa, int bravo});
abstract class Servico { int chama({int alfa, int bravo}); int get yankee; }
class Falso implements Servico {
  int _yankee = 1;
  int _zulu = 2;
  @override
  dynamic noSuchMethod(Invocation i) => _yankee + _zulu;
}
({int alfa, int bravo}) par() => (alfa: 1, bravo: 2);
void main() {
  Assinatura? g;
  print(g);
  print(par());
  final Servico s = Falso();
  print(s.chama(bravo: 1));
  print(s.yankee);
  final f = Ponto.alfa;
  print(f(1).x);
  print(Ponto.beta(2).x);
  print(Filho.gama(3).v);
  print(usa());
}
"#,
];

/// Todos os arquivos de um diretório de saída (caminho relativo → bytes).
fn arquivos(raiz: &Path) -> BTreeMap<String, Vec<u8>> {
    fn juntar(raiz: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                juntar(raiz, &p, out);
            } else {
                let rel = p.strip_prefix(raiz).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    juntar(raiz, raiz, &mut out);
    out
}

#[test]
fn recompilacao_incremental_e_igual_a_compilacao_limpa() {
    let tmp = tempfile::tempdir().unwrap();
    let sdk = sdk_simulado(tmp.path());
    let proj = tmp.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let apoio = proj.join("apoio.dart");
    let main = proj.join("main.dart");

    // Sessão longa: três revisões, cada nome novo internado quando chega.
    let saida_sessao = tmp.path().join("js_sessao");
    std::fs::write(&apoio, APOIO[0]).unwrap();
    std::fs::write(&main, MAIN[0]).unwrap();
    let mut sessao = Sessao::nova(&main, Some(&sdk), None, &saida_sessao).expect("sessão");
    sessao.compilar().expect("primeira compilação");
    for i in 1..3 {
        std::fs::write(&apoio, APOIO[i]).unwrap();
        std::fs::write(&main, MAIN[i]).unwrap();
        sessao.arquivo_mudou(&apoio);
        sessao.arquivo_mudou(&main);
        let r = sessao.compilar().expect("recompilação");
        assert!(!r.primeira);
    }

    // Compilação limpa do mesmo fonte final, com arena de nomes nova.
    let saida_limpa = tmp.path().join("js_limpa");
    let mut limpa = Sessao::nova(&main, Some(&sdk), None, &saida_limpa).expect("sessão limpa");
    limpa.compilar().expect("compilação limpa");

    let a = arquivos(&saida_sessao);
    let b = arquivos(&saida_limpa);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>(), "conjuntos de arquivos diferentes");
    for (caminho, limpo) in &b {
        let incremental = &a[caminho];
        if incremental != limpo {
            let (ti, tl) = (String::from_utf8_lossy(incremental), String::from_utf8_lossy(limpo));
            let linha = ti.lines().zip(tl.lines()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "{caminho}: a recompilação incremental diverge da compilação limpa na linha {}:\n  incremental: {}\n  limpa:       {}",
                linha + 1,
                ti.lines().nth(linha).unwrap_or("<fim>"),
                tl.lines().nth(linha).unwrap_or("<fim>")
            );
        }
    }

    // O teste só vale se o texto final tem os construtores na ordem da fonte.
    let modulo = b.iter().find(|(c, _)| c.ends_with("main.js")).map(|(_, t)| String::from_utf8_lossy(t).into_owned()).expect("módulo main");
    let pos = |s: &str| modulo.find(s).unwrap_or_else(|| panic!("`{s}` ausente do módulo"));
    assert!(pos("_#alfa#tearOff") < pos("_#beta#tearOff") && pos("_#beta#tearOff") < pos("_#gama#tearOff"), "tearoffs fora da ordem de declaração");
}
