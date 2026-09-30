#![cfg(feature = "jit")]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Encerra a CLI e devolve o stderr dela. Com `esperado`, antes espera a
/// linha que o contém (até 60 s): a CLI escreve o diagnóstico da geração
/// logo depois de publicá-la, e o programa, que já roda o código novo, pode
/// escrever a saída nova antes — matar a CLI ao ver a saída perderia o
/// diagnóstico.
fn encerrar(child: &mut Child, esperado: Option<&str>) -> String {
    let stderr = child.stderr.take().expect("stderr");
    let (tx, rx) = mpsc::channel::<String>();
    let leitor = std::thread::spawn(move || {
        for linha in BufReader::new(stderr).lines() {
            let Ok(linha) = linha else { break };
            if tx.send(linha).is_err() {
                break;
            }
        }
    });
    let mut texto = String::new();
    if let Some(esperado) = esperado {
        let prazo = Instant::now() + Duration::from_secs(60);
        while let Ok(linha) = rx.recv_timeout(prazo.saturating_duration_since(Instant::now())) {
            texto.push_str(&linha);
            texto.push('\n');
            if linha.contains(esperado) {
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    for linha in rx {
        texto.push_str(&linha);
        texto.push('\n');
    }
    leitor.join().expect("leitor do stderr");
    texto
}

/// Grava a fixture como um editor grava: conteúdo novo no mesmo arquivo, com
/// o mtime de agora. (`fs::copy` no Windows — `CopyFileW` — preserva o mtime
/// da origem, e duas fixtures do mesmo tamanho, gravadas juntas no checkout,
/// deixariam o arquivo com a mesma assinatura de antes.)
fn editar(origem: &std::path::Path, destino: &std::path::Path) {
    let conteudo = std::fs::read(origem).expect("fixture");
    std::fs::write(destino, conteudo).expect("edição");
}

/// A CLI com a biblioteca do SDK da fonte (o único modo desde o espaço
/// unificado, docs/NATIVO-ESPACO-UNIFICADO.md §4.7).
fn cli_com_sdk() -> Command {
    let dll = dartforge_emit_native::sdk_modulo::dll_do_sdk_da_fonte().expect("DLL do SDK da fonte");
    let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
    command.env("DARTFORGE_SDK_DLL", dll);
    command
}

#[test]
#[ignore = "compila DLL do SDK da fonte; executar no job sdk-fonte do Pesado"]
fn cli_preserva_estatico_apos_editar_o_mesmo_arquivo_dart() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    // Um diretório por teste: eles rodam em paralelo no mesmo processo, e a
    // limpeza de um apagava a fixture do outro.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_estado_v1.dart"), &entrada);

    let mut command = cli_com_sdk();
    command.arg("reload").arg(&entrada).arg("--preservar-estado")
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    // A primeira geração compila o programa inteiro (o perfil `test` do CI
    // é debug): o mesmo prazo do teste ao vivo.
    let first = rx.recv_timeout(Duration::from_secs(60));
    let first_len = rx.recv_timeout(Duration::from_secs(60));
    if first.as_deref() == Ok("1") && first_len.as_deref() == Ok("1") {
        editar(&fixtures.join("reload_estado_v2.dart"), &entrada);
    }
    let second = if first.as_deref() == Ok("1") && first_len.as_deref() == Ok("1") {
        rx.recv_timeout(Duration::from_secs(20))
    } else {
        Err(mpsc::RecvTimeoutError::Disconnected)
    };
    let second_len = if second.as_deref() == Ok("11") {
        rx.recv_timeout(Duration::from_secs(20))
    } else {
        Err(mpsc::RecvTimeoutError::Disconnected)
    };
    let stderr = encerrar(&mut child, Some("geração 2: estado preservado"));
    reader.join().expect("leitor");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    assert_eq!(first.as_deref(), Ok("1"), "{stderr}");
    assert_eq!(first_len.as_deref(), Ok("1"), "{stderr}");
    assert_eq!(second.as_deref(), Ok("11"), "{stderr}");
    assert_eq!(second_len.as_deref(), Ok("2"), "{stderr}");
    assert!(stderr.contains("geração 1: estado preservado"), "{stderr}");
    assert!(stderr.contains("geração 2: estado preservado"), "{stderr}");
}

/// Recarga com o estado do espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
/// §5.2, item 9): os globais guardam strings, listas compactas (`<int>`,
/// `<double>`) e gerais, uma closure que encadeia as das gerações
/// anteriores, um `double` em caixa e uma lista grande (com cartões, §2.7)
/// gravada entre gerações; o programa é recarregado três vezes, e cada
/// geração vê o estado de todas as anteriores.
#[test]
#[ignore = "compila DLL do SDK da fonte; executar no job sdk-fonte do Pesado"]
fn cli_preserva_o_estado_do_espaco_unificado_em_tres_recargas() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-espaco-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_espaco_v1.dart"), &entrada);

    let mut command = cli_com_sdk();
    command.arg("reload").arg(&entrada).arg("--preservar-estado")
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    // Cada geração imprime uma linha; a edição seguinte só depois dela.
    let esperadas = [
        "v1 1 a1 1 1.5 s1 1.5 1 1",
        "v2 2 a1a2 1,2 1.5,2.5 s1,s2 2.5 3 2",
        "v3 3 a1a2a3 1,2,3 1.5,2.5,3.5 s1,s2,s3 3.5 6 3",
        "v4 4 a1a2a3a4 1,2,3,4 1.5,2.5,3.5,4.5 s1,s2,s3,s4 4.5 10 4",
    ];
    let mut vistas = Vec::new();
    for (i, _) in esperadas.iter().enumerate() {
        let prazo = Duration::from_secs(if i == 0 { 60 } else { 20 });
        let Ok(linha) = rx.recv_timeout(prazo) else { break };
        vistas.push(linha);
        if i + 1 < esperadas.len() {
            editar(&fixtures.join(format!("reload_espaco_v{}.dart", i + 2)), &entrada);
        }
    }
    let stderr = encerrar(&mut child, Some("geração 4: estado preservado"));
    reader.join().expect("leitor");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    assert_eq!(vistas, esperadas, "{stderr}");
    for g in 1..=4 {
        assert!(stderr.contains(&format!("geração {g}: estado preservado")), "{stderr}");
    }
}

/// Hot reload ao vivo: a edição chega ao programa em execução (um timer
/// periódico), no ponto seguro do laço, sem executar o `main` de novo, e o
/// estático continua a contagem.
#[test]
fn cli_recarrega_o_programa_em_execucao() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-vivo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_vivo_v1.dart"), &entrada);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload").arg(&entrada)
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    let mut linhas = Vec::new();
    let mut ultimo_v1 = None;
    let mut primeiro_v2 = None;
    let mut editado = false;
    while let Ok(linha) = rx.recv_timeout(Duration::from_secs(60)) {
        linhas.push(linha.clone());
        if let Some(n) = linha.strip_prefix("v1 ").and_then(|n| n.parse::<u32>().ok()) {
            ultimo_v1 = Some(n);
            if n == 3 && !editado {
                editado = true;
                editar(&fixtures.join("reload_vivo_v2.dart"), &entrada);
            }
        }
        if let Some(n) = linha.strip_prefix("v2 ").and_then(|n| n.parse::<u32>().ok()) {
            primeiro_v2 = Some(n);
            break;
        }
    }
    let stderr = encerrar(&mut child, Some("geração 2: publicada no programa em execução"));
    reader.join().expect("leitor");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    let (Some(v1), Some(v2)) = (ultimo_v1, primeiro_v2) else {
        panic!("sem as duas versões na saída: {linhas:?}\n{stderr}");
    };
    assert_eq!(v2, v1 + 1, "a contagem continua do estático vivo: {linhas:?}");
    assert_eq!(linhas.iter().filter(|l| *l == "main").count(), 1, "o main não roda de novo: {linhas:?}");
    assert!(stderr.contains("geração 2: publicada no programa em execução"), "{stderr}");
}

