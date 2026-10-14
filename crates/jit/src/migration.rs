//! Migration of live instances on a structural reload (J03).
//!
//! The native emitter writes into the IR the object layout of each program class
//! (`; df.fields <id> <base> name:flags:ty,…`, with `a` nullable and `l` late;
//! `base` are the fields before the declared ones, the enum ones). When a class
//! changes its fields, the reload no longer has to be rejected: every live object
//! of that class moves to the new layout **by field name**, at publication — with
//! every isolate stopped at the event safepoint, with no Dart frame on the stack
//! —, and whatever it lacks receives:
//!
//! * `null`, if the new field's type accepts `null`;
//! * nothing (it stays uninitialized), if the new field is `late`: reading it runs
//!   its initializer, as the VM does with an added field, or throws
//!   `LateInitializationError`.
//!
//! The rest is rejected with the exact message, without changing the session: a
//! new field that is neither nullable nor `late` (the live object has no value for
//! it), a field that changed its type or its `late` (the stored value may not
//! fit), and changes in the enum fields.

/// A layout field written in the IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    pub name: String,
    pub owner: Option<String>,
    pub nullable: bool,
    pub late: bool,
    pub ty: String,
}

/// The layout of a class: id, enum fields before the declared ones, fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    pub class: i64,
    pub base: usize,
    pub fields: Vec<Field>,
}

/// The migration of a class's objects: the new number of fields and, for each new
/// position, the old one the value comes from (`-1`: `null` or uninitialized).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Migration {
    pub class: i64,
    pub origin: Vec<i64>,
}

