//! Edit latency with the generation engine turned on (engine plan §6.5).
//!
//! `cargo run --release -p dartforge-dev --example measure_generation -- <input.dart> <package_config.json> [output] [repetitions] [sem-motor]`
//!
//! Opens the session, compiles, picks a component whose `.template.dart`
//! comes from the native generator (with `.html` and `.scss` next to it) and
//! a `.dart` of the package without Angular, and measures each edit applied
//! and reverted (the file goes back to its original text at the end of each
//! one): the clock runs from detection to the written modules, which is the
//! session's `compile`.
use dartforge_dev::generation::{build_stage, GenerationStage};
use dartforge_dev::{Report, Session};
use std::path::{Path, PathBuf};

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn row(name: &str, r: &Report) {
    let m = r.engine.as_ref();
    println!(
        "{name:<34}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>8}{:>8}{:>9}{:>9}",
        ms(r.total()),
        m.map(|m| ms(m.elapsed_time)).unwrap_or(0.0),
        m.map(|m| ms(m.native_time)).unwrap_or(0.0),
        ms(r.generation_reload),
        ms(r.write),
        m.map(|m| m.executed_actions).unwrap_or(0),
        m.map(|m| m.changed_outputs).unwrap_or(0),
        r.reanalyzed_units,
        r.written_modules,
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let input = PathBuf::from(args.first().expect("usage: measure_generation <input.dart> <package_config.json> [output] [repetitions]"));
    let packages = PathBuf::from(args.get(1).expect("package_config.json"));
    let output = PathBuf::from(args.get(2).cloned().unwrap_or_else(|| "target/dev-measure-generation".into()));
    let repetitions: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(3);

    // `sem-motor` (no engine): the same session without a stage (the
    // baseline of the zero-cost rule), with the files of
    // `MEASURE_COMPONENT`/`MEASURE_NO_ANGULAR`.
    let no_engine = args.get(4).is_some_and(|a| a == "sem-motor");
    let stage = if no_engine {
        None
    } else {
        match build_stage(&input, Some(&packages)) {
            Some(Ok(e)) => Some(e),
            Some(Err(e)) => panic!("engine: {e}"),
            None => panic!("the project uses no builders"),
        }
    };
    let engine = stage.as_ref().map(|e| e.engine());
    let stages: Vec<Box<dyn GenerationStage>> = stage.into_iter().map(|e| Box::new(e) as Box<dyn GenerationStage>).collect();
    let mut session = Session::with_stages(&input, None, Some(&packages), &output, stages).expect("session");
    let t = std::time::Instant::now();
    let r = session.compile().expect("first compilation");
    println!("first compilation: {:.1} ms (wall clock {:.1} ms)", ms(r.total()), ms(t.elapsed()));
    print!("{}", r.text());

    // Covered component: native `.template.dart` output with `.html` and `.scss`.
    let root = dartforge_build::package_root(&input).expect("root");
    let (component, no_angular) = if let Ok(c) = std::env::var("MEASURE_COMPONENT") {
        let dart = PathBuf::from(c);
        ((dart.clone(), dart.with_extension("html"), dart.with_extension("scss")), std::env::var("MEASURE_NO_ANGULAR").ok().map(PathBuf::from))
    } else {
        let engine = engine.clone().expect("no engine: set MEASURE_COMPONENT");
        let m = engine.lock().unwrap();
        let mut comp = None;
        let mut native = std::collections::BTreeSet::new();
        for (i, a) in m.graph.actions.iter().enumerate() {
            let Some(r) = m.record(i) else { continue };
            let f = &m.phases[a.phase];
            if !matches!(r.origin, dartforge_build::engine::Origin::Native(_)) || f.factory != "templateCompiler" {
                continue;
            }
            let dart = root.join(a.input.path.as_ref());
            native.insert(dart.clone());
            let html = dart.with_extension("html");
            let scss = dart.with_extension("scss");
            let html_name = html.file_name().unwrap().to_string_lossy().to_string();
            let uses = std::fs::read_to_string(&dart)
                .is_ok_and(|t| t.contains("@Component") && t.contains(&html_name) && t.contains("styleUrls"));
            if comp.is_none() && a.input.path.starts_with("lib/") && html.is_file() && scss.is_file() && uses {
                comp = Some((dart, html, scss));
            }
        }
        // A small `.dart` of the package, without Angular: the generator
        // considers it trivial.
        let plain = native
            .iter()
            .find(|p| {
                p.to_string_lossy().contains("/src/")
                    && std::fs::metadata(p).is_ok_and(|m| m.len() < 8000)
                    && std::fs::read_to_string(p).is_ok_and(|t| {
                        !t.contains("@Component")
                            && !t.contains("@Directive")
                            && !t.contains("@Pipe")
                            && !t.contains("@Injectable")
                            && !t.contains("GenerateInjector")
                            && !t.contains("part ")
                            && t.contains("class ")
                    })
            })
            .cloned();
        (comp.expect("no covered component with .html and .scss"), plain)
    };
    let (dart, html, scss) = component;
    let _ = no_engine;
    println!("component: {}", dart.display());
    if let Some(s) = &no_angular {
        println!("without Angular: {}", s.display());
    }
    println!(
        "{:<34}{:>9}{:>9}{:>9}{:>9}{:>9}{:>8}{:>8}{:>9}{:>9}",
        "edit", "total", "engine", "native", "reload", "write", "actions", "outputs", "units", "modules"
    );
    let mut edits: Vec<(&str, PathBuf, String)> = vec![
        ("text in the .html", html.clone(), "\n<span>measure</span>\n".into()),
        ("property in the .scss", scss.clone(), "\n.measure-x { color: red; }\n".into()),
        ("body outside the template (.dart)", dart.clone(), "\nvoid _measure() { var x = 1; x++; }\n".into()),
    ];
    if let Some(s) = no_angular {
        edits.push(("body in a .dart without Angular", s, "\nvoid _measureNoAngular() { var x = 1; x++; }\n".into()));
    }
    for (name, file, addition) in edits {
        let original = std::fs::read(&file).expect("read");
        for i in 0..repetitions {
            let mut new = original.clone();
            new.extend_from_slice(addition.as_bytes());
            std::fs::write(&file, &new).unwrap();
            for p in session.changes() {
                session.file_changed(&p);
            }
            session.file_changed(&file);
            let r = session.compile().expect("edit");
            row(&format!("{name} #{i}"), &r);
            if let (true, Some(engine)) = (std::env::var("MEASURE_DEBUG").is_ok(), engine.as_ref()) {
                let m = engine.lock().unwrap();
                for (a, action) in m.graph.actions.iter().enumerate() {
                    if root.join(action.input.path.as_ref()) == dart || root.join(action.input.path.as_ref()).with_extension("") == dart.with_extension("") {
                        if let Some(r) = m.record(a) {
                            println!("    {} {:?} {:?}", action.input.path, r.origin, r.reason);
                        }
                    }
                }
                print!("{}", r.text());
            }
            std::fs::write(&file, &original).unwrap();
            for p in session.changes() {
                session.file_changed(&p);
            }
            session.file_changed(&file);
            let r = session.compile().expect("revert");
            row(&format!("  reverted #{i}"), &r);
        }
    }
    let _ = Path::new("");
}
