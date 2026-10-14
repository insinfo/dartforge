//! Fold host platform flags only after proving the SDK's native getter chain.

use super::fn_builder::FnBuilder;
use crate::{context::Context, hir::{Constant, Instruction, Operand, Type}};
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, ExprKind, FunctionBody};

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub(super) fn read_platform_flag(&mut self, variable: VariableId) -> Option<Operand> {
        if std::env::var("DARTFORGE_NO_PLATFORM_SPECIALIZATION").as_deref() == Ok("1") {
            return None;
        }
        let value = proven_flag(self.ctx, variable)?;
        let value = self.emit(Instruction::Const(Constant::Bool(value)), Type::I1);
        let repr = self.repr(super::members::variable_type(self.ctx, variable));
        Some(self.coerce(value, repr))
    }
}

fn host_name() -> Option<&'static str> {
    // target_os() currently treats unsupported hosts as Linux; the runtime does not.
    if cfg!(target_os = "macos") { Some("macos") }
    else if cfg!(target_os = "windows") { Some("windows") }
    else if cfg!(target_os = "linux") { Some("linux") }
    else { None }
}

fn field(ctx: &Context, class: ClassId, name: &str) -> Option<VariableId> {
    let symbol = ctx.interner.lookup(name)?;
    let function = *ctx.program.class(class).static_members.get(&symbol)?;
    let function = ctx.program.function(function);
    if function.patched_by.is_some() { return None; }
    let variable = function.variable?;
    let v = ctx.program.variable(variable);
    (v.class == Some(class) && v.static_ && v.final_ && !v.late && !v.external).then_some(variable)
}

fn initializer(ctx: &Context, variable: VariableId) -> Option<(UnitId, ast::ExprId)> {
    let VariableRef::Field { unit, .. } = ctx.program.variable(variable).node else { return None; };
    Some((unit, super::layouts_arc::initializer(ctx, variable)?))
}

fn unparenthesized(ctx: &Context, unit: UnitId, mut expression: ast::ExprId) -> ast::ExprId {
    for _ in 0..32 {
        if let ExprKind::Parenthesized(inner) = ctx.program.unit(unit).ast.expr(expression).kind { expression = inner; }
        else { break; }
    }
    expression
}

fn named(ctx: &Context, unit: UnitId, expression: ast::ExprId, name: &str) -> bool {
    matches!(&ctx.program.unit(unit).ast.expr(expression).kind,
        ExprKind::Identifier(identifier) if ctx.symbol_name(identifier.sym) == name)
}

fn method(ctx: &Context, class: ClassId, name: &str) -> Option<FunctionElementId> {
    let symbol = ctx.interner.lookup(name)?;
    let mut function = *ctx.program.class(class).static_members.get(&symbol)?;
    // A custom SDK patch may replace the declaration; inspect its actual body.
    for _ in 0..8 {
        let f = ctx.program.function(function);
        if let Some(patch) = f.patched_by { function = patch; continue; }
        let owner = f.class?;
        if ctx.program.class_representative(owner) != ctx.program.class_representative(class)
            || !f.static_ || f.variable.is_some() { return None; }
        return Some(function);
    }
    None
}

