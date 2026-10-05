//! Os testes dirigidos das raízes por mapas de pilha
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3, §7.5 e §14.7).
//!
//! Cada caso é um programa de `corpus/nativo/gc_*` (que o diferencial também
//! roda contra a VM) com a saída calculada à mão. Ele é compilado com
//! `--raizes=mapas` e rodado com `--gc-stress`, o veneno com quarentena e a
//! prova de que coletou (`DARTFORGE_GC_STATS=exigir`: sem coleta, ou sem
//! raiz lida de mapa, o executável sai com 70). Depois, com a sabotagem dele,
//! o caso **tem de falhar** — na compilação, na execução ou na saída —; um
//! caso que passa com a sabotagem não prova nada.
//!
//! Também: o build de conferência (`DARTFORGE_RAIZES_CONFERIR=1` com
//! `DARTFORGE_GC_PERCURSO=conferir`, E2.5) e a coleta agendada por semente
//! (`DARTFORGE_GC_AGENDA`).
//!
//! Pesado (uma compilação por caso e por sabotagem): roda com
//! `DARTFORGE_TESTES_MAPAS=1`, nos alvos que têm o modo.
//! Escrito sem compilar nem executar (2026-10-05).
#![cfg(feature = "jit")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Caso {
    arquivo: &'static str,
    esperado: &'static str,
    /// Compilar com `--excecoes=tabelas`.
    tabelas: bool,
    /// Compilar com as raízes na pilha-sombra (`--raizes=sombra`): o caso
    /// cuja sabotagem é do quadro da pilha-sombra (D5).
    sombra: bool,
    /// A sabotagem do emissor (`DARTFORGE_SABOTAGEM` na compilação).
    sabotagem_do_emissor: Option<&'static str>,
    /// A sabotagem do runtime (`DARTFORGE_SABOTAGEM` na execução).
    sabotagem_do_runtime: Option<&'static str>,
}

