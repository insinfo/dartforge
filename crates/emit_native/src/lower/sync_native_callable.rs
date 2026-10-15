//! Exact versioned package entry points for owner-thread synchronous callbacks.
//!
//! Only resolved declarations from the defining package library are replaced.
//! Generic creation redirects to the SDK helper with the existing function RTI
//! tuple, rather than treating its timeout argument as a class factory tuple.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::{Constant, Operand, Terminator, Type};
use dartforge_elements::model::FunctionKind;
use dartforge_frontend::ast::ParameterKind;
use dartforge_types::table::Type as DartType;

fn declaration(ctx: &Context, fid: usize) -> bool {
    let function = &ctx.program.functions[fid];
    ctx.program.library(function.library).uri == "package:dartforge/src/sync_native_callable.dart"
        && function.kind == FunctionKind::Function
        && function.class.is_none()
        && function.extension.is_none()
        && crate::source::has_pragma(ctx.program, ctx.interner, fid, "dartforge:sync-callback-v1")
}

fn interface(
    ctx: &Context,
    ty: dartforge_types::TypeId,
    library: &str,
    name: &str,
    nullable: bool,
) -> bool {
    matches!(ctx.table.get(ty), DartType::Interface { class, args, nullable: n }
        if Some(*class) == ctx.sdk_class(library, name) && args.is_empty() && *n == nullable)
}

