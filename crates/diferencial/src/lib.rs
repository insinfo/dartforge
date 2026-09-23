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
pub use oraculos::{Ambiente, dartforge, dartforge_nativo, dartforge_nativo_ir, dartforge_producao, oraculo_dart, oraculo_ddc};
pub use processo::Saida;
pub use relatorio::{Divergencia, IrPrograma, Resultado, comparar, diferencas_ir, relatorio, relatorio_ir};

use dartforge_emit_native::resumo::ResumoIr;
use std::sync::{Condvar, Mutex};

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

/// Aplica `f` a cada programa com `threads` trabalhadores (0 = núcleos
/// disponíveis) puxando de uma fila compartilhada, e devolve os resultados na
/// **ordem do corpus**, qualquer que tenha sido a ordem de conclusão.
/// `progresso` é chamado no fim de cada programa (para imprimir andamento).
pub fn em_paralelo<T: Send>(
    programas: &[Programa],
    threads: usize,
    f: impl Fn(&Programa) -> T + Sync,
    progresso: impl Fn(&T) + Sync,
) -> Vec<T> {
    let n = if threads == 0 { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) } else { threads };
    let n = n.max(1).min(programas.len().max(1));
    let proximo = Mutex::new(0usize);
    let resultados: Mutex<Vec<Option<T>>> = Mutex::new((0..programas.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| {
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
                    let r = f(&programas[i]);
                    progresso(&r);
                    resultados.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    resultados.into_inner().unwrap().into_iter().map(|r| r.expect("todo programa executado")).collect()
}

/// Executa o corpus inteiro com `std::thread` (fila compartilhada), preservando a ordem.
/// `progresso` é chamado no fim de cada programa (para imprimir andamento).
pub fn executar_corpus(amb: &Ambiente, programas: &[Programa], op: Opcoes, progresso: impl Fn(&Resultado) + Sync) -> Vec<Resultado> {
    em_paralelo(programas, op.threads, |p| executar_programa(amb, p, op), progresso)
}

/// Emissões simultâneas quando `DARTFORGE_IR_PARALELO_MAX` não diz outra coisa.
///
/// Cada emissão carrega o SDK, analisa e baixa o programa dentro do processo
/// do harness. Hoje isso é barato (medido em `ESTADO.md` §3.2: o corpus
/// inteiro em menos de meio segundo, pico de 10 MB com oito ao mesmo tempo),
/// porque a seção `vm` do `libraries.json` só declara `dart:cli` e o
/// `include` de `vm_common` ainda não é seguido. Quando for, cada emissão
/// passa a analisar o SDK inteiro — no JS isso custa centenas de MB (§1.3) —,
/// e oito ao mesmo tempo não cabem na máquina de 7,7 GB compartilhada. O
/// teste de determinismo não precisa de oito emissões *físicas*: precisa de
/// oito trabalhadores disputando a fila, para a ordem de conclusão mudar — e
/// ela continua mudando com o limite, porque quem pega a vaga seguinte é
/// decidido pelo escalonador.
pub const EMISSOES_SIMULTANEAS_PADRAO: usize = 2;

/// Semáforo contado: no máximo `max` trechos protegidos rodando ao mesmo tempo,
/// qualquer que seja o número de trabalhadores.
#[derive(Debug)]
pub struct Limitador {
    max: usize,
    livres: Mutex<usize>,
    vaga: Condvar,
}

impl Limitador {
    pub fn novo(max: usize) -> Limitador {
        let max = max.max(1);
        Limitador { max, livres: Mutex::new(max), vaga: Condvar::new() }
    }

    /// `DARTFORGE_IR_PARALELO_MAX` quando é um número positivo, senão
    /// [`EMISSOES_SIMULTANEAS_PADRAO`].
    pub fn do_ambiente() -> Limitador {
        let max = std::env::var("DARTFORGE_IR_PARALELO_MAX")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(EMISSOES_SIMULTANEAS_PADRAO);
        Limitador::novo(max)
    }

    pub fn max(&self) -> usize {
        self.max
    }

    /// Executa `f` ocupando uma vaga; espera se não houver. A vaga volta mesmo
    /// que `f` entre em pânico.
    pub fn com<T>(&self, f: impl FnOnce() -> T) -> T {
        struct Vaga<'a>(&'a Limitador);
        impl Drop for Vaga<'_> {
            fn drop(&mut self) {
                *self.0.livres.lock().unwrap_or_else(|e| e.into_inner()) += 1;
                self.0.vaga.notify_one();
            }
        }
        {
            let mut livres = self.livres.lock().unwrap();
            while *livres == 0 {
                livres = self.vaga.wait(livres).unwrap();
            }
            *livres -= 1;
        }
        let _vaga = Vaga(self);
        f()
    }
}

/// Emite o LLVM IR de cada programa (sem Clang, ligação nem execução) e
/// guarda só o resumo: o texto é descartado ainda dentro da vaga do
/// `limite`, para a memória dele não se somar à da emissão seguinte.
pub fn emitir_ir_corpus(programas: &[Programa], threads: usize, limite: &Limitador) -> Vec<IrPrograma> {
    em_paralelo(
        programas,
        threads,
        |p| IrPrograma {
            nome: p.nome.clone(),
            resultado: limite.com(|| dartforge_nativo_ir(p).map(|texto| ResumoIr::de(&texto))),
        },
        |_| {},
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn programas(n: usize) -> Vec<Programa> {
        (0..n).map(|i| Programa { nome: format!("p{i}"), entrada: "x.dart".into(), arquivos: vec![], diverge_ddc: None }).collect()
    }

    #[test]
    fn em_paralelo_preserva_a_ordem() {
        let ps = programas(24);
        let esperado: Vec<String> = ps.iter().map(|p| p.nome.clone()).collect();
        for threads in [1, 4, 8] {
            let chamados = AtomicUsize::new(0);
            // Os primeiros demoram mais: com vários trabalhadores, a ordem de
            // conclusão fica diferente da ordem do corpus.
            let r = em_paralelo(
                &ps,
                threads,
                |p| {
                    let i: u64 = p.nome[1..].parse().unwrap();
                    std::thread::sleep(Duration::from_millis(24 - i));
                    p.nome.clone()
                },
                |_| {
                    chamados.fetch_add(1, Ordering::SeqCst);
                },
            );
            assert_eq!(r, esperado, "{threads} trabalhadores");
            assert_eq!(chamados.load(Ordering::SeqCst), ps.len());
        }
    }

    #[test]
    fn limitador_respeita_o_maximo() {
        let limite = Limitador::novo(2);
        let dentro = AtomicUsize::new(0);
        let pico = AtomicUsize::new(0);
        em_paralelo(
            &programas(16),
            8,
            |_| {
                limite.com(|| {
                    let agora = dentro.fetch_add(1, Ordering::SeqCst) + 1;
                    pico.fetch_max(agora, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(5));
                    dentro.fetch_sub(1, Ordering::SeqCst);
                })
            },
            |_| {},
        );
        let pico = pico.load(Ordering::SeqCst);
        assert!((1..=2).contains(&pico), "pico de {pico} dentro do limitador de 2");
    }
}