const CASOS: &[Caso] = &[
    Caso {
        arquivo: "gc_d01_argumento_vivo.dart",
        esperado: "18335 1999000 2000\n",
        tabelas: false, sombra: false,
        sabotagem_do_emissor: Some("sem_uso_ficticio"),
        sabotagem_do_runtime: None,
    },
    // D7: a folha falsa, no mesmo programa.
    Caso {
        arquivo: "gc_d01_argumento_vivo.dart",
        esperado: "18335 1999000 2000\n",
        tabelas: false, sombra: false,
        sabotagem_do_emissor: Some("folha:dartforge_string_concat"),
        sabotagem_do_runtime: None,
    },
    // §14.7: o leitor sem a regra do `nop` (só o Windows x64 a tem).
    Caso {
        arquivo: "gc_d01_argumento_vivo.dart",
        esperado: "18335 1999000 2000\n",
        tabelas: false, sombra: false,
        sabotagem_do_emissor: None,
        sabotagem_do_runtime: if cfg!(windows) { Some("sem_nop") } else { None },
    },
    Caso {
        arquivo: "gc_d02_muitos_vivos.dart",
        esperado: "4320 h-49 2.0\n",
        tabelas: false, sombra: false,
        sabotagem_do_emissor: Some("sem_uso_ficticio"),
        sabotagem_do_runtime: None,
    },
    Caso {
        arquivo: "gc_d03_excecao_profunda.dart",
        esperado: "32255\n",
        tabelas: true, sombra: false,
        sabotagem_do_emissor: Some("pouso_sem_topo"),
        sabotagem_do_runtime: None,
    },
    Caso {
        arquivo: "gc_d04_finally.dart",
        esperado: "336 10 1566\n",
        tabelas: true, sombra: false,
        sabotagem_do_emissor: Some("sem_uso_ficticio"),
        sabotagem_do_runtime: None,
    },
    Caso {
        arquivo: "gc_d06_valores_brutos.dart",
        esperado: "249750.0 999000 3890 1248750.0\n",
        tabelas: false, sombra: false,
        sabotagem_do_emissor: Some("bruto_no_mapa"),
        sabotagem_do_runtime: None,
    },
    Caso { arquivo: "gc_d10_closures_async.dart", esperado: "4050\n", tabelas: false, sombra: false, sabotagem_do_emissor: None, sabotagem_do_runtime: None },
    Caso {
        arquivo: "gc_d11_campos_de_objeto_morto.dart",
        esperado: "26800\n",
        tabelas: false, sombra: false,
        sabotagem_do_emissor: Some("sem_uso_ficticio"),
        sabotagem_do_runtime: None,
    },
    Caso {
        arquivo: "gc_d13_estaticos_e_constantes.dart",
        esperado: "13890\n",
        tabelas: false, sombra: false,
        sabotagem_do_emissor: None,
        sabotagem_do_runtime: None,
    },
    Caso { arquivo: "gc_d14_quadro_residual.dart", esperado: "499500\n", tabelas: false, sombra: false, sabotagem_do_emissor: None, sabotagem_do_runtime: None },
    // D5: o slot lido antes de escrito, na pilha-sombra; com o quadro sujo
    // (4098 em vez de zero), o slot ainda não escrito vira raiz inválida.
    Caso {
        arquivo: "gc_d05_slot_antes_de_escrito.dart",
        esperado: "1729 140\n",
        tabelas: false,
        sombra: true,
        sabotagem_do_emissor: Some("quadro_sujo"),
        sabotagem_do_runtime: None,
    },
    // D5 também nos mapas (o `phi` com a entrada nula, a continuação `async`).
    Caso {
        arquivo: "gc_d05_slot_antes_de_escrito.dart",
        esperado: "1729 140\n",
        tabelas: false,
        sombra: false,
        sabotagem_do_emissor: Some("sem_uso_ficticio"),
        sabotagem_do_runtime: None,
    },
    // D8: o `StackOverflowError` capturado, nos dois modelos de exceção; com
    // a folga de 4 KiB, montar e lançar o erro estoura a pilha de verdade.
    Caso {
        arquivo: "gc_d08_estouro_de_pilha.dart",
        esperado: "3003 0\n",
        tabelas: false,
        sombra: false,
        sabotagem_do_emissor: None,
        sabotagem_do_runtime: Some("folga"),
    },
    Caso {
        arquivo: "gc_d08_estouro_de_pilha.dart",
        esperado: "3003 0\n",
        tabelas: true,
        sombra: false,
        sabotagem_do_emissor: None,
        sabotagem_do_runtime: Some("folga"),
    },
    // D12: os caches estáticos do runtime com handle são raiz.
    Caso {
        arquivo: "gc_d12_cache_do_runtime.dart",
        esperado: "1000 1000 4000\n",
        tabelas: false,
        sombra: false,
        sabotagem_do_emissor: None,
        sabotagem_do_runtime: Some("tipo_sem_raiz"),
    },
    // D9: a exceção que sai de uma função Dart que o runtime chamou; sem as
    // portas, ela atravessa os quadros Rust.
    Caso {
        arquivo: "gc_d09_fronteira_rust.dart",
        esperado: "925\n",
        tabelas: true,
        sombra: false,
        sabotagem_do_emissor: None,
        sabotagem_do_runtime: Some("sem_porta"),
    },
    // D9: `Isolate.exit` dentro de `finally`, com a exceção vinda da porta
    // ainda em curso; a saída do isolado não é capturável.
    Caso {
        arquivo: "gc_d09_isolate_exit_em_finally.dart",
        esperado: "870\n",
        tabelas: true,
        sombra: false,
        sabotagem_do_emissor: None,
        sabotagem_do_runtime: Some("sem_porta"),
    },
    // D9: o callback da FFI que lança, chamado pelo `qsort` da libc (um
    // quadro C) e pelo próprio Dart pelo ponteiro nativo. Sem o pouso da
    // entrada do callback, a exceção atravessa o quadro C e chega ao `main`.
    Caso {
        arquivo: "gc_d09_callback_ffi.dart",
        esperado: "[1, 1, 2, 3, 3, 3, 5, 8, 8, 9] 0 14 2000\n",
        tabelas: true,
        sombra: false,
        sabotagem_do_emissor: Some("callback_sem_pouso"),
        sabotagem_do_runtime: None,
    },
];

