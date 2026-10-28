//! The closed world over a real program (`tests/programs/cases.dart`), with
//! assertions by element name. Compiles against the SDK: `--ignored`.

use dartforge_elements::model::{ClassId, Element, FunctionElementId, Program, VariableId};
use dartforge_intern::Interner;
use dartforge_world::{ClassLevel, Roots, World, WorldInput, check_consistency, compute};
use std::path::Path;

struct Case<'a> {
    p: &'a Program,
    i: &'a Interner,
    m: &'a World,
}

impl Case<'_> {
    fn class(&self, name: &str) -> ClassId {
        (0..self.p.classes.len())
            .map(|i| ClassId(i as u32))
            .find(|c| {
                !self.p.library(self.p.class(*c).library).is_sdk
                    && self.i.resolve(self.p.class(*c).name) == name
            })
            .unwrap_or_else(|| panic!("class {name}"))
    }
    /// `C.m` (class member), `ext:m` (extension member) or `m` (top-level).
    fn function(&self, name: &str) -> FunctionElementId {
        let (class, member) = match name.split_once('.') {
            Some((c, m)) => (Some(self.class(c)), m),
            None => (None, name),
        };
        let ext = member.strip_prefix("ext:");
        let member = ext.unwrap_or(member);
        (0..self.p.functions.len())
            .map(|i| FunctionElementId(i as u32))
            .find(|f| {
                let func = self.p.function(*f);
                !self.p.library(func.library).is_sdk
                    && self.i.resolve(func.name) == member
                    && func.class == class
                    && func.extension.is_some() == ext.is_some()
            })
            .unwrap_or_else(|| panic!("function {name}"))
    }
    fn variable(&self, name: &str) -> VariableId {
        (0..self.p.variables.len())
            .map(|i| VariableId(i as u32))
            .find(|v| {
                !self.p.library(self.p.variable(*v).library).is_sdk
                    && self.i.resolve(self.p.variable(*v).name) == name
            })
            .unwrap_or_else(|| panic!("variable {name}"))
    }
    fn live(&self, name: &str) -> bool {
        self.m.is_live_function(self.function(name))
    }
    /// `C.m` by kind: `write` picks the setter (`set m`), otherwise the
    /// getter/method. Tells them apart by `FunctionKind`, because both are
    /// named `m`.
    fn live_kind(&self, name: &str, write: bool) -> bool {
        use dartforge_elements::model::FunctionKind;
        let (class, member) = match name.split_once('.') {
            Some((c, m)) => (Some(self.class(c)), m),
            None => (None, name),
        };
        let f = (0..self.p.functions.len())
            .map(|i| FunctionElementId(i as u32))
            .find(|f| {
                let func = self.p.function(*f);
                !self.p.library(func.library).is_sdk
                    && self.i.resolve(func.name) == member
                    && func.class == class
                    && matches!(func.kind, FunctionKind::Setter) == write
            })
            .unwrap_or_else(|| panic!("function {name} (write={write})"));
        self.m.is_live_function(f)
    }
    fn level(&self, name: &str) -> ClassLevel {
        self.m.class_level(self.class(name))
    }
}