pub(super) fn is_creation(ctx: &Context, fid: usize) -> bool {
    if !declaration(ctx, fid)
        || ctx.symbol_name(ctx.program.functions[fid].name) != "_createSyncNativeCallableV1"
    {
        return false;
    }
    let Some(signature) = ctx.outline.functions.get(fid) else {
        return false;
    };
    if signature.type_params.len() != 1
        || signature.parameters.len() != 3
        || signature
            .parameters
            .iter()
            .any(|p| p.kind != ParameterKind::Required)
        || !interface(
            ctx,
            ctx.table.param(signature.type_params[0]).bound,
            "core",
            "Function",
            false,
        )
        || !interface(ctx, signature.parameters[0].ty, "core", "Function", false)
        || !interface(ctx, signature.parameters[1].ty, "core", "Object", true)
        || !interface(ctx, signature.parameters[2].ty, "core", "int", false)
    {
        return false;
    }
    matches!(ctx.table.get(signature.return_type), DartType::Interface { class, args, nullable: false }
        if Some(*class) == ctx.sdk_class("ffi", "NativeCallable") && args.len() == 1
        && matches!(ctx.table.get(args[0]), DartType::TypeParameter { param, nullable: false } if *param == signature.type_params[0]))
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub(super) fn lower_sync_callback_support(&mut self, fid: usize) -> bool {
        if !declaration(self.ctx, fid)
            || self.ctx.symbol_name(self.ctx.program.functions[fid].name)
                != "_supportsSyncNativeCallableV1"
            || self.func.return_ty != Type::I1
            || !self.func.params.is_empty()
        {
            return false;
        }
        let Some(signature) = self.ctx.outline.functions.get(fid) else {
            return false;
        };
        if !signature.type_params.is_empty()
            || !signature.parameters.is_empty()
            || !interface(self.ctx, signature.return_type, "core", "bool", false)
        {
            return false;
        }
        self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Bool(
            true,
        )))));
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::hir::{Constant, Instruction, Module, Operand, Terminator, Type};
    use std::path::Path;

    fn lower(entry: &Path, config: &Path) -> Module {
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
        let library = program
            .functions
            .iter()
            .find(|f| interner.resolve(f.name) == "_supportsSyncNativeCallableV1")
            .unwrap()
            .library;
        // Real emission uses the source SDK's generic helper ABI. Restrict the
        // test module to the defining package while retaining SDK signatures.
        let context =
            crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
                .with_source_sdk()
                .only_the_library(library);
        let mut module = crate::lower::lower_program(&context);
        // Library-only lowering skips program-level FFI inventory. Exercise
        // that real program pass explicitly over the complete inferred table.
        super::super::ffi::lower_ffi(&context, &mut module);
        module
    }

    #[test]
    fn real_package_creation_preserves_generic_timeout_and_native_signatures() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/dartforge").canonicalize().unwrap();
            let config = directory.path().join("packages.json");
            std::fs::write(&config, serde_json::json!({"configVersion": 2, "packages": [{"name": "dartforge", "rootUri": url::Url::from_directory_path(&package).unwrap().as_str(), "packageUri": "lib/", "languageVersion": "3.6"}]}).to_string()).unwrap();
            let entry = directory.path().join("main.dart");
            std::fs::write(&entry, "import 'dart:ffi'; import 'package:dartforge/src/sync_native_callable.dart'; typedef Mixed = Double Function(Pointer<Void>, Float, Double); SyncNativeCallable<T> make<T extends Function>(Function callback) => SyncNativeCallable<T>(callback, exceptionalReturn: 17, queueTimeout: const Duration(microseconds: 731)); void main() { print(SyncNativeCallable.isSupported); final a = make<Int32 Function(Int32)>((int value) => value + 1); final b = make<Mixed>((Pointer<Void> p, double f, double d) => f + d); a.close(); b.close(); }").unwrap();
            let module = lower(&entry, &config);
            let support = module.functions.iter().find(|f| f.name == "_supportsSyncNativeCallableV1").unwrap();
            assert!(matches!(support.blocks[0].terminator, Terminator::Return(Some(Operand::Constant(Constant::Bool(true))))));
            let redirected: Vec<_> = module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).filter_map(|(_, instruction, _)| {
                if let Instruction::CallStatic { symbol, args, ret_ty } = instruction {
                    if symbol.ends_with("._dartforgeCallableSync") { return Some((args, ret_ty)); }
                }
                None
            }).collect();
            assert!(!redirected.is_empty(), "real generic package creation was not redirected");
            for (args, ret) in redirected {
                assert_eq!(*ret, Type::Ref);
                assert_eq!(args.len(), 4, "callback, exceptional return, timeout, function type tuple");
                assert!(!matches!(args[3], Operand::Constant(Constant::Int(0))), "generic function T must not become dynamic");
            }
            let keys: Vec<_> = module.ffi_callbacks.iter().map(|c| c.key.as_str()).collect();
            assert!(keys.contains(&"i_i"), "Int32 signature missing: {keys:?}");
            assert!(keys.contains(&"d_pfd"), "instantiated mixed native signature missing: {keys:?}");
        }).unwrap().join().unwrap();
    }

    #[test]
    fn noncanonical_or_wrong_version_declarations_keep_their_dart_bodies() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let library = directory.path().join("pkg/lib/src");
            std::fs::create_dir_all(&library).unwrap();
            let config = directory.path().join("packages.json");
            std::fs::write(&config, r#"{"configVersion":2,"packages":[{"name":"dartforge","rootUri":"pkg/","packageUri":"lib/","languageVersion":"3.6"}]}"#).unwrap();
            let entry = directory.path().join("main.dart");
            for (file, version, callback, timeout) in [
                ("other.dart", "v1", "Function", "int"),
                ("sync_native_callable.dart", "v2", "Function", "int"),
                ("sync_native_callable.dart", "v1", "Object", "int"),
                ("sync_native_callable.dart", "v1", "Function", "double"),
            ] {
                std::fs::write(library.join(file), format!("import 'dart:ffi'; @pragma('dartforge:sync-callback-{version}') bool _supportsSyncNativeCallableV1() => false; @pragma('dartforge:sync-callback-{version}') NativeCallable<T> _createSyncNativeCallableV1<T extends Function>({callback} callback, Object? exceptionalReturn, {timeout} timeout) => throw UnsupportedError('fallback'); NativeCallable<Int32 Function(Int32)> run() => _createSyncNativeCallableV1<Int32 Function(Int32)>((int v) => v, 17, 731); bool support() => _supportsSyncNativeCallableV1();")).unwrap();
                std::fs::write(&entry, format!("import 'package:dartforge/src/{file}'; void main() {{ print(support()); run().close(); }}")).unwrap();
                let module = lower(&entry, &config);
                assert!(!module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallStatic { symbol, .. } if symbol.ends_with("._dartforgeCallableSync"))), "{file}, {version}, {callback}, {timeout}");
                if file != "sync_native_callable.dart" || version != "v1" {
                    let support = module.functions.iter().find(|f| f.name == "_supportsSyncNativeCallableV1").unwrap();
                    assert!(!matches!(support.blocks[0].terminator, Terminator::Return(Some(Operand::Constant(Constant::Bool(true))))));
                }
            }
        }).unwrap().join().unwrap();
    }
}
