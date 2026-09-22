//! Platô de memória sob edições sucessivas: o portão da meta sem GC na ferramenta.
//!
//! Cada edição deveria tornar inalcançável o estado da edição anterior. Se
//! `live_bytes` crescer com N, alguma estrutura viva guarda referência ao modelo
//! antigo — cache sem teto, snapshot por versão nunca descartado, ou grafo
//! retendo saídas antigas. Sem este teste, "usa menos memória" é anedota, e
//! nenhum outro item da meta pode ser declarado concluído.
//!
//! O teste aplica N edições que invalidam coisas diferentes — corpo de função,
//! assinatura e import — e afirma que os bytes vivos estabilizam num platô em
//! vez de crescer com N. A sessão retém exatamente um snapshot por construção,
//! então qualquer crescimento monotônico aqui é retenção indevida, não modelo
//! grande.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_compiler::{CompilerSession, Optimization};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Diretório exclusivo do teste, removido ao final.
struct Fixture(PathBuf);

impl Fixture {
    /// Reserva um caminho novo mesmo após uma execução anterior interrompida.
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "dartforge-plato-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
    }
    /// Grava um arquivo no diretório reservado e devolve seu caminho.
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }
}

impl Drop for Fixture {
    /// Remove o diretório exclusivo; este teste não cria links simbólicos.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Corpo com largura fixa: a revisão ocupa sempre dois dígitos para que o
/// snapshot retido tenha o mesmo tamanho e o platô não esconda variação de
/// conteúdo atrás de tolerância.
fn corpo(revisao: usize) -> String {
    format!(
        "int somar(int n) {{ var soma = {:02}; for (var i = 0; i < n; i++) {{ soma += i; }} return soma; }}\n",
        revisao % 100
    )
}

/// N edições sucessivas alternando as três invalidações; devolve os bytes vivos
/// após cada compilação. Nenhuma saída é retida: cada `Compilation` cai no fim
/// da iteração, como cairia entre duas teclas num servidor.
fn editar(fixture: &Fixture, edicoes: usize) -> Vec<usize> {
    let entrada = fixture.write(
        "main.dart",
        "import 'dep.dart';\nvoid main() { print(somar(3)); }\n",
    );
    fixture.write("dep.dart", &corpo(0));
    let mut sessao = CompilerSession::new();
    let mut vivos = Vec::with_capacity(edicoes);
    for revisao in 0..edicoes {
        match revisao % 3 {
            // Corpo: só o valor inicial muda, a interface é a mesma.
            0 => {
                fixture.write("dep.dart", &corpo(revisao));
            }
            // Assinatura: uma função extra aparece e some em revisões alternadas.
            1 => {
                let mut fonte = corpo(revisao);
                if revisao.is_multiple_of(2) {
                    fonte.push_str("int extra(int n) { return n; }\n");
                }
                fixture.write("dep.dart", &fonte);
            }
            // Import: a entrada perde o import em revisões alternadas.
            _ => {
                fixture.write("dep.dart", &corpo(revisao));
                if revisao.is_multiple_of(2) {
                    fixture.write("main.dart", "void main() { print(1); }\n");
                } else {
                    fixture.write(
                        "main.dart",
                        "import 'dep.dart';\nvoid main() { print(somar(3)); }\n",
                    );
                }
            }
        }
        sessao
            .compile_path(&entrada, Optimization::None)
            .expect("revisão do platô deve compilar");
        vivos.push(dartforge_instrument::live_bytes());
    }
    vivos
}

/// `live_bytes` estabiliza: o máximo da segunda metade não supera o máximo do
/// platô de referência além da tolerância.
///
/// A tolerância (8 KiB) cobre a oscilação legítima entre revisões de formas
/// diferentes — uma assinatura extra retida é centenas de bytes — e o ruído do
/// harness. Uma retenção real de um snapshot por edição seria dezenas de KiB
/// por edição vezes dezenas de edições, ordens de grandeza acima.
#[test]
fn edicoes_sucessivas_estabilizam_em_plato() {
    let fixture = Fixture::new();
    let vivos = editar(&fixture, 60);
    assert_eq!(vivos.len(), 60);
    let referencia = vivos[10..20].iter().max().copied().unwrap();
    let depois = vivos[20..].iter().max().copied().unwrap();
    assert!(
        depois <= referencia.saturating_add(8 * 1024),
        "memória cresceu com N: platô {referencia} bytes, cauda {depois} bytes"
    );
}
