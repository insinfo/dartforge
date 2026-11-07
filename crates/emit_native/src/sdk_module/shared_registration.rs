//! Eager table registration owned by the shared SDK image.
//! Optimized program IR expects the same startup guarantee as the static link.
use super::SdkLibrary;
use crate::hir::Module;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

pub(super) fn install(
    modules: &[(String, Module)],
    libraries: &mut [SdkLibrary],
) -> Result<(), String> {
    let mut tables = BTreeMap::new();
    for (_, module) in modules {
        for (cid, getter, _) in &module.method_tables {
            if module
                .closed_class_tables
                .as_ref()
                .is_some_and(|active| !active.contains(cid))
            {
                continue;
            }
            if let Some(prior) = tables.insert(*cid, getter.as_str())
                && prior != getter
            {
                return Err(format!(
                    "SDK class {cid} has conflicting method table getters"
                ));
            }
        }
    }
    let first = libraries
        .first_mut()
        .ok_or("shared SDK has no emitted libraries")?;
    let mut owned = Vec::new();
    super::defined_symbols(&first.ir, &mut owned);
    let owned: HashSet<_> = owned.iter().map(String::as_str).collect();
    if owned.contains("df.register.sdk_tables") {
        return Err("shared SDK already defines its eager table registration".into());
    }
    let mut declarations = String::new();
    let mut declared = owned.clone();
    for line in first.ir.lines().filter(|line| line.starts_with("declare ")) {
        if let Some((_, function)) = line.split_once('@')
            && let Some((name, _)) = function.split_once('(')
        {
            declared.insert(name);
        }
    }
    if !first.ir.contains("@dartforge_register_table(") {
        declarations.push_str("declare void @dartforge_register_table(i64, ptr)\n");
    }
    let mut body = String::from("define void @df.register.sdk_tables() {\nb0:\n");
    for (cid, getter) in tables {
        if declared.insert(getter) {
            writeln!(declarations, "declare ptr @{getter}()").unwrap();
        }
        writeln!(
            body,
            "  call void @dartforge_register_table(i64 {cid}, ptr @{getter})"
        )
        .unwrap();
    }
    body.push_str("  ret void\n}\n");
    first.ir.push_str(&declarations);
    first.ir.push_str(&body);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_uses_image_owned_getters_in_class_order() {
        let mut a = Module::new();
        a.method_tables.push((42, "df.mt.A".into(), vec![]));
        let mut b = Module::new();
        b.method_tables.push((7, "df.mt.B".into(), vec![]));
        b.method_tables.push((11, "df.mt.C".into(), vec![]));
        let mut libraries = vec![SdkLibrary {
            uri: "dart:a".into(),
            ir: "define ptr @df.mt.A() { ret ptr null }\ndeclare ptr @df.mt.B()\n".into(),
            rejected: vec![],
            tables: vec![],
            allocation_exports: vec![],
        }];
        install(
            &[("dart:a".into(), a), ("dart:b".into(), b)],
            &mut libraries,
        )
        .unwrap();
        let ir = &libraries[0].ir;
        assert!(!ir.contains("declare ptr @df.mt.A"));
        assert_eq!(ir.matches("declare ptr @df.mt.B()").count(), 1);
        assert_eq!(ir.matches("declare ptr @df.mt.C()").count(), 1);
        let body = ir
            .split("define void @df.register.sdk_tables()")
            .nth(1)
            .unwrap();
        assert!(
            body.find("i64 7, ptr @df.mt.B").unwrap() < body.find("i64 42, ptr @df.mt.A").unwrap()
        );
        assert_eq!(
            body.matches("call void @dartforge_register_table(").count(),
            3
        );
    }
}