/// The inverse of the emitter's `escapar`: `$` and two hexadecimal digits per
/// UTF-8 byte.
fn unescape(part: &str) -> Option<String> {
    let b = part.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            v.push(u8::from_str_radix(part.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            v.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(v).ok()
}

/// The layouts written in the IR `ir` (empty for an IR without them: the
/// comparison falls back to the field count of the allocations).
pub(crate) fn ir_layouts(ir: &str) -> Vec<Layout> {
    ir.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("; df.fields ")?;
            let mut p = rest.splitn(3, ' ');
            let class = p.next()?.parse().ok()?;
            let base = p.next()?.parse().ok()?;
            let list = p.next().unwrap_or("");
            let fields = list
                .split(',')
                .filter(|c| !c.is_empty())
                .map(|c| {
                    let mut q = c.split(':');
                    let name = unescape(q.next()?)?;
                    let flags = q.next()?;
                    let ty = unescape(q.next()?)?;
                    let owner = match q.next() {
                        Some(owner) => Some(unescape(owner)?),
                        None => None,
                    };
                    Some(Field { name, owner, nullable: flags.contains('a'), late: flags.contains('l'), ty })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Layout { class, base, fields })
        })
        .collect()
}

/// The migration of the classes whose layout changed from `before` to `after`, or
/// the rejection message. `name` gives a class's name by id.
pub(crate) fn plan_fn(before: &[Layout], after: &[Layout], name: &dyn Fn(i64) -> String) -> Result<Vec<Migration>, String> {
    let mut plan = Vec::new();
    for old_gen in before {
        let Some(new) = after.iter().find(|l| l.class == old_gen.class) else { continue };
        if new == old_gen {
            continue;
        }
        let class = name(old_gen.class);
        // A bare field spelling is not a physical declaration identity:
        // inherited private fields (or shadowed public fields) can share it.
        // Legacy layouts remain usable when their identities are unambiguous.
        for layout in [old_gen, new] {
            let mut seen = std::collections::HashSet::new();
            if layout.fields.iter().any(|field| !seen.insert((&field.owner, &field.name))) {
                return Err(format!("the field identities of {class} are ambiguous; restart the session"));
            }
        }
        if !old_gen.fields.is_empty() && !new.fields.is_empty()
            && old_gen.fields.iter().any(|field| field.owner.is_some())
                != new.fields.iter().any(|field| field.owner.is_some())
        {
            return Err(format!("the field identity metadata of {class} changed format; restart the session"));
        }
        if new.base != old_gen.base {
            return Err(format!("the enum fields of {class} changed; the reload is rejected — restart the session"));
        }
        let mut origin: Vec<i64> = (0..new.base as i64).collect();
        for c in &new.fields {
            match old_gen.fields.iter().position(|v| v.name == c.name && v.owner == c.owner) {
                Some(i) => {
                    let v = &old_gen.fields[i];
                    if v.ty != c.ty {
                        return Err(format!(
                            "field `{}` of {class} changed its type ({} to {}); the live objects hold values of the \
                             old type, so the reload is rejected — restart the session",
                            c.name, v.ty, c.ty
                        ));
                    }
                    if v.late != c.late {
                        return Err(format!(
                            "field `{}` of {class} {} `late`; the reload is rejected — restart the session",
                            c.name,
                            if c.late { "became" } else { "is no longer" }
                        ));
                    }
                    origin.push((old_gen.base + i) as i64);
                }
                None if c.nullable || c.late => origin.push(-1),
                None => {
                    return Err(format!(
                        "the new field `{}` of {class} ({}) is neither nullable nor `late`: the live objects have no \
                         value for it, so the reload is rejected — restart the session, or declare it `late` or nullable",
                        c.name, c.ty
                    ));
                }
            }
        }
        plan.push(Migration { class: old_gen.class, origin });
    }
    Ok(plan)
}

/// The plan in the runtime's format (`dartforge_set_migration`):
/// `[n, (class, new_len, origin…)…]`.
pub(crate) fn encode(plan: &[Migration]) -> Vec<i64> {
    let mut v = vec![plan.len() as i64];
    for m in plan {
        v.push(m.class);
        v.push(m.origin.len() as i64);
        v.extend(&m.origin);
    }
    v
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    fn field(name: &str, nullable: bool, late: bool, ty: &str) -> Field {
        Field { name: name.into(), owner: None, nullable, late, ty: ty.into() }
    }

    #[test]
    fn reads_the_ir_layouts() {
        let ir = "; df.fields 7 0 valor::int,rotulo$5f:a:String$3f\n; df.fields 8 2 \ndefine void @f() { ret void }\n";
        let l = ir_layouts(ir);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].fields, vec![field("valor", false, false, "int"), field("rotulo_", true, false, "String?")]);
        assert_eq!((l[1].class, l[1].base, l[1].fields.len()), (8, 2, 0));
    }

    #[test]
    fn migrates_by_name_and_accepts_nullable_and_late() {
        let before = [Layout { class: 5, base: 0, fields: vec![field("a", false, false, "int"), field("b", false, false, "String")] }];
        let after = [Layout {
            class: 5,
            base: 0,
            fields: vec![field("b", false, false, "String"), field("c", true, false, "int?"), field("d", false, true, "int")],
        }];
        let plan = plan_fn(&before, &after, &|_| "C".into()).expect("aceita");
        assert_eq!(plan, vec![Migration { class: 5, origin: vec![1, -1, -1] }]);
        assert_eq!(encode(&plan), vec![1, 5, 3, 1, -1, -1]);
    }

    #[test]
    fn rejects_new_field_without_value_and_changed_type() {
        let before = [Layout { class: 5, base: 0, fields: vec![field("a", false, false, "int")] }];
        let new = [Layout { class: 5, base: 0, fields: vec![field("a", false, false, "int"), field("z", false, false, "int")] }];
        let error = plan_fn(&before, &new, &|_| "C".into()).unwrap_err();
        assert!(error.contains("the new field `z` of C (int) is neither nullable nor `late`"), "{error}");
        let ty = [Layout { class: 5, base: 0, fields: vec![field("a", false, false, "String")] }];
        let error = plan_fn(&before, &ty, &|_| "C".into()).unwrap_err();
        assert!(error.contains("changed its type (int to String)"), "{error}");
    }

    #[test]
    fn declaring_owners_distinguish_reordered_and_removed_homonymous_fields() {
        let owned = |owner: &str| {
            let mut field = field("_value", false, false, "int");
            field.owner = Some(owner.into());
            field
        };
        let first = owned("base.dart:Base");
        let second = owned("child.dart:Base");
        let old = Layout { class: 5, base: 0, fields: vec![first.clone(), second.clone()] };
        let reordered = Layout { class: 5, base: 0, fields: vec![second.clone(), first] };
        assert_eq!(plan_fn(&[old.clone()], &[reordered], &|_| "C".into()).unwrap()[0].origin, vec![1, 0]);
        let removed = Layout { class: 5, base: 0, fields: vec![second] };
        assert_eq!(plan_fn(&[old], &[removed], &|_| "C".into()).unwrap()[0].origin, vec![1]);
        let parsed = ir_layouts("; df.fields 5 0 _value::int:base$2edart$3aBase\n");
        assert_eq!(parsed[0].fields[0].owner.as_deref(), Some("base.dart:Base"));
    }

    #[test]
    fn ambiguous_legacy_fields_and_changed_identity_format_are_refused() {
        let bare = field("_value", false, false, "int");
        let old = Layout { class: 5, base: 0, fields: vec![bare.clone(), bare.clone()] };
        let new = Layout { class: 5, base: 0, fields: vec![bare.clone()] };
        assert!(plan_fn(&[old.clone()], &[new.clone()], &|_| "C".into()).unwrap_err().contains("ambiguous"));
        assert!(plan_fn(&[old.clone()], &[old], &|_| "C".into()).unwrap().is_empty());
        let mut owned = bare;
        owned.owner = Some("main.dart:C".into());
        let annotated = Layout { class: 5, base: 0, fields: vec![owned] };
        for (before, after) in [(new.clone(), annotated.clone()), (annotated, new)] {
            assert!(plan_fn(&[before], &[after], &|_| "C".into()).unwrap_err().contains("changed format"));
        }
    }

    #[test]
    #[ignore = "requires LLVM and the source-built Dart SDK"]
    fn emitted_private_fields_migrate_by_declaration_not_spelling() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = std::env::temp_dir().join(format!("dartforge-jit-field-identity-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let entry = dir.join("main.dart");
            let child = dir.join("child.dart");
            std::fs::write(dir.join("base.dart"), "class Base { int _value = 11; int get baseValue => _value; void setBaseValue(int value) { _value = value; } }\n").unwrap();
            let original_child = "import 'base.dart'; class Child extends Base { int _value = 22; int get childValue => _value; void setChildValue(int value) { _value = value; } }\n";
            std::fs::write(&child, original_child).unwrap();
            let source = "import 'child.dart'; final instance = Child(); int turns = 0; void main() { if (++turns != 1 || instance.baseValue != 11 || instance.childValue != 22) throw StateError('Initial fields'); print(instance.baseValue + instance.childValue); instance.setBaseValue(101); instance.setChildValue(202); }\n";
            std::fs::write(&entry, source).unwrap();
            let sdk = std::env::var_os("DARTFORGE_TEST_SDK_LIB").map(std::path::PathBuf::from);
            let options = dartforge_emit_native::CompileOptions {
                contracts: dartforge_emit_native::contracts::ContractMode::Verify,
                sdk: sdk.as_deref(), packages: None, timings: false, optimize: false,
                language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None,
            };
            let before = dartforge_emit_native::emit_reloadable_ir(&entry, &options, None).unwrap();
            let old = ir_layouts(&before.text);
            let old_child = old.iter().find(|layout| layout.fields.len() == 2
                && layout.fields.iter().all(|field| field.name == "_value")).unwrap();
            std::fs::write(&child, original_child.replace("int _value = 22;", "int _value = 22; int? extra;")).unwrap();
            std::fs::write(&entry, "import 'child.dart'; final instance = Child(); int turns = 0; void main() { if (++turns != 2 || instance.baseValue != 101 || instance.childValue != 202 || instance.extra != null) throw StateError('Live private fields were not preserved'); print(instance.baseValue + instance.childValue); }\n").unwrap();
            let after = dartforge_emit_native::emit_reloadable_ir(&entry, &options, Some(&before.text)).unwrap();
            let new = ir_layouts(&after.text);
            let plan = plan_fn(&old, &new, &|id| format!("class {id}")).unwrap();
            assert_eq!(plan.iter().find(|migration| migration.class == old_child.class).unwrap().origin, vec![0, 1, -1]);
            let dll = dartforge_emit_native::sdk_module::source_sdk_dll().unwrap();
            let mut session = crate::JitSession::new_for_ir_with(&before.text, Some(&dll)).unwrap();
            session.add_reloadable_module("field-identity", &before.text).unwrap();
            assert_eq!(session.run_reloadable_main().unwrap().exit_code, 0);
            session.hot_reload("field-identity", &after.text).unwrap();
            assert_eq!(session.run_reloadable_main().unwrap().exit_code, 0);
            drop(session);
            std::fs::remove_dir_all(dir).unwrap();
        }).unwrap().join().unwrap();
    }
}
