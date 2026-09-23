//! Com `filtro: None` o emissor tem de ser **byte a byte** o de sempre — é o
//! que garante que o perfil de desenvolvimento e o `dartforge serve` não
//! sentem a etapa 5 (`docs/JS-PRODUCAO.md` §1.7). O mesmo programa é
//! compilado por três caminhos e os módulos são comparados:
//!
//! 1. `dartforge_emit_js::compilar` (o caminho do `compile-js`);
//! 2. `Analise::emitir(None)`;
//! 3. `Analise::emitir(Some(&TudoVivo))` — o filtro que diz tudo vivo.
//!
//! Se o `DARTFORGE_IDENTIDADE_REF` apontar para um diretório com a saída do
//! `dartforge compile-js` de **antes** da mudança (`<nome>/…js`), o caminho 1
//! também é comparado com ele.

use std::path::{Path, PathBuf};

fn programas() -> Vec<PathBuf> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut v: Vec<PathBuf> = ["p1_basico", "p2_funcoes", "p3_classes", "p3_genericas", "p4_colecoes", "p5_excecoes", "p6_async", "p7_dinamico", "p7_padroes", "p7_bibliotecas/main", "p8_nosuchmethod_ordem"]
        .iter()
        .map(|n| raiz.join(format!("crates/emit_js/tests/programas/{n}.dart")))
        .collect();
    for n in ["51_mixins", "57_nosuchmethod", "90_dynamic_chamadas", "207_factory_redirecionada_tearoff", "209_js_interop/main", "212_rti_de_superclasse_generica"] {
        let p = raiz.join(format!("corpus/js/{n}.dart"));
        if p.is_file() {
            v.push(p);
        }
    }
    v
}

fn modulos(e: &dartforge_emit_js::Emitido) -> Vec<(String, String)> {
    let mut m = e.modulos.clone();
    m.sort();
    m
}

#[test]
#[ignore = "compila programas reais contra o SDK; rodar com --ignored"]
fn filtro_nenhum_e_tudo_vivo_emitem_o_mesmo_texto() {
    let referencia = std::env::var("DARTFORGE_IDENTIDADE_REF").ok().map(PathBuf::from);
    for p in programas() {
        let pacotes = p.parent().map(|d| d.join(".dart_tool/package_config.json")).filter(|x| x.is_file());
        let a = dartforge_emit_js::compilar(&p, None, pacotes.as_deref()).expect("compilar");
        let ((b, c), _) = dartforge_emit_js::compilar_com(&p, None, pacotes.as_deref(), |an| {
            let b = an.emitir(None)?;
            let c = an.emitir(Some(&dartforge_emit_js::filtro::TudoVivo))?;
            Ok((b, c))
        })
        .expect("compilar_com");
        let (ma, mb, mc) = (modulos(&a), modulos(&b), modulos(&c));
        assert_eq!(ma, mb, "{}: `compilar` e `emitir(None)` divergem", p.display());
        assert_eq!(mb, mc, "{}: `emitir(None)` e `emitir(TudoVivo)` divergem", p.display());
        assert_eq!(a.entrada, c.entrada);
        if let Some(r) = &referencia {
            let nome = p.file_stem().unwrap().to_string_lossy().to_string();
            let dir = r.join(&nome);
            if dir.is_dir() {
                for (caminho, texto) in &ma {
                    let antigo = std::fs::read_to_string(dir.join(caminho)).unwrap_or_else(|_| panic!("{}: falta {caminho} na referência", p.display()));
                    assert_eq!(&antigo.replace("\r\n", "\n"), texto, "{}: {caminho} difere da saída de antes", p.display());
                }
            }
        }
    }
}