#[test]
#[ignore = "compiles against the SDK; run with --ignored"]
fn closed_world_cases() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/cases.dart");
    let (r, _) = dartforge_emit_js::compile_with(&file, None, None, &Default::default(), |a| {
        let e = WorldInput {
            program: a.program,
            interner: a.interner,
            table: a.table,
            outline: a.outline,
            bodies: a.bodies,
        };
        let mut roots = Roots::default();
        let lib = a.program.entry.expect("entry library");
        let main = a.interner.lookup("main").unwrap();
        let Some(Element::Function(f)) = a
            .program
            .library(lib)
            .declared
            .get(&main)
            .and_then(|b| b.getter)
        else {
            panic!("main")
        };
        roots.functions.push(f);
        roots.selectors.push("toJson".into());
        let m = compute(e, &roots);
        let c = Case {
            p: a.program,
            i: a.interner,
            m: &m,
        };
        let mut errors: Vec<String> = Vec::new();
        let mut check = |cond: bool, o: &str| {
            if !cond {
                errors.push(o.to_string());
            }
        };
        // dynamic by name, in every instantiated class
        check(
            c.live("A.f") && c.live("B.f"),
            "A.f and B.f live (dynamic d.f())",
        );
        check(!c.live("A.g") && !c.live("A.morto"), "A.g and A.morto dead");
        check(
            c.level("NaoInstanciada") == ClassLevel::Dead && !c.live("NaoInstanciada.f"),
            "non-instantiated class dead even with a live selector",
        );
        check(
            c.level("SoTipo") == ClassLevel::Type,
            "class only in `is` is Tipo",
        );
        check(
            c.level("I") == ClassLevel::Type,
            "implemented interface is Tipo",
        );
        // SDK and Object protocol
        check(
            c.live("Comp.compareTo"),
            "compareTo live through Comparable (rule i)",
        );
        check(c.live("Comp.toString"), "toString live (rule ii)");
        check(!c.live("Comp.naoUsado"), "Comp.naoUsado dead");
        check(c.live("Nsm.noSuchMethod"), "noSuchMethod live");
        // super, mixin
        check(
            c.live("Base.s") && c.level("Base") == ClassLevel::Instantiated,
            "Base.s live through the chain",
        );
        check(
            c.live("M.doMixin") && !c.live("M.mixinMorto"),
            "mixin member by selector",
        );
        // statics and top-level
        check(
            c.live("Estat.usado") && !c.live("Estat.naoUsado"),
            "used static live, the other dead",
        );
        check(
            c.m.is_live_variable(c.variable("contador")),
            "static read/written live",
        );
        check(
            c.live("topoUsado") && !c.live("topoMorto") && !c.live("topoMorto2"),
            "top-level functions",
        );
        check(
            !c.m.is_live_variable(c.variable("preguicoso")),
            "unread top-level variable dead",
        );
        check(
            c.level("Cor") == ClassLevel::Instantiated,
            "used enum instantiated",
        );
        // extension
        check(
            c.live("ext:grita") && !c.live("ext:cala"),
            "extension members by name",
        );
        check(
            c.live("Rio.corre") && c.live("Rio.some") && c.live("ext:rola"),
            "class members and the extension member called on its `on` type live",
        );
        check(
            !c.live("ext:corre") && !c.live("ext:some"),
            "extension members dead under a call on another class or a dynamic call",
        );
        {
            use dartforge_elements::model::FunctionKind;
            let accessors: Vec<(FunctionKind, bool)> = (0..a.program.functions.len())
                .map(|i| FunctionElementId(i as u32))
                .filter(|f| {
                    let func = a.program.function(*f);
                    func.extension.is_some_and(|e| {
                        a.program
                            .extension(e)
                            .name
                            .is_some_and(|n| a.interner.resolve(n) == "ESaco")
                    }) && a.interner.resolve(func.name).trim_end_matches('=') == "cheio"
                })
                .map(|f| (a.program.function(f).kind, m.is_live_function(f)))
                .collect();
            check(
                accessors.len() == 2 && accessors.iter().all(|(_, live)| *live),
                "getter and setter of an extension accessor live under `+=` and `++`",
            );
        }
        // constructors
        check(
            c.level("Fab") == ClassLevel::Instantiated && c.live("Fab._"),
            "redirecting factory instantiates the target",
        );
        check(c.m.ctor_tearoff(c.function("Tear.")), "constructor tearoff");
        check(
            c.level("ConstUsada") == ClassLevel::Instantiated
                && c.level("ConstMorta") == ClassLevel::Dead,
            "used and unused const",
        );
        check(
            c.level("Json") == ClassLevel::Dead,
            "unreferenced class dead",
        );
        // selector kind: read and write are independent
        check(
            c.live_kind("SoLeitura.v", false) && !c.live_kind("SoLeitura.v", true),
            "read getter lives, never-written setter dies",
        );
        check(
            c.live_kind("SoEscrita.w", true) && !c.live_kind("SoEscrita.w", false),
            "written setter lives, never-read getter dies",
        );
        // restriction by receiver type: `toca` on `Sopro` does not keep `Flauta`'s `toca`
        check(
            c.live("Sopro.toca") && c.live("Gaita.toca"),
            "toca live in the receiver's cone (base and subclass)",
        );
        check(
            !c.live("Flauta.toca") && !c.live("Flauta.assobia"),
            "toca/assobia dead in the disjoint instantiated class",
        );
        check(
            c.level("Flauta") == ClassLevel::Instantiated,
            "Flauta instantiated even with its members pruned",
        );
        // dry-run check and determinism
        let inc = check_consistency(e, &roots, &m);
        check(inc.is_empty(), &format!("consistency check: {inc:?}"));
        let mut perm = roots.clone();
        perm.selectors.insert(0, "toJson".into());
        let m2 = compute(e, &perm);
        let same = (0..a.program.functions.len()).all(|i| {
            m.is_live_function(FunctionElementId(i as u32))
                == m2.is_live_function(FunctionElementId(i as u32))
        }) && (0..a.program.classes.len())
            .all(|i| m.class_level(ClassId(i as u32)) == m2.class_level(ClassId(i as u32)));
        check(same, "determinism");
        Ok(errors)
    })
    .expect("compilation");
    assert!(r.is_empty(), "failures:\n{}", r.join("\n"));
}

