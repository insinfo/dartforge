//! Portão estrutural da regra de custo zero (PLANO.md, "geração de código e
//! macros", item 1): um programa sem classe `macro` não abre sessão de
//! macros, não fala com executor nenhum e sai do hospedeiro exatamente como
//! entrou — como o `dartforge_build::instancias()` para o motor de build.
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_macros_host::executor::{Disponibilidade, ExecutorMacros, PedidoDeExecucao, ServicoDeConsultas};
use dartforge_macros_host::montagem::Resultado;
use dartforge_macros_host::protocolo::Apresentacao;
use serde_json::Value;
use std::path::Path;

/// Um executor que reprova o teste se for tocado.
struct Proibido;

impl ExecutorMacros for Proibido {
    fn disponibilidade(&self) -> Disponibilidade {
        panic!("custo zero: o executor não pode ser consultado")
    }
    fn iniciar(&mut self) -> Result<Apresentacao, String> {
        panic!("custo zero: o executor não pode ser iniciado")
    }
    fn instanciar(&mut self, _: &str, _: &str, _: &Value) -> Result<(u64, Vec<String>), String> {
        panic!("custo zero")
    }
    fn executar(&mut self, _: &PedidoDeExecucao, _: &mut dyn ServicoDeConsultas) -> Result<Resultado, String> {
        panic!("custo zero")
    }
    fn encerrar(&mut self) {
        panic!("custo zero")
    }
}

#[test]
fn sem_macro_nao_abre_sessao() {
    let Some(lib) = SdkLayout::discover() else {
        eprintln!("sem SDK: pulado");
        return;
    };
    let sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
    let entrada = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/js/01_print.dart");
    let mut nomes = Interner::new();
    let (p, d) = load_lenient(&entrada, &sdk, None, &mut nomes);
    assert!(d.is_empty(), "{d:?}");
    assert!(!dartforge_macros_host::tem_macros(&p), "o 01_print não declara classe macro");
    let bibliotecas = p.libraries.len();
    let mut recargas = 0usize;
    let mut carregar = |_: &mut Interner, _| -> (dartforge_elements::model::Program, Vec<Diagnostic>) {
        recargas += 1;
        panic!("custo zero: nenhuma recarga")
    };
    let saida = dartforge_macros_host::aplicar(p, &mut nomes, None, &mut carregar, &mut Proibido)
        .unwrap_or_else(|_| panic!("sem macro não há erro"));
    assert_eq!(saida.program.libraries.len(), bibliotecas);
    assert!(saida.textos.is_empty() && saida.geracao.is_none() && saida.macros_executadas == 0);
    assert_eq!(dartforge_macros_host::sessoes(), 0);
    assert_eq!(recargas, 0);
}
