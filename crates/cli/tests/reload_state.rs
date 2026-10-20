#![cfg(feature = "jit")]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Deadline per output line. The first line only comes out after the JIT
/// compiles the program and, with a cold cache, the SDK libraries it uses:
/// on the Windows CI (2 cores, tests in parallel, empty cache) this took
/// more than 60 s. It only matters when the test fails.
const DEADLINE: Duration = Duration::from_secs(300);

/// Ends the CLI and returns its stderr. With `expected`, first waits for the
/// line that contains it (up to [`DEADLINE`]): the CLI writes the generation's
/// diagnostic right after publishing it, and the program, already running the
/// new code, may write the new output before — killing the CLI on seeing the
/// output would lose the diagnostic.
fn shut_down(child: &mut Child, expected: Option<&str>) -> String {
    let stderr = child.stderr.take().expect("stderr");
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut text = String::new();
    if let Some(expected) = expected {
        let deadline = Instant::now() + DEADLINE;
        while let Ok(line) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            text.push_str(&line);
            text.push('\n');
            if line.contains(expected) {
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    for line in rx {
        text.push_str(&line);
        text.push('\n');
    }
    reader.join().expect("stderr reader");
    text
}

/// Writes the fixture the way an editor does: new content into the same file,
/// with the current mtime. (`fs::copy` on Windows — `CopyFileW` — preserves
/// the source's mtime, and two fixtures of the same size, written together in
/// the checkout, would leave the file with the same signature as before.)
fn edit(source: &std::path::Path, destination: &std::path::Path) {
    let content = std::fs::read(source).expect("fixture");
    std::fs::write(destination, content).expect("edit");
}

/// The CLI with the source-built SDK library (the only mode since the
/// unified space, docs/NATIVO-ESPACO-UNIFICADO.md §4.7).
fn cli_with_sdk() -> Command {
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
    command.env("DARTFORGE_SDK_DLL", dll);
    command
}

#[test]
#[ignore = "compiles the source-built SDK DLL; run in the sdk-fonte job of the Pesado workflow"]
fn cli_preserves_static_after_editing_the_same_dart_file() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    // One directory per test: they run in parallel in the same process, and
    // one's cleanup used to delete the other's fixture.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-reload-cli-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let input = dir.join("main.dart");
    edit(&fixtures.join("reload_state_v1.dart"), &input);

    let mut command = cli_with_sdk();
    command
        .arg("reload")
        .arg(&input)
        .arg("--preserve-state")
        .arg("--interval")
        .arg("50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("line"));
        }
    });

    // The first generation compiles the whole program (CI's `test` profile
    // is debug): the same deadline as the live test.
    let first = rx.recv_timeout(DEADLINE);
    let first_len = rx.recv_timeout(DEADLINE);
    if first.as_deref() == Ok("1") && first_len.as_deref() == Ok("1") {
        edit(&fixtures.join("reload_state_v2.dart"), &input);
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
    let stderr = shut_down(&mut child, Some("generation 2: state preserved"));
    reader.join().expect("reader");
    std::fs::remove_dir_all(&dir).expect("fixture cleanup");

    assert_eq!(first.as_deref(), Ok("1"), "{stderr}");
    assert_eq!(first_len.as_deref(), Ok("1"), "{stderr}");
    assert_eq!(second.as_deref(), Ok("11"), "{stderr}");
    assert_eq!(second_len.as_deref(), Ok("2"), "{stderr}");
    assert!(stderr.contains("generation 1: state preserved"), "{stderr}");
    assert!(stderr.contains("generation 2: state preserved"), "{stderr}");
}

/// Reload with the state of the unified space (docs/NATIVO-ESPACO-UNIFICADO.md
/// §5.2, item 9): the globals hold strings, compact (`<int>`, `<double>`) and
/// general lists, a closure that chains those of the previous generations, a
/// boxed `double` and a large list (with cards, §2.7) written between
/// generations; the program is reloaded three times, and each generation sees
/// the state of all the previous ones.
#[test]
#[ignore = "compiles the source-built SDK DLL; run in the sdk-fonte job of the Pesado workflow"]
fn cli_preserves_unified_space_state_across_three_reloads() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-reload-espaco-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let input = dir.join("main.dart");
    edit(&fixtures.join("reload_espaco_v1.dart"), &input);

    let mut command = cli_with_sdk();
    command
        .arg("reload")
        .arg(&input)
        .arg("--preserve-state")
        .arg("--interval")
        .arg("50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("line"));
        }
    });

    // Each generation prints one line; the next edit only after it.
    let expected_lines = [
        "v1 1 a1 1 1.5 s1 1.5 1 1",
        "v2 2 a1a2 1,2 1.5,2.5 s1,s2 2.5 3 2",
        "v3 3 a1a2a3 1,2,3 1.5,2.5,3.5 s1,s2,s3 3.5 6 3",
        "v4 4 a1a2a3a4 1,2,3,4 1.5,2.5,3.5,4.5 s1,s2,s3,s4 4.5 10 4",
    ];
    let mut seen = Vec::new();
    for (i, _) in expected_lines.iter().enumerate() {
        let deadline = if i == 0 {
            DEADLINE
        } else {
            Duration::from_secs(20)
        };
        let Ok(line) = rx.recv_timeout(deadline) else {
            break;
        };
        seen.push(line);
        if i + 1 < expected_lines.len() {
            edit(
                &fixtures.join(format!("reload_espaco_v{}.dart", i + 2)),
                &input,
            );
        }
    }
    let stderr = shut_down(&mut child, Some("generation 4: state preserved"));
    reader.join().expect("reader");
    std::fs::remove_dir_all(&dir).expect("fixture cleanup");

    assert_eq!(seen, expected_lines, "{stderr}");
    for g in 1..=4 {
        assert!(
            stderr.contains(&format!("generation {g}: state preserved")),
            "{stderr}"
        );
    }
}

