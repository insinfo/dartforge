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
pub use oraculos::{Ambiente, dartforge, oraculo_dart, oraculo_ddc};
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
}

impl Default for Opcoes {
    fn default() -> Self {
        Opcoes { com_forge: true, threads: 0 }
    }
}

/// Executa um programa nos executores pedidos.
pub fn executar_programa(amb: &Ambiente, programa: &Programa, op: Opcoes) -> Resultado {
    let dart = oraculo_dart(amb, programa);
    let ddc = oraculo_ddc(amb, programa, &amb.dir_saida("ddc", programa));
    let forge = op.com_forge.then(|| dartforge(amb, programa, &amb.dir_saida("forge", programa)));
    Resultado { programa: programa.clone(), dart, ddc, forge }
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
