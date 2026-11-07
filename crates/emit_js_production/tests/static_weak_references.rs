//! Production static weak references follow the selected backend's tree shaking.
use std::path::PathBuf;
use std::process::{Command, Output};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "dartforge-static-weak-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn dart() -> PathBuf {
    let sdk = PathBuf::from(
        std::env::var_os("DARTFORGE_DART_SDK")
            .expect("DARTFORGE_DART_SDK selects the host Dart 3.6.2 oracle"),
    );
    assert_eq!(
        std::fs::read_to_string(sdk.join("version")).unwrap().trim(),
        "3.6.2"
    );
    sdk.join("bin")
        .join(if cfg!(windows) { "dart.exe" } else { "dart" })
}
fn compiler() -> PathBuf {
    std::env::var_os("DARTFORGE_JSPROD")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_dartforge-jsprod")))
}
fn success(out: &Output) {
    assert!(
        out.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
#[ignore = "requires the host Dart 3.6.2 SDK and Node"]
fn weak_targets_keep_strong_uses_identity_hash_rti_and_local_aliases() {
    let dir = Fixture::new();
    let sdk = dart();
    for (case, source, expected) in [
        (
            "basic",
            include_str!("programs/weak_reference_contracts.dart"),
            "weak tearoff contracts ok\n",
        ),
        (
            "advanced",
            include_str!("programs/weak_reference_advanced.dart"),
            "advanced weak contracts ok\n",
        ),
        (
            "locals",
            include_str!("programs/weak_reference_locals.dart"),
            "local weak contracts ok\n",
        ),
        (
            "dead-conditions",
            include_str!("programs/weak_reference_dead_conditions.dart"),
            "dead weak conditions ok\n",
        ),
        // dart2js accepts constructor wrappers; the AOT frontend rejects them.
        (
            "constructors",
            include_str!("programs/weak_reference_constructors.dart"),
            "constructor weak contracts ok\n",
        ),
    ] {
        let entry = dir.0.join(format!("{case}.dart"));
        std::fs::write(&entry, source).unwrap();
        for level in ["-O1", "-O2"] {
            let reference = dir.0.join(format!("{case}-reference{level}.js"));
            let official = Command::new(&sdk)
                .args(["compile", "js", level])
                .arg(&entry)
                .arg("-o")
                .arg(&reference)
                .output()
                .unwrap();
            success(&official);
            let artifact = dir.0.join(format!("{case}-candidate{level}.js"));
            let mut command = Command::new(compiler());
            command.arg(&entry).arg(level).arg("-o").arg(&artifact);
            command
                .arg("--sdk")
                .arg(std::env::var_os("DARTFORGE_SDK_LIB").expect("DARTFORGE_SDK_LIB"));
            let ours = command.output().unwrap();
            success(&ours);
            for file in [&reference, &artifact] {
                let run = Command::new("node").arg(file).output().unwrap();
                success(&run);
                assert!(
                    run.stderr.is_empty(),
                    "{}",
                    String::from_utf8_lossy(&run.stderr)
                );
                assert_eq!(
                    String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n"),
                    expected,
                    "{case} {level}"
                );
            }
        }
    }
}
#[test]
#[ignore = "requires the host Dart 3.6.2 SDK"]
fn invalid_weak_uses_and_unused_declarations_are_rejected_before_pruning() {
    let dir = Fixture::new();
    let sdk = dart();
    for (case, source) in [
        (
            "unused-instance",
            "class C { @pragma('weak-tearoff-reference') Function? weak(Function? x) => x; } void main() {}",
        ),
        (
            "null-target",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x) => x; void main() { weak(null); }",
        ),
        (
            "cast-target",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x) => x; int target()=>1; void main() { weak(target as Function); }",
        ),
        (
            "conditional-target",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x) => x; int a()=>1; int b()=>2; void main() { weak(true ? a : b); }",
        ),
        (
            "parameter-target",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x) => x; int target([int x=1])=>x; void main() { weak(target); }",
        ),
        (
            "runtime-alias",
            "@pragma('weak-tearoff-reference') Function? weak(Function? x) => x; int target()=>1; void main() { final alias=target; weak(alias); }",
        ),
    ] {
        let entry = dir.0.join(format!("{case}.dart"));
        std::fs::write(&entry, source).unwrap();
        let official = Command::new(&sdk)
            .args(["compile", "js", "-O2"])
            .arg(&entry)
            .arg("-o")
            .arg(dir.0.join(format!("{case}-official.js")))
            .output()
            .unwrap();
        assert!(!official.status.success(), "{case}");
        let artifact = dir.0.join(format!("{case}-candidate.js"));
        let ours = Command::new(compiler())
            .arg(&entry)
            .args(["-O2", "-o"])
            .arg(&artifact)
            .arg("--sdk")
            .arg(std::env::var_os("DARTFORGE_SDK_LIB").expect("DARTFORGE_SDK_LIB"))
            .output()
            .unwrap();
        assert!(!ours.status.success(), "{case}");
        assert!(!artifact.exists(), "{case}");
        assert!(
            String::from_utf8_lossy(&ours.stderr).contains("invalid static weak reference"),
            "{case}: {}",
            String::from_utf8_lossy(&ours.stderr)
        );
    }
}
