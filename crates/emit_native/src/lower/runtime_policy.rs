//! Versioned GC policy and isolate-state package bodies backed by scalar ABIs.
//!
//! Match resolved defining libraries and semantic signatures, leaving ordinary
//! VM fallback bodies intact when a declaration does not match the contract.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::{Instruction, Operand, Terminator, Type};
use dartforge_elements::model::FunctionKind;
use dartforge_frontend::ast::ParameterKind;
use dartforge_types::TypeId;
use dartforge_types::table::Type as DartType;

fn scalar(ctx: &Context, ty: TypeId, expected: Type) -> bool {
    match expected {
        Type::Void => matches!(ctx.table.get(ty), DartType::Void),
        Type::I1 | Type::I64 => {
            let name = if expected == Type::I1 { "bool" } else { "int" };
            matches!(ctx.table.get(ty), DartType::Interface { class, args, nullable: false }
                if Some(*class) == ctx.sdk_class("core", name) && args.is_empty())
        }
        _ => false,
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub(super) fn lower_runtime_policy(&mut self, fid: usize) -> bool {
        let function = &self.ctx.program.functions[fid];
        if function.kind != FunctionKind::Function
            || function.class.is_some()
            || function.extension.is_some()
            || !self.func.params.is_empty()
            || !crate::source::has_pragma(
                self.ctx.program,
                self.ctx.interner,
                fid,
                "dartforge:runtime-policy-v1",
            )
        {
            return false;
        }
        let uri = self.ctx.program.library(function.library).uri.as_str();
        let (native, abi, expected, count) = match (uri, self.ctx.symbol_name(function.name)) {
            ("package:dartforge/src/gc_policy.dart", "_gcSupportedV1") => {
                ("gc_policy_supported", Type::I8, Type::I1, 0)
            }
            ("package:dartforge/src/gc_policy.dart", "_gcElapsedMicrosV1") => {
                ("gc_policy_elapsed_micros", Type::I64, Type::I64, 0)
            }
            ("package:dartforge/src/gc_policy.dart", "_gcDeferV1") => {
                ("gc_policy_defer", Type::I64, Type::I64, 2)
            }
            ("package:dartforge/src/gc_policy.dart", "_gcCancelV1") => {
                ("gc_policy_cancel", Type::Void, Type::Void, 0)
            }
            ("package:dartforge/src/gc_policy.dart", "_gcRequestIdleV1") => {
                ("gc_policy_request_idle", Type::Void, Type::Void, 0)
            }
            ("package:dartforge/src/gc_policy.dart", "_gcCollectionsV1") => {
                ("gc_policy_collections", Type::I64, Type::I64, 0)
            }
            ("package:dartforge/src/runtime_state.dart", "_eventNextDeadlineMicrosV1") => {
                ("event_next_deadline_micros", Type::I64, Type::I64, 0)
            }
            ("package:dartforge/src/runtime_state.dart", "_isolateThreadIdV1") => {
                ("isolate_thread_id", Type::I64, Type::I64, 0)
            }
            _ => return false,
        };
        let Some(signature) = self.ctx.outline.functions.get(fid) else {
            return false;
        };
        let valid_return = if expected == Type::Void {
            matches!(self.func.return_ty, Type::Ref | Type::Void)
        } else {
            self.func.return_ty == expected
        };
        if !valid_return
            || !signature.type_params.is_empty()
            || !scalar(self.ctx, signature.return_type, expected)
            || signature.parameters.len() != count
            || signature
                .parameters
                .iter()
                .any(|p| p.kind != ParameterKind::Required || !scalar(self.ctx, p.ty, Type::I64))
        {
            return false;
        }
        self.declare_parameters(fid, false);
        let args = self
            .func
            .params
            .iter()
            .map(|(value, _, ty)| {
                debug_assert_eq!(*ty, Type::I64);
                (Operand::Val(*value), Type::I64)
            })
            .collect();
        let value = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: format!("dartforge_native_DartForge_{native}"),
                args,
                ret_ty: abi,
            },
            abi,
        );
        if expected == Type::Void {
            // Ordinary Dart void functions retain their managed-null return
            // convention; the native runtime operation itself returns void.
            let value = (self.func.return_ty == Type::Ref)
                .then_some(Operand::Constant(crate::hir::Constant::Null));
            self.terminate(Terminator::Return(value));
        } else {
            let value = self.coerce(value, expected);
            self.terminate(Terminator::Return(Some(value)));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn module(entry: &Path, config: &Path) -> crate::hir::Module {
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let program =
            dartforge_elements::load::load(entry, &sdk, Some(config), &mut interner).unwrap();
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) =
            dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program
            .libraries
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.is_sdk)
            .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
            .collect();
        let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(
            &program,
            &interner,
            &mut table,
            &core,
            &mut outline,
            &libraries,
        );
        let context = Context::new(&program, &interner, &table, &core, &outline, &bodies);
        crate::lower::lower_program(&context)
    }

    fn config(directory: &Path) -> std::path::PathBuf {
        let config = directory.join("packages.json");
        std::fs::write(&config, r#"{"configVersion":2,"packages":[{"name":"dartforge","rootUri":"pkg/","packageUri":"lib/","languageVersion":"3.6"}]}"#).unwrap();
        std::fs::create_dir_all(directory.join("pkg/lib/src")).unwrap();
        config
    }

    #[test]
    fn canonical_policy_bodies_use_exact_scalar_runtime_abis() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let config = config(directory.path());
            let declarations = [
                ("gc_policy.dart", "_gcSupportedV1", "bool", "", "false", "gc_policy_supported", Type::I8, 0),
                ("gc_policy.dart", "_gcElapsedMicrosV1", "int", "", "0", "gc_policy_elapsed_micros", Type::I64, 0),
                ("gc_policy.dart", "_gcDeferV1", "int", "int durationMicros, int maxExtraBytes", "0", "gc_policy_defer", Type::I64, 2),
                ("gc_policy.dart", "_gcCancelV1", "void", "", "null", "gc_policy_cancel", Type::Void, 0),
                ("gc_policy.dart", "_gcRequestIdleV1", "void", "", "null", "gc_policy_request_idle", Type::Void, 0),
                ("gc_policy.dart", "_gcCollectionsV1", "int", "", "0", "gc_policy_collections", Type::I64, 0),
                ("runtime_state.dart", "_eventNextDeadlineMicrosV1", "int", "", "-1", "event_next_deadline_micros", Type::I64, 0),
                ("runtime_state.dart", "_isolateThreadIdV1", "int", "", "0", "isolate_thread_id", Type::I64, 0),
            ];
            for file in ["gc_policy.dart", "runtime_state.dart"] {
                let source: String = declarations.iter().filter(|(f, ..)| *f == file)
                    .map(|(_, name, ret, params, fallback, ..)| format!("@pragma('dartforge:runtime-policy-v1') {ret} {name}({params}) => {fallback};\n")).collect();
                std::fs::write(directory.path().join("pkg/lib/src").join(file), source).unwrap();
            }
            let entry = directory.path().join("main.dart");
            std::fs::write(&entry, "import 'package:dartforge/src/gc_policy.dart'; import 'package:dartforge/src/runtime_state.dart'; void main() {}").unwrap();
            let module = module(&entry, &config);
            for (_, name, _, _, _, native, abi, count) in declarations {
                let function = module.functions.iter().find(|f| f.name == name).unwrap();
                let calls: Vec<_> = function.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(_, instruction, ty)| {
                    if let Instruction::CallRuntime { name: called, args, ret_ty } = instruction
                        && called == &format!("dartforge_native_DartForge_{native}") { return Some((args, ret_ty, ty)); }
                    None
                }).collect();
                assert_eq!(calls.len(), 1, "{name}: {function:?}");
                let (args, ret, registered) = calls[0];
                assert_eq!(*ret, abi);
                assert_eq!(*registered, abi);
                assert_eq!(args.len(), count);
                assert_eq!(function.params.len(), count);
                for (argument, parameter) in args.iter().zip(&function.params) {
                    assert_eq!(*argument, (Operand::Val(parameter.0), Type::I64));
                }
                if abi == Type::I8 {
                    assert_eq!(function.return_ty, Type::I1);
                    assert!(function.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, ty)| matches!(instruction, Instruction::Trunc { from: Type::I8, to: Type::I1, .. }) && *ty == Type::I1));
                }
                if abi == Type::Void {
                    assert!(matches!((&function.blocks.last().unwrap().terminator, function.return_ty),
                        (Terminator::Return(None), Type::Void) |
                        (Terminator::Return(Some(Operand::Constant(crate::hir::Constant::Null))), Type::Ref)));
                }
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn wrong_library_version_and_semantic_shapes_preserve_fallbacks() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let config = config(directory.path());
            let entry = directory.path().join("main.dart");
            for (file, version, name, ret, parameters, fallback) in [
                ("other.dart", "v1", "_gcCollectionsV1", "int", "", "17"),
                ("runtime_state.dart", "v1", "_gcCollectionsV1", "int", "", "17"),
                ("gc_policy.dart", "v2", "_gcCollectionsV1", "int", "", "17"),
                ("gc_policy.dart", "v1", "_gcCollectionsV1", "int?", "", "null"),
                ("gc_policy.dart", "v1", "_gcSupportedV1", "int", "", "17"),
                ("gc_policy.dart", "v1", "_gcDeferV1", "int", "double durationMicros, int bytes", "17"),
                ("gc_policy.dart", "v1", "_gcDeferV1", "int", "int? durationMicros, int bytes", "17"),
                ("gc_policy.dart", "v1", "_gcDeferV1", "int", "[int durationMicros = 0, int bytes = 0]", "17"),
                ("gc_policy.dart", "v1", "_gcCollectionsV1<T>", "int", "", "17"),
                ("runtime_state.dart", "v1", "_eventNextDeadlineMicrosV1", "double", "", "17.0"),
            ] {
                std::fs::write(directory.path().join("pkg/lib/src").join(file), format!("@pragma('dartforge:runtime-policy-{version}') {ret} {name}({parameters}) => {fallback};")).unwrap();
                std::fs::write(&entry, format!("import 'package:dartforge/src/{file}'; void main() {{}}")).unwrap();
                let module = module(&entry, &config);
                assert!(!module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_native_DartForge_gc_policy_") || name == "dartforge_native_DartForge_event_next_deadline_micros" || name == "dartforge_native_DartForge_isolate_thread_id")), "{file}, {version}, {name}, {ret}, {parameters}");
            }
        }).unwrap().join().unwrap();
    }
}