fn proven_flag(ctx: &Context, variable: VariableId) -> Option<bool> {
    let host = host_name()?;
    let platform = ctx.sdk_class("io", "Platform")?;
    let class = ctx.program.class(platform);
    if !class.type_params.is_empty() { return None; }
    let name = ctx.symbol_name(ctx.program.variable(variable).name);
    if !matches!(name, "isLinux" | "isMacOS" | "isWindows" | "isAndroid" | "isIOS" | "isFuchsia")
        || field(ctx, platform, name)? != variable
        || super::members::variable_type(ctx, variable) != ctx.core.bool_ { return None; }
    let (unit, expression) = initializer(ctx, variable)?;
    let ast = &ctx.program.unit(unit).ast;
    let ExprKind::Binary { op: ast::BinaryOp::Eq, left, right } = &ast.expr(unparenthesized(ctx, unit, expression)).kind else { return None; };
    if !named(ctx, unit, *left, "operatingSystem") { return None; }
    let ExprKind::String(literal) = &ast.expr(unparenthesized(ctx, unit, *right)).kind else { return None; };
    let expected = literal.constant_value()?;
    let expected = expected.as_str()?;
    let os = field(ctx, platform, "operatingSystem")?;
    let VariableRef::Field { unit: os_unit, member: os_member, .. } = ctx.program.variable(os).node else { return None; };
    let ast::MemberKind::Field(declaration) = &ctx.program.unit(os_unit).ast.member(os_member).kind else { return None; };
    // App inference leaves untyped SDK fields dynamic. The native chain below
    // proves their actual String value; an explicit incompatible type is refused.
    if declaration.ty.is_some() && super::members::variable_type(ctx, os) != ctx.core.string { return None; }
    let (unit, expression) = initializer(ctx, os)?;
    let ExprKind::Property { target, name, null_aware: false } = &ctx.program.unit(unit).ast.expr(expression).kind else { return None; };
    if ctx.symbol_name(name.sym) != "operatingSystem" || !named(ctx, unit, *target, "_Platform") { return None; }
    // In a static initializer, a member would shadow the library's class name.
    if class.static_members.contains_key(&ctx.interner.lookup("_Platform")?) { return None; }
    let library = ctx.program.library(class.library);
    let binding = library.scope.get(&ctx.interner.lookup("_Platform")?)?;
    if binding.ambiguous { return None; }
    let Some(Element::Class(private)) = binding.getter else { return None; };
    let canonical = ctx.sdk_class("io", "_Platform")?;
    if ctx.program.class_representative(private) != ctx.program.class_representative(canonical)
        || !ctx.program.class(private).type_params.is_empty() { return None; }
    let getter = method(ctx, private, "operatingSystem")?;
    let f = ctx.program.function(getter);
    if f.kind != FunctionKind::Getter || f.external
        || ctx.outline.functions.get(getter.0 as usize)?.return_type != ctx.core.string { return None; }
    let FunctionRef::Function { unit, function } = f.node else { return None; };
    let ast = &ctx.program.unit(unit).ast;
    let f = ast.function(function);
    if f.modifier != ast::AsyncModifier::None || !f.type_params.is_empty() || f.parameters.is_some() { return None; }
    let FunctionBody::Expression(expression) = f.body else { return None; };
    let ExprKind::Call { target: callee, arguments, .. } = &ast.expr(expression).kind else { return None; };
    if !arguments.args.is_empty() || !named(ctx, unit, *callee, "_operatingSystem") { return None; }
    let native = method(ctx, private, "_operatingSystem")?;
    let f = ctx.program.function(native);
    if !f.external || f.kind != FunctionKind::Function
        || ctx.outline.functions.get(native.0 as usize)?.return_type != ctx.core.string { return None; }
    let FunctionRef::Function { unit, function } = f.node else { return None; };
    let f = ctx.program.unit(unit).ast.function(function);
    if f.modifier != ast::AsyncModifier::None || !f.type_params.is_empty() || f.parameters.as_ref().is_none_or(|p| !p.is_empty())
        || !matches!(f.body, FunctionBody::Empty | FunctionBody::Native(_)) { return None; }
    let (native_name, _) = crate::source::native_pragma(ctx.program, ctx.interner, native.0 as usize);
    if native_name.as_deref() != Some("Platform_OperatingSystem") { return None; }
    Some(host == expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_types::{CoreTypes, TypeTable};

    fn inspect(mut edit: impl FnMut(&mut dartforge_elements::model::Program, &mut dartforge_intern::Interner), check: impl FnOnce(&Context)) {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/platform_flags/main.dart");
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let mut program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
        edit(&mut program, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, library)| !library.is_sdk)
            .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(&program, &interner, &mut table, &core, &mut outline, &libraries);
        let context = Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
        check(&context);
    }

    #[test]
    fn canonical_flags_are_proven_without_inferring_sdk_bodies() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| inspect(|_, _| {}, |ctx| {
            let platform = ctx.sdk_class("io", "Platform").unwrap();
            for (name, operating_system) in [("isWindows", "windows"), ("isMacOS", "macos"), ("isLinux", "linux"), ("isAndroid", "android"), ("isIOS", "ios"), ("isFuchsia", "fuchsia")] {
                let flag = field(ctx, platform, name).unwrap();
                assert_eq!(proven_flag(ctx, flag), Some(host_name().unwrap() == operating_system), "{name}");
            }
            let module = crate::lower::lower_program(ctx);
            assert!(module.errors.is_empty(), "{:?}", module.errors);
            for (name, value) in [("windowsBranch", host_name() == Some("windows")), ("macBranch", host_name() == Some("macos")), ("forwardedFlag", host_name() == Some("macos")), ("aliasFlag", host_name() == Some("macos"))] {
                let function = module.functions.iter().find(|function| function.symbol.ends_with(&format!(".{name}"))).unwrap();
                assert!(function.blocks.iter().flat_map(|block| &block.instructions).any(|instruction| matches!(&instruction.1, Instruction::Const(Constant::Bool(actual)) if *actual == value)), "{}", function.symbol);
            }
            let user = ctx.program.classes.iter().position(|class| ctx.symbol_name(class.name) == "Platform" && !ctx.program.library(class.library).is_sdk).unwrap();
            let user = ClassId(user as u32);
            assert_eq!(proven_flag(ctx, field(ctx, user, "isWindows").unwrap()), None);
        })).unwrap().join().unwrap();
    }
    fn sdk_flag(program: &dartforge_elements::model::Program, interner: &dartforge_intern::Interner, name: &str) -> VariableId {
        let class = program.classes.iter().find(|class| interner.resolve(class.name) == "Platform" && program.library(class.library).uri == "dart:io").unwrap();
        *class.fields.iter().find(|&&variable| interner.resolve(program.variable(variable).name) == name).unwrap()
    }

    #[test]
    fn initializer_and_native_annotation_changes_keep_fallback() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            inspect(|program, interner| {
                let flag = sdk_flag(program, interner, "isWindows");
                let VariableRef::Field { unit, member, index } = program.variable(flag).node else { panic!() };
                let ast::MemberKind::Field(declaration) = &program.unit(unit).ast.member(member).kind else { panic!() };
                let expression = declaration.variables[index].initializer.unwrap();
                program.units[unit.0 as usize].ast.exprs[expression.0 as usize].kind = ExprKind::Bool(false);
            }, |ctx| {
                let platform = ctx.sdk_class("io", "Platform").unwrap();
                assert_eq!(proven_flag(ctx, field(ctx, platform, "isWindows").unwrap()), None);
                assert!(proven_flag(ctx, field(ctx, platform, "isMacOS").unwrap()).is_some());
            });
            inspect(|program, _| {
                for unit in &mut program.units {
                    for expression in &mut unit.ast.exprs {
                        if let ExprKind::String(literal) = &mut expression.kind
                            && literal.constant_value().is_some_and(|value| value.as_bytes() == b"Platform_OperatingSystem") {
                            literal.parts = vec![ast::StringPart::Text("Platform_OperatingSystemVersion".into())].into_boxed_slice();
                        }
                    }
                }
            }, |ctx| {
                let platform = ctx.sdk_class("io", "Platform").unwrap();
                assert_eq!(proven_flag(ctx, field(ctx, platform, "isMacOS").unwrap()), None);
            });
        }).unwrap().join().unwrap();
    }

    #[test]
    fn sdk_getter_wrapper_effects_are_not_erased() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| inspect(|program, interner| {
            let function = program.functions.iter().find(|function| interner.resolve(function.name) == "operatingSystem"
                && function.kind == FunctionKind::Getter && function.variable.is_none()
                && function.class.is_some_and(|class| interner.resolve(program.class(class).name) == "_Platform")
                && program.library(function.library).uri == "dart:io").unwrap();
            let FunctionRef::Function { unit, function } = function.node else { panic!() };
            let print = interner.intern("print");
            let ast = &mut program.units[unit.0 as usize].ast;
            let span = ast.function(function).span;
            let FunctionBody::Expression(original) = ast.function(function).body else { panic!() };
            let target = ast::ExprId(ast.exprs.len() as u32);
            ast.exprs.push(ast::Expr { span, kind: ExprKind::Identifier(ast::Name { sym: print, span }) });
            let argument = ast::ExprId(ast.exprs.len() as u32);
            ast.exprs.push(ast::Expr { span, kind: ExprKind::String(ast::StringLit { span, parts: vec![ast::StringPart::Text("platform wrapper".into())].into_boxed_slice() }) });
            let call = ast::ExprId(ast.exprs.len() as u32);
            ast.exprs.push(ast::Expr { span, kind: ExprKind::Call { target, arguments: Box::new(ast::Arguments { span, type_args: Box::new([]), args: vec![ast::Argument { name: None, value: argument }].into_boxed_slice() }) } });
            let effect = ast::StmtId(ast.stmts.len() as u32);
            ast.stmts.push(ast::Stmt { span, kind: ast::StmtKind::Expression(call) });
            let result = ast::StmtId(ast.stmts.len() as u32);
            ast.stmts.push(ast::Stmt { span, kind: ast::StmtKind::Return(Some(original)) });
            let block = ast::StmtId(ast.stmts.len() as u32);
            ast.stmts.push(ast::Stmt { span, kind: ast::StmtKind::Block(vec![effect, result].into_boxed_slice()) });
            ast.functions[function.0 as usize].body = FunctionBody::Block(block);
        }, |ctx| {
            let platform = ctx.sdk_class("io", "Platform").unwrap();
            assert_eq!(proven_flag(ctx, field(ctx, platform, "isWindows").unwrap()), None);
        })).unwrap().join().unwrap();
    }

}
