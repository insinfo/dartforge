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
                    Some(Field { name, nullable: flags.contains('a'), late: flags.contains('l'), ty })
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
        if new.base != old_gen.base {
            return Err(format!("the enum fields of {class} changed; the reload is rejected — restart the session"));
        }
        let mut origin: Vec<i64> = (0..new.base as i64).collect();
        for c in &new.fields {
            match old_gen.fields.iter().position(|v| v.name == c.name) {
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
        Field { name: name.into(), nullable, late, ty: ty.into() }
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
}