/// Live hot reload: the edit reaches the running program (a periodic timer),
/// at the event loop's safepoint, without running `main` again, and the
/// static keeps counting.
#[test]
fn cli_reloads_the_running_program() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-reload-vivo-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let input = dir.join("main.dart");
    edit(&fixtures.join("reload_live_v1.dart"), &input);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload")
        .arg(&input)
        .arg("--interval")
        .arg("50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("line"));
        }
    });

    let mut lines = Vec::new();
    let mut last_v1 = None;
    let mut first_v2 = None;
    let mut edited = false;
    while let Ok(line) = rx.recv_timeout(DEADLINE) {
        lines.push(line.clone());
        if let Some(n) = line.strip_prefix("v1 ").and_then(|n| n.parse::<u32>().ok()) {
            last_v1 = Some(n);
            if n == 3 && !edited {
                edited = true;
                edit(&fixtures.join("reload_live_v2.dart"), &input);
            }
        }
        if let Some(n) = line.strip_prefix("v2 ").and_then(|n| n.parse::<u32>().ok()) {
            first_v2 = Some(n);
            break;
        }
    }
    let stderr = shut_down(
        &mut child,
        Some("generation 2: published into the running program"),
    );
    reader.join().expect("reader");
    std::fs::remove_dir_all(&dir).expect("fixture cleanup");

    let (Some(v1), Some(v2)) = (last_v1, first_v2) else {
        panic!("missing the two versions in the output: {lines:?}\n{stderr}");
    };
    assert_eq!(
        v2,
        v1 + 1,
        "the count continues from the live static: {lines:?}"
    );
    assert_eq!(
        lines.iter().filter(|l| *l == "main").count(),
        1,
        "main does not run again: {lines:?}"
    );
    assert!(
        stderr.contains("generation 2: published into the running program"),
        "{stderr}"
    );
}

/// Structural reload (J03): a new class inserted before a class with a live
/// object (in name order, which used to number the classes) is accepted —
/// the same class keeps the same id in the new generation — and the live
/// object keeps its state and the members' new body.
#[test]
fn cli_reloads_with_an_inserted_class() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-reload-classe-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let input = dir.join("main.dart");
    edit(&fixtures.join("reload_class_v1.dart"), &input);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload")
        .arg(&input)
        .arg("--interval")
        .arg("50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("line"));
        }
    });

    let mut lines = Vec::new();
    let mut last_v1 = None;
    let mut first_v2 = None;
    let mut edited = false;
    while let Ok(line) = rx.recv_timeout(DEADLINE) {
        lines.push(line.clone());
        let parts: Vec<&str> = line.split(' ').collect();
        match parts.as_slice() {
            ["v1", n, "z"] => {
                let n: u32 = n.parse().expect("count");
                last_v1 = Some(n);
                if n == 3 && !edited {
                    edited = true;
                    edit(&fixtures.join("reload_class_v2.dart"), &input);
                }
            }
            ["v2", ..] => {
                first_v2 = Some(line.clone());
                break;
            }
            _ => {}
        }
    }
    let stderr = shut_down(
        &mut child,
        Some("generation 2: published into the running program"),
    );
    reader.join().expect("reader");
    std::fs::remove_dir_all(&dir).expect("fixture cleanup");

    let (Some(v1), Some(v2)) = (last_v1, first_v2) else {
        panic!("missing the two versions in the output: {lines:?}\n{stderr}");
    };
    assert_eq!(
        v2,
        format!("v2 {} zz alfa true", v1 + 1),
        "{lines:?}\n{stderr}"
    );
    assert_eq!(
        lines.iter().filter(|l| *l == "main").count(),
        1,
        "main does not run again: {lines:?}"
    );
    assert!(
        stderr.contains("generation 2: published into the running program"),
        "{stderr}"
    );
}

