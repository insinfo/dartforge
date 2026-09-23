//! O mesmo programa, emitido várias vezes — em sequência e em threads —, tem
//! de sair **byte a byte** igual. Os programas escolhidos têm encaminhadores
//! de `noSuchMethod`, cuja ordem já saiu da iteração de um `HashMap`
//! (`Ctx::unimplemented_abstract`) e mudava de uma execução para outra do
//! mesmo binário. Cada `HashMap` novo sorteia as suas chaves (`RandomState`),
//! e cada thread tem a sua semente: repetir e espalhar em threads é o que
//! expõe uma iteração sem ordem que chegue ao texto.

use std::path::{Path, PathBuf};

fn programas() -> Vec<PathBuf> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut v = vec![raiz.join("tests/programas/p8_nosuchmethod_ordem.dart")];
    let corpus = raiz.join("../../corpus/js/57_nosuchmethod.dart");
    if corpus.is_file() {
        v.push(corpus);
    }
    v
}

fn emitir(p: &Path) -> Vec<(String, String)> {
    let mut m = dartforge_emit_js::compilar(p, None, None).expect("compilar").modulos;
    m.sort();
    m
}

const SEQUENCIAIS: usize = 6;
const THREADS: usize = 4;

#[test]
#[ignore = "compila programas reais contra o SDK; rodar com --ignored"]
fn emissao_com_nosuchmethod_e_identica_byte_a_byte() {
    for p in programas() {
        // A primeira compilação também prepara o artefato do SDK, antes que as
        // threads o disputem.
        let referencia = emitir(&p);
        for i in 1..SEQUENCIAIS {
            assert!(emitir(&p) == referencia, "{}: execução {i} diverge da primeira", p.display());
        }
        let resultados: Vec<Vec<(String, String)>> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..THREADS).map(|_| s.spawn(|| emitir(&p))).collect();
            hs.into_iter().map(|h| h.join().expect("thread")).collect()
        });
        for (i, r) in resultados.iter().enumerate() {
            assert!(*r == referencia, "{}: thread {i} diverge da execução sequencial", p.display());
        }
    }
}

/// A ordem dos encaminhadores é a do CFE (`ClassMembersNodeBuilder.build`):
/// superclasse, mixins na ordem do `with`, interfaces na ordem do
/// `implements`, getter antes do setter de mesmo nome.
#[test]
#[ignore = "compila programas reais contra o SDK; rodar com --ignored"]
fn encaminhadores_saem_na_ordem_do_cfe() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programas/p8_nosuchmethod_ordem.dart");
    let modulos = emitir(&p);
    let texto = &modulos.iter().find(|(c, _)| c.ends_with("p8_nosuchmethod_ordem.js")).expect("módulo do programa").1;
    let classe = &texto[texto.find("class Fantasma").expect("classe Fantasma")..];
    let esperado = [
        "incrementa() {",
        "get dobro()",
        "get historico()",
        "registra(evento) {",
        "get total()",
        "soma(a, b) {",
        "area() {",
        "perimetro() {",
        "get nome()",
        "set nome(v)",
        "get lados()",
        "get cor()",
        "set cor(v)",
        "pinta(tinta, opts) {",
    ];
    let mut ultimo = 0;
    for m in esperado {
        let pos = classe.find(m).unwrap_or_else(|| panic!("encaminhador `{m}` não encontrado"));
        assert!(pos >= ultimo, "encaminhador `{m}` fora da ordem do CFE");
        ultimo = pos;
    }
}