fn ligado() -> bool {
    let alvo = (cfg!(windows) && cfg!(target_arch = "x86_64"))
        || ((cfg!(target_os = "linux") || cfg!(target_os = "macos")) && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64")));
    let pedido = std::env::var("DARTFORGE_TESTES_MAPAS").is_ok_and(|v| v == "1");
    if !pedido {
        eprintln!("DARTFORGE_TESTES_MAPAS=1 não definida: testes das raízes por mapas pulados");
    }
    alvo && pedido
}

fn fonte(arquivo: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/nativo").join(arquivo)
}

fn dir_de_trabalho(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dartforge-mapas-{}-{nome}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Compila `arquivo` em modo mapas (ou na pilha-sombra, com `sombra`); o
/// erro leva a mensagem do compilador. Em modo mapas, o conferidor das
/// raízes depois do RS4GC (§7.4) roda em cada módulo; na pilha-sombra, o
/// conferidor de dominância dos slots.
fn compilar(arquivo: &str, tabelas: bool, sombra: bool, ambiente: &[(&str, &str)], saida: &Path) -> Result<(), String> {
    let mut c = Command::new(env!("CARGO_BIN_EXE_dartforge"));
    c.arg("compile-native").arg(fonte(arquivo)).arg("-o").arg(saida);
    c.arg(if sombra { "--raizes=sombra" } else { "--raizes=mapas" });
    if tabelas {
        c.arg("--excecoes=tabelas");
    }
    c.env_remove("DARTFORGE_SABOTAGEM").env_remove("DARTFORGE_RAIZES_CONFERIR");
    if sombra {
        // O conferidor de dominância do modo sombra (§7.4).
        c.env("DARTFORGE_CONFERIR_SOMBRA", "1");
    } else {
        c.env("DARTFORGE_CONFERIR_RS4GC", "1");
    }
    for (k, v) in ambiente {
        c.env(k, v);
    }
    let r = c.output().expect("executar o dartforge");
    if r.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&r.stderr).into_owned()) }
}

/// Roda com o estresse (uma coleta antes de cada alocação) ou sem ele (a
/// agenda decide), o veneno e a prova de que coletou.
fn rodar_com(exe: &Path, estresse: bool, ambiente: &[(&str, &str)]) -> Output {
    let mut c = Command::new(exe);
    c.env_remove("DARTFORGE_GC_STRESS");
    if estresse {
        c.env("DARTFORGE_GC_STRESS", "1");
    }
    c.env("DARTFORGE_GC_STATS", "exigir").env("DARTFORGE_GC_VENENO", "1");
    c.env_remove("DARTFORGE_SABOTAGEM").env_remove("DARTFORGE_GC_PERCURSO").env_remove("DARTFORGE_GC_AGENDA");
    for (k, v) in ambiente {
        c.env(k, v);
    }
    c.output().expect("executar o programa")
}

fn rodar(exe: &Path, ambiente: &[(&str, &str)]) -> Output {
    rodar_com(exe, true, ambiente)
}

