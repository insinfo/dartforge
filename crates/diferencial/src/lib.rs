//! Harness diferencial: `dart run` (semântica) × `dartdevc`+Node (contrato) × DartForge.
//!
//! Corpus em `corpus/js/`: um programa por arquivo (`NN_tema.dart`) ou por diretório
//! com `main.dart`. Ver `docs/EMISSAO-DDC.md` e `docs/CONTRATO-DDC.md`.

pub mod contrato;
pub mod corpus;
pub mod oraculos;
pub mod processo;
pub mod relatorio;

pub use corpus::{Programa, listar};
pub use oraculos::{Ambiente, dartforge, dartforge_nativo, dartforge_producao, oraculo_dart, oraculo_ddc};
pub use processo::Saida;
pub use relatorio::{Divergencia, Resultado, comparar, relatorio};

use std::sync::{Arc, Mutex};

/// O que executar para cada programa.
#[derive(Debug, Clone, Copy)]
pub struct Opcoes {
    /// Executar também o DartForge (senão só os dois oráculos).
    pub com_forge: bool,
    /// Número de threads; 0 = núcleos disponíveis.
    pub threads: usize,
    /// Executar no modo nativo AOT (comparando contra o oráculo Dart VM).
    pub nativo: bool,
    /// Executar também o **perfil de produção** do backend JavaScript
    /// (`dartforge-jsprod`): arquivo único, `dart_sdk.js` podado pelo mundo
    /// fechado. Ver `docs/JS-PRODUCAO.md`. Independente de `nativo`: os dois
    /// executores são backends diferentes do mesmo programa.
    pub com_producao: bool,
}

impl Default for Opcoes {
    fn default() -> Self {
        Opcoes { com_forge: true, threads: 0, nativo: false, com_producao: false }
    }
}

/// Executa um programa nos executores pedidos.
pub fn executar_programa(amb: &Ambiente, programa: &Programa, op: Opcoes) -> Resultado {
    let dart = oraculo_dart(amb, programa);
    // O perfil de produção do JS é independente do backend escolhido: é um
    // quarto executor do mesmo programa, e pode rodar ao lado do nativo.
    let producao = op.com_producao.then(|| dartforge_producao(amb, programa, &amb.dir_saida("producao", programa)));
    if op.nativo {
        // No modo nativo não há contrato do DDC para comparar; a referência é
        // sempre a VM (ver `Resultado::referencia`).
        let ddc = Saida { stdout: String::new(), stderr: String::new(), codigo: 0 };
        let forge = op.com_forge.then(|| dartforge_nativo(amb, programa, &amb.dir_saida("nativo", programa)));
        Resultado { programa: programa.clone(), dart, ddc, forge, nativo: true, producao }
    } else {
        let ddc = oraculo_ddc(amb, programa, &amb.dir_saida("ddc", programa));
        let forge = op.com_forge.then(|| dartforge(amb, programa, &amb.dir_saida("forge", programa)));
        Resultado { programa: programa.clone(), dart, ddc, forge, nativo: false, producao }
    }
}

/// Executa o corpus inteiro com `std::thread` (fila compartilhada), preservando a ordem.
/// `progresso` é chamado no fim de cada programa (para imprimir andamento).
pub fn executar_corpus(amb: &Ambiente, programas: &[Programa], op: Opcoes, progresso: impl Fn(&Resultado) + Sync) -> Vec<Resultado> {
    let n = if op.threads == 0 { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) } else { op.threads };
    let n = n.max(1).min(programas.len().max(1));
    let proximo = Arc::new(Mutex::new(0usize));
    let resultados: Arc<Mutex<Vec<Option<Resultado>>>> = Arc::new(Mutex::new((0..programas.len()).map(|_| None).collect()));
    std::thread::scope(|s| {
        for _ in 0..n {
            let proximo = Arc::clone(&proximo);
            let resultados = Arc::clone(&resultados);
            let progresso = &progresso;
            s.spawn(move || {
                loop {
                    let i = {
                        let mut g = proximo.lock().unwrap();
                        let i = *g;
                        *g += 1;
                        i
                    };
                    if i >= programas.len() {
                        break;
                    }
                    let r = executar_programa(amb, &programas[i], op);
                    progresso(&r);
                    resultados.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    Arc::try_unwrap(resultados).unwrap().into_inner().unwrap().into_iter().map(|r| r.expect("todo programa executado")).collect()
}