/// Recarga estrutural (J03): uma classe nova inserida antes de uma classe
/// com objeto vivo (na ordem dos nomes, que numerava as classes) é aceita —
/// a mesma classe fica com o mesmo id na geração nova — e o objeto vivo
/// continua com o estado e o corpo novo dos membros.
#[test]
fn cli_recarrega_com_classe_inserida() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-classe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_classe_v1.dart"), &entrada);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload").arg(&entrada)
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    let mut linhas = Vec::new();
    let mut ultimo_v1 = None;
    let mut primeira_v2 = None;
    let mut editado = false;
    while let Ok(linha) = rx.recv_timeout(Duration::from_secs(60)) {
        linhas.push(linha.clone());
        let partes: Vec<&str> = linha.split(' ').collect();
        match partes.as_slice() {
            ["v1", n, "z"] => {
                let n: u32 = n.parse().expect("contagem");
                ultimo_v1 = Some(n);
                if n == 3 && !editado {
                    editado = true;
                    editar(&fixtures.join("reload_classe_v2.dart"), &entrada);
                }
            }
            ["v2", ..] => {
                primeira_v2 = Some(linha.clone());
                break;
            }
            _ => {}
        }
    }
    let stderr = encerrar(&mut child, Some("geração 2: publicada no programa em execução"));
    reader.join().expect("leitor");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    let (Some(v1), Some(v2)) = (ultimo_v1, primeira_v2) else {
        panic!("sem as duas versões na saída: {linhas:?}\n{stderr}");
    };
    assert_eq!(v2, format!("v2 {} zz alfa true", v1 + 1), "{linhas:?}\n{stderr}");
    assert_eq!(linhas.iter().filter(|l| *l == "main").count(), 1, "o main não roda de novo: {linhas:?}");
    assert!(stderr.contains("geração 2: publicada no programa em execução"), "{stderr}");
}

