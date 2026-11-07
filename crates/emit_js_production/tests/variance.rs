//! Production variance RTI follows declared class parameters at every level.
use std::path::PathBuf;
use std::process::{Command, Output};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "dartforge-variance-{}-{}-{}",
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
fn runtime_subtyping_survives_metadata_and_production_minification() {
    let directory = Fixture::new();
    let sdk = dart();
    for (case, source, expected) in [
        (
            "contravariant",
            include_str!("programs/variance_contravariant.dart"),
            "contravariance ok\n",
        ),
        (
            "invariant",
            include_str!("programs/variance_invariant.dart"),
            "invariance ok\n",
        ),
        (
            "contracts",
            include_str!("programs/variance_runtime_contracts.dart"),
            "variance contracts ok\n",
        ),
        (
            "deep_hierarchy",
            include_str!("programs/deep_class_hierarchy.dart"),
            "deep inheritance ok\n",
        ),
    ] {
        let entry = directory.0.join(format!("{case}.dart"));
        std::fs::write(&entry, source).unwrap();
        for level in ["-O1", "-O2"] {
            let reference = directory.0.join(format!("{case}-reference{level}.js"));
            let official = Command::new(&sdk)
                .args(["compile", "js", level, "--enable-experiment=variance"])
                .arg(&entry)
                .arg("-o")
                .arg(&reference)
                .env_remove("DARTFORGE_JSPROD_VARIANCE")
                .output()
                .unwrap();
            success(&official);
            let artifact = directory.0.join(format!("{case}-candidate{level}.js"));
            let ours = Command::new(compiler())
                .arg(&entry)
                .args([
                    level,
                    "--own-sdk",
                    "--language-version=3.6",
                    "--enable-experiment=variance",
                    "-o",
                ])
                .arg(&artifact)
                .arg("--sdk")
                .arg(std::env::var_os("DARTFORGE_SDK_LIB").expect("DARTFORGE_SDK_LIB"))
                .env_remove("DARTFORGE_JSPROD_VARIANCE")
                .output()
                .unwrap();
            success(&ours);
            for file in [&reference, &artifact] {
                let runner = file.with_extension("cjs");
                std::fs::write(&runner, format!(
                    "globalThis.self=Object.create(globalThis);\nself.scheduleImmediate=(f)=>{{Promise.resolve().then(f);}};\nrequire('./{}');\n",
                    file.file_name().unwrap().to_str().unwrap()
                )).unwrap();
                let output = Command::new("node").arg(&runner).output().unwrap();
                success(&output);
                assert!(
                    output.stderr.is_empty(),
                    "{case} {level}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(
                    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
                    expected,
                    "{case} {level}"
                );
            }
        }
    }
}