/// A class that disappears in one generation and comes back in the next keeps
/// the same id (J03): its live object, held in a global, still belongs to it
/// (`z is Zeta`). The middle generation has neither the class nor the global.
#[test]
fn cli_class_that_disappears_and_returns_keeps_its_id() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-reload-volta-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let input = dir.join("main.dart");
    edit(&fixtures.join("reload_class_v1.dart"), &input);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload")
        .arg(&input)
        .arg("--interval")
        .arg("50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("line"));
        }
    });

    let mut lines = Vec::new();
    let mut phase = 0;
    let mut first_v2 = None;
    while let Ok(line) = rx.recv_timeout(DEADLINE) {
        lines.push(line.clone());
        let parts: Vec<&str> = line.split(' ').collect();
        match parts.as_slice() {
            ["v1", "3", "z"] if phase == 0 => {
                phase = 1;
                edit(&fixtures.join("reload_class_removed.dart"), &input);
            }
            ["vb", ..] if phase == 1 => {
                phase = 2;
                edit(&fixtures.join("reload_class_v2.dart"), &input);
            }
            ["v2", ..] => {
                first_v2 = Some(line.clone());
                break;
            }
            _ => {}
        }
    }
    let stderr = shut_down(&mut child, None);
    reader.join().expect("reader");
    std::fs::remove_dir_all(&dir).expect("fixture cleanup");

    let v2 = first_v2.unwrap_or_else(|| panic!("missing v2 in the output: {lines:?}\n{stderr}"));
    assert!(
        v2.ends_with(" zz alfa true"),
        "the live object stopped being a Zeta: {v2}\n{lines:?}\n{stderr}"
    );
}

/// Structural reload (J03): a removed field, a new nullable field (before an
/// existing one) and a new `late` field with an initializer. The live object
/// migrates by name: `valor` carries on, `rotulo` is `null` and `dobro` runs
/// the initializer on first read.
#[test]
fn cli_reloads_with_changed_fields() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-reload-campos-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let input = dir.join("main.dart");
    edit(&fixtures.join("reload_fields_v1.dart"), &input);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload")
        .arg(&input)
        .arg("--interval")
        .arg("50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("line"));
        }
    });

    let mut lines = Vec::new();
    let mut last_v1 = None;
    let mut first_v2 = None;
    let mut edited = false;
    while let Ok(line) = rx.recv_timeout(DEADLINE) {
        lines.push(line.clone());
        let parts: Vec<&str> = line.split(' ').collect();
        match parts.as_slice() {
            ["v1", n, "7"] => {
                let n: u32 = n.parse().expect("count");
                last_v1 = Some(n);
                if n == 3 && !edited {
                    edited = true;
                    edit(&fixtures.join("reload_fields_v2.dart"), &input);
                }
            }
            ["v2", ..] => {
                first_v2 = Some(line.clone());
                break;
            }
            _ => {}
        }
    }
    let stderr = shut_down(
        &mut child,
        Some("generation 2: published into the running program"),
    );
    reader.join().expect("reader");
    std::fs::remove_dir_all(&dir).expect("fixture cleanup");

    let (Some(v1), Some(v2)) = (last_v1, first_v2) else {
        panic!("missing the two versions in the output: {lines:?}\n{stderr}");
    };
    assert_eq!(
        v2,
        format!("v2 {} null true", v1 + 1),
        "{lines:?}\n{stderr}"
    );
    assert_eq!(
        lines.iter().filter(|l| *l == "main").count(),
        1,
        "main does not run again: {lines:?}"
    );
    assert!(
        stderr.contains("generation 2: published into the running program"),
        "{stderr}"
    );
}

/// Structural reload (J03): the signature of a top-level function and of a
/// method changes (optional, named parameter). The functions get new stable
/// entries and the new code calls them; the live object carries on.
#[test]
fn cli_reloads_with_a_changed_signature() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-reload-assinatura-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let input = dir.join("main.dart");
    edit(&fixtures.join("reload_signature_v1.dart"), &input);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload")
        .arg(&input)
        .arg("--interval")
        .arg("50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("line"));
        }
    });

    let mut lines = Vec::new();
    let mut last_v1 = None;
    let mut first_v2 = None;
    let mut edited = false;
    while let Ok(line) = rx.recv_timeout(DEADLINE) {
        lines.push(line.clone());
        let parts: Vec<&str> = line.split(' ').collect();
        match parts.as_slice() {
            ["v1", _, n] => {
                let n: u32 = n.parse().expect("count");
                last_v1 = Some(n);
                if n == 3 && !edited {
                    edited = true;
                    edit(&fixtures.join("reload_signature_v2.dart"), &input);
                }
            }
            ["v2", ..] => {
                first_v2 = Some(line.clone());
                break;
            }
            _ => {}
        }
    }
    let stderr = shut_down(
        &mut child,
        Some("generation 2: published into the running program"),
    );
    reader.join().expect("reader");
    std::fs::remove_dir_all(&dir).expect("fixture cleanup");

    let (Some(v1), Some(v2)) = (last_v1, first_v2) else {
        panic!("missing the two versions in the output: {lines:?}\n{stderr}");
    };
    let n = v1 + 1;
    assert_eq!(v2, format!("v2 p{n} {}", 3 * n), "{lines:?}\n{stderr}");
    assert_eq!(
        lines.iter().filter(|l| *l == "main").count(),
        1,
        "main does not run again: {lines:?}"
    );
    assert!(
        stderr.contains("generation 2: published into the running program"),
        "{stderr}"
    );
}
