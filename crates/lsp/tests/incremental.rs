//! "Incremental = do zero" (docs/LSP-ESPECIFICACAO.md §16.8), no modo curto:
//! uma sequência de edições aplicada a um servidor com o caminho incremental
//! (a troca de unidade no `didChange` e o completar especulativo, §16.10 I1
//! a I4) e a outro sem ele; depois de cada passo, as respostas nas posições
//! amostradas são iguais. As edições de corpo não derrubam a sessão; as de
//! assinatura e de diretivas derrubam.

mod comum;

use comum::Projeto;
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{AnalisadorSemantico, Servidor};
use serde_json::{Value, json};

/// Um projeto cujo servidor não usa o caminho incremental.
fn sem_incremental(nome: &str) -> Projeto {
    let mut p = Projeto::novo(nome);
    let sdk = SdkLayout::load(&p.raiz.join("sdk/lib"), "dartdevc").unwrap();
    p.servidor = Servidor::com_analisador(AnalisadorSemantico::novo(Some(sdk)).sem_incremental());
    let raiz = p.uri("");
    p.requisitar("initialize", json!({"rootUri": raiz, "capabilities": {}}));
    p
}

/// (linha, coluna) da `n`-ésima ocorrência (0: a primeira) de `agulha` em
/// `texto`, mais `delta` colunas (ASCII).
fn posicao(texto: &str, agulha: &str, n: usize, delta: u32) -> (u32, u32) {
    let i = texto.match_indices(agulha).nth(n).unwrap_or_else(|| panic!("sem {agulha}")).0;
    let antes = &texto[..i];
    let linha = antes.matches('\n').count() as u32;
    let coluna = antes.rsplit('\n').next().unwrap().len() as u32;
    (linha, coluna + delta)
}

/// As respostas comparadas numa posição.
fn respostas(p: &mut Projeto, l: u32, c: u32) -> Vec<Value> {
    let mut v = Vec::new();
    for metodo in ["textDocument/hover", "textDocument/definition", "textDocument/documentHighlight", "textDocument/completion"] {
        v.push(p.na_posicao(metodo, "lib/a.dart", l, c, json!({}))["result"].clone());
    }
    v
}

const V0: &str = "class A<T> {
  T valor;
  A(this.valor);
  int dobro(int x) => x * 2;
}

int contar(List<int> xs) {
  var total = 0;
  for (final x in xs) {
    total += x;
  }
  return total;
}

class B extends A<int> {
  B() : super(0);
  String nome = '';
}

void main() {
  var b = B();
  print(b.valor);
}
";

/// Corpo de `contar` com tipos novos antes de `B` (desloca os `TypeId`
/// das cláusulas de `B`).
const V1: &str = "class A<T> {
  T valor;
  A(this.valor);
  int dobro(int x) => x * 2;
}

int contar(List<int> xs) {
  var total = 0;
  var lista = <String>[];
  Map<int, String> m = {};
  for (final x in xs) {
    total += x + lista.length + m.length;
  }
  return total;
}

class B extends A<int> {
  B() : super(0);
  String nome = '';
}

void main() {
  var b = B();
  print(b.valor);
}
";

/// Corpo de `main`.
const V2: &str = "class A<T> {
  T valor;
  A(this.valor);
  int dobro(int x) => x * 2;
}

int contar(List<int> xs) {
  var total = 0;
  var lista = <String>[];
  Map<int, String> m = {};
  for (final x in xs) {
    total += x + lista.length + m.length;
  }
  return total;
}

class B extends A<int> {
  B() : super(0);
  String nome = '';
}

void main() {
  var b = B();
  print(b.valor + b.dobro(contar([1, 2])));
  b.
}
";

/// Assinatura de `contar`.
const V3: &str = "class A<T> {
  T valor;
  A(this.valor);
  int dobro(int x) => x * 2;
}

int contar(List<int> xs, [int inicio = 0]) {
  var total = inicio;
  for (final x in xs) {
    total += x;
  }
  return total;
}

class B extends A<int> {
  B() : super(0);
  String nome = '';
}

void main() {
  var b = B();
  print(b.valor + b.dobro(contar([1, 2])));
  b.
}
";