/// O número `campo` da linha de estatísticas (`"map_roots":123`).
fn estatistica(stderr: &str, campo: &str) -> Option<u64> {
    let chave = format!("\"{campo}\":");
    let i = stderr.find(&chave)? + chave.len();
    stderr[i..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

fn saida_certa(r: &Output, esperado: &str) -> bool {
    r.status.success() && String::from_utf8_lossy(&r.stdout).replace("\r\n", "\n") == esperado
}

#[test]
fn casos_dirigidos_passam_e_caem_com_a_sabotagem() {
    if !ligado() {
        return;
    }
    let dir = dir_de_trabalho("dirigidos");
    for (k, caso) in CASOS.iter().enumerate() {
        let exe = dir.join(format!("caso{k}{}", std::env::consts::EXE_SUFFIX));
        compilar(caso.arquivo, caso.tabelas, caso.sombra, &[], &exe).unwrap_or_else(|e| panic!("{}: não compilou:\n{e}", caso.arquivo));
        let r = rodar(&exe, &[]);
        let stderr = String::from_utf8_lossy(&r.stderr).into_owned();
        assert!(saida_certa(&r, caso.esperado), "{}: saída errada ({}):\n{}\n{stderr}", caso.arquivo, r.status, String::from_utf8_lossy(&r.stdout));
        // A prova de que coletou e leu raízes de mapa (o `exigir` já sai com
        // 70 sem isso; aqui a conferência é explícita).
        assert!(estatistica(&stderr, "collections").is_some_and(|n| n > 0), "{}: nenhuma coleta:\n{stderr}", caso.arquivo);
        assert!(
            caso.sombra || estatistica(&stderr, "map_roots").is_some_and(|n| n > 0),
            "{}: nenhuma raiz lida de mapa:\n{stderr}",
            caso.arquivo
        );

        if let Some(s) = caso.sabotagem_do_emissor {
            let sab = dir.join(format!("caso{k}-sab{}", std::env::consts::EXE_SUFFIX));
            let caiu = match compilar(caso.arquivo, caso.tabelas, caso.sombra, &[("DARTFORGE_SABOTAGEM", s)], &sab) {
                Err(_) => true,
                Ok(()) => !saida_certa(&rodar(&sab, &[]), caso.esperado),
            };
            assert!(caiu, "{}: passou com a sabotagem `{s}` — o caso não prova o que diz", caso.arquivo);
        }
        if let Some(s) = caso.sabotagem_do_runtime {
            let r = rodar(&exe, &[("DARTFORGE_SABOTAGEM", s)]);
            assert!(!saida_certa(&r, caso.esperado), "{}: passou com a sabotagem `{s}` do runtime", caso.arquivo);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// E2.5: o build de conferência grava, antes de cada ponto de coleta, os
/// vivos dele na pilha-sombra; o runtime confere que o percurso por mapas
/// visitou todos.
#[test]
fn percurso_conferido_contra_a_pilha_sombra() {
    if !ligado() {
        return;
    }
    let dir = dir_de_trabalho("conferencia");
    for caso in CASOS.iter().filter(|c| !c.sombra && c.sabotagem_do_runtime.is_none() && c.sabotagem_do_emissor.is_some()) {
        let exe = dir.join(format!("conf{}", std::env::consts::EXE_SUFFIX));
        compilar(caso.arquivo, caso.tabelas, false, &[("DARTFORGE_RAIZES_CONFERIR", "1")], &exe)
            .unwrap_or_else(|e| panic!("{}: não compilou:\n{e}", caso.arquivo));
        let r = rodar(&exe, &[("DARTFORGE_GC_PERCURSO", "conferir")]);
        let stderr = String::from_utf8_lossy(&r.stderr).into_owned();
        assert!(saida_certa(&r, caso.esperado), "{}: a conferência falhou ({}):\n{stderr}", caso.arquivo, r.status);
        assert!(estatistica(&stderr, "checked_roots").is_some_and(|n| n > 0), "{}: nenhuma raiz conferida:\n{stderr}", caso.arquivo);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// §7.5: a coleta agendada por semente, no lugar do estresse total.
#[test]
fn coleta_agendada_por_semente() {
    if !ligado() {
        return;
    }
    let dir = dir_de_trabalho("agenda");
    let caso = &CASOS[0];
    let exe = dir.join(format!("agenda{}", std::env::consts::EXE_SUFFIX));
    compilar(caso.arquivo, false, false, &[], &exe).unwrap_or_else(|e| panic!("não compilou:\n{e}"));
    for semente in ["1,7", "42,13", "9,3"] {
        let r = rodar_com(&exe, false, &[("DARTFORGE_GC_AGENDA", semente)]);
        assert!(saida_certa(&r, caso.esperado), "agenda {semente}: {}", String::from_utf8_lossy(&r.stderr));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