#[test]
#[ignore = "compiles against the SDK (bodies inferred); run with --ignored"]
fn always_null_parameters() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/null_params.dart");
    let (r, _) = dartforge_emit_js::compile_with_options(
        &file,
        None,
        None,
        &Default::default(),
        None,
        true,
        |a| {
            let e = WorldInput {
                program: a.program,
                interner: a.interner,
                table: a.table,
                outline: a.outline,
                bodies: a.bodies,
            };
            let mut roots = Roots::default();
            let lib = a.program.entry.expect("entry library");
            let main = a.interner.lookup("main").unwrap();
            let Some(Element::Function(f)) = a
                .program
                .library(lib)
                .declared
                .get(&main)
                .and_then(|b| b.getter)
            else {
                panic!("main")
            };
            roots.functions.push(f);
            let options = dartforge_world::Options {
                include_sdk: true,
                without_asserts: true,
                without_records: false,
                without_legacy: false,
                without_dynamic_named: false,
            };
            let mut computation = dartforge_world::Computation::new(e, options);
            computation.seed(&roots);
            let m = computation.world();
            let c = Case {
                p: a.program,
                i: a.interner,
                m: &m,
            };
            let mut errors: Vec<String> = Vec::new();
            for dead in [
                "deadStatic",
                "deadCtor",
                "deadVirtual",
                "deadAfterReturn",
                "deadChained",
                "Config._build",
            ] {
                if c.live(dead) {
                    errors.push(format!("{dead} should be dead"));
                }
            }
            for live in [
                "liveTitle",
                "liveTearOff",
                "liveDynamic",
                "liveWritten",
                "liveDefault",
                "liveVirtual",
            ] {
                if !c.live(live) {
                    errors.push(format!("{live} should be live"));
                }
            }
            let mut proved: Vec<&str> = m
                .null_params()
                .iter()
                .filter(|k| !a.program.unit(k.unit).uri.starts_with("dart:"))
                .map(|k| a.interner.resolve(k.name))
                .collect();
            proved.sort_unstable();
            let expected = [
                "extra", "extra", "extra", "mode", "suffix", "suffix", "suffix", "tag",
            ];
            if proved != expected {
                errors.push(format!("proved {proved:?}, expected {expected:?}"));
            }
            Ok(errors)
        },
    )
    .expect("compilation");
    assert!(r.is_empty(), "failures:\n{}", r.join("\n"));
}