/// Uma classe que some numa geração e volta na seguinte fica com o mesmo id
/// (J03): o objeto vivo dela, guardado num global, continua sendo dela
/// (`z is Zeta`). A geração do meio não tem a classe nem o global.
#[test]
fn cli_classe_que_some_e_volta_mantem_o_id() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-volta-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_classe_v1.dart"), &entrada);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload").arg(&entrada)
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    let mut linhas = Vec::new();
    let mut fase = 0;
    let mut primeira_v2 = None;
    while let Ok(linha) = rx.recv_timeout(Duration::from_secs(60)) {
        linhas.push(linha.clone());
        let partes: Vec<&str> = linha.split(' ').collect();
        match partes.as_slice() {
            ["v1", "3", "z"] if fase == 0 => {
                fase = 1;
                editar(&fixtures.join("reload_classe_sem.dart"), &entrada);
            }
            ["vb", ..] if fase == 1 => {
                fase = 2;
                editar(&fixtures.join("reload_classe_v2.dart"), &entrada);
            }
            ["v2", ..] => {
                primeira_v2 = Some(linha.clone());
                break;
            }
            _ => {}
        }
    }
    let stderr = encerrar(&mut child, None);
    reader.join().expect("leitor");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    let v2 = primeira_v2.unwrap_or_else(|| panic!("sem a v2 na saída: {linhas:?}\n{stderr}"));
    assert!(v2.ends_with(" zz alfa true"), "o objeto vivo deixou de ser Zeta: {v2}\n{linhas:?}\n{stderr}");
}

/// Recarga estrutural (J03): campo removido, campo anulável novo (antes de
/// um existente) e `late` novo com inicializador. O objeto vivo migra pelo
/// nome: `valor` continua, `rotulo` é `null` e `dobro` roda o inicializador
/// na primeira leitura.
#[test]
fn cli_recarrega_com_campos_mudados() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-campos-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_campos_v1.dart"), &entrada);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload").arg(&entrada)
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    let mut linhas = Vec::new();
    let mut ultimo_v1 = None;
    let mut primeira_v2 = None;
    let mut editado = false;
    while let Ok(linha) = rx.recv_timeout(Duration::from_secs(60)) {
        linhas.push(linha.clone());
        let partes: Vec<&str> = linha.split(' ').collect();
        match partes.as_slice() {
            ["v1", n, "7"] => {
                let n: u32 = n.parse().expect("contagem");
                ultimo_v1 = Some(n);
                if n == 3 && !editado {
                    editado = true;
                    editar(&fixtures.join("reload_campos_v2.dart"), &entrada);
                }
            }
            ["v2", ..] => {
                primeira_v2 = Some(linha.clone());
                break;
            }
            _ => {}
        }
    }
    let stderr = encerrar(&mut child, Some("geração 2: publicada no programa em execução"));
    reader.join().expect("leitor");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    let (Some(v1), Some(v2)) = (ultimo_v1, primeira_v2) else {
        panic!("sem as duas versões na saída: {linhas:?}\n{stderr}");
    };
    assert_eq!(v2, format!("v2 {} null true", v1 + 1), "{linhas:?}\n{stderr}");
    assert_eq!(linhas.iter().filter(|l| *l == "main").count(), 1, "o main não roda de novo: {linhas:?}");
    assert!(stderr.contains("geração 2: publicada no programa em execução"), "{stderr}");
}

/// Recarga estrutural (J03): a assinatura de uma função de topo e de um
/// método muda (parâmetro opcional, nomeado). As funções ganham entradas
/// estáveis novas e o código novo as chama; o objeto vivo continua.
#[test]
fn cli_recarrega_com_assinatura_mudada() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-assinatura-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_assinatura_v1.dart"), &entrada);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload").arg(&entrada)
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    let mut linhas = Vec::new();
    let mut ultimo_v1 = None;
    let mut primeira_v2 = None;
    let mut editado = false;
    while let Ok(linha) = rx.recv_timeout(Duration::from_secs(60)) {
        linhas.push(linha.clone());
        let partes: Vec<&str> = linha.split(' ').collect();
        match partes.as_slice() {
            ["v1", _, n] => {
                let n: u32 = n.parse().expect("contagem");
                ultimo_v1 = Some(n);
                if n == 3 && !editado {
                    editado = true;
                    editar(&fixtures.join("reload_assinatura_v2.dart"), &entrada);
                }
            }
            ["v2", ..] => {
                primeira_v2 = Some(linha.clone());
                break;
            }
            _ => {}
        }
    }
    let stderr = encerrar(&mut child, Some("geração 2: publicada no programa em execução"));
    reader.join().expect("leitor");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    let (Some(v1), Some(v2)) = (ultimo_v1, primeira_v2) else {
        panic!("sem as duas versões na saída: {linhas:?}\n{stderr}");
    };
    let n = v1 + 1;
    assert_eq!(v2, format!("v2 p{n} {}", 3 * n), "{linhas:?}\n{stderr}");
    assert_eq!(linhas.iter().filter(|l| *l == "main").count(), 1, "o main não roda de novo: {linhas:?}");
    assert!(stderr.contains("geração 2: publicada no programa em execução"), "{stderr}");
}