/// As posições amostradas de um texto.
fn amostras(texto: &str) -> Vec<(u32, u32)> {
    let mut v = vec![posicao(texto, "b.valor", 0, 2), posicao(texto, "B()", 1, 0), posicao(texto, "total", 1, 0), posicao(texto, "xs", 1, 1)];
    if texto.contains("  b.\n") {
        v.push(posicao(texto, "  b.\n", 0, 4));
    }
    if texto.contains("lista.length") {
        v.push(posicao(texto, "lista.length", 0, 6));
    }
    v
}

#[test]
fn edicoes_de_corpo_e_de_assinatura_dao_o_mesmo_que_do_zero() {
    let mut quente = Projeto::novo("incremental-quente");
    let mut frio = sem_incremental("incremental-frio");
    quente.abrir("lib/a.dart", V0);
    frio.abrir("lib/a.dart", V0);
    for (l, c) in amostras(V0) {
        assert_eq!(respostas(&mut quente, l, c), respostas(&mut frio, l, c), "V0 em {l}:{c}");
    }
    let mut versao = 1;
    let mut trocas = 0;
    for (texto, corpo) in [(V1, true), (V2, true), (V3, false), (V2, false), (V1, true), (V0, true)] {
        versao += 1;
        let antes = quente.servidor.analisador().estatisticas_da_sessao();
        quente.mudar("lib/a.dart", versao, texto);
        frio.mudar("lib/a.dart", versao, texto);
        let depois = quente.servidor.analisador().estatisticas_da_sessao();
        if corpo {
            trocas += 1;
            assert_eq!((depois.trocas_de_corpo, depois.invalidadas), (trocas, antes.invalidadas), "{depois:?}");
        } else {
            assert_eq!(depois.trocas_de_corpo, trocas, "{depois:?}");
        }
        for (l, c) in amostras(texto) {
            let r = respostas(&mut quente, l, c);
            assert_eq!(r, respostas(&mut frio, l, c), "versão {versao} em {l}:{c}");
            // O completar especulativo desfaz a troca: o estado retido
            // responde igual depois dele.
            assert_eq!(respostas(&mut quente, l, c), r, "repetido, versão {versao} em {l}:{c}");
        }
    }
    // Edições de corpo sem recarga: a sessão quente carregou menos.
    let q = quente.servidor.analisador().estatisticas_da_sessao();
    let f = frio.servidor.analisador().estatisticas_da_sessao();
    assert!(q.carregadas < f.carregadas, "{q:?} {f:?}");
}

#[test]
fn diretivas_e_forma_derrubam_a_sessao() {
    let mut p = Projeto::novo("incremental-diretivas");
    p.abrir("lib/a.dart", V0);
    respostas(&mut p, 1, 4);
    let e = p.servidor.analisador().estatisticas_da_sessao();
    // Uma diretiva nova.
    p.mudar("lib/a.dart", 2, &format!("import 'dart:math';\n{V0}"));
    let d = p.servidor.analisador().estatisticas_da_sessao();
    assert_eq!((d.invalidadas, d.trocas_de_corpo), (e.invalidadas + 1, 0), "{d:?}");
    respostas(&mut p, 2, 4);
    // Uma declaração nova (forma).
    p.mudar("lib/a.dart", 3, &format!("import 'dart:math';\n{V0}int z() => 0;\n"));
    let f = p.servidor.analisador().estatisticas_da_sessao();
    assert_eq!((f.invalidadas, f.trocas_de_corpo), (d.invalidadas + 1, 0), "{f:?}");
    // Um corpo que vaza (chave aberta): recusa ou troca, nunca resposta
    // diferente da do zero.
    respostas(&mut p, 2, 4);
    let vaza = format!("import 'dart:math';\n{}int z() => 0;\n", V0.replace("return total;", "return total; {"));
    p.mudar("lib/a.dart", 4, &vaza);
    let mut frio = sem_incremental("incremental-vaza");
    frio.abrir("lib/a.dart", &vaza);
    for (l, c) in [(2, 4), (21, 9)] {
        assert_eq!(respostas(&mut p, l, c), respostas(&mut frio, l, c), "{l}:{c}");
    }
}
