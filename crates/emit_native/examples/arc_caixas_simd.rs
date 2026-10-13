//! Prova AOT das caixas SIMD ARC: palavras da ABI, bits e morte após release.
//! Tracing preserva a fábrica antiga; o harness adquire seu owner explicitamente.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, fmt::Write, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") | None => false,
        Some(other) => return Err(format!("modo desconhecido: {other}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(other) => return Err(format!("nível desconhecido: {other}")),
    };
    let release = match args.next().as_deref() {
        None => true,
        Some("sem-liberar") => false,
        Some(other) => return Err(format!("controle desconhecido: {other}")),
    };
    if args.next().is_some() {
        return Err("argumentos excedentes".into());
    }
    let types = [Type::V4F32, Type::V4I32, Type::V2F64];
    let mut m = Module::new();
    m.arc_memory = arc;
    m.entry_symbol = Some("prova_simd".into());
    for (n, ty) in types.iter().enumerate() {
        let s = format!("caixa{n}");
        m.dart_ref_returns.insert(s.clone());
        m.functions.push(Function {
            symbol: s.clone(),
            name: s,
            debug_info: None,
            params: vec![(ValueId(0), "pistas".into(), *ty)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::Box {
                        op: Operand::Val(ValueId(0)),
                        from: *ty,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        });
    }
    let mut plans: HashMap<_, _> = m
        .functions
        .iter()
        .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
        .collect();
    prepare_arc_dart_module(&mut m, &mut plans)?;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    for (name, decl) in [
        ("dartforge_arc_observar_heap_v1", "declare i8 @dartforge_arc_observar_heap_v1(i64)"),
        ("dartforge_arc_collect", "declare void @dartforge_arc_collect()"),
        ("dartforge_print_handle", "declare void @dartforge_print_handle(i64)"),
        ("dartforge_arc_retain", "declare void @dartforge_arc_retain(i64)"),
        ("dartforge_arc_release", "declare void @dartforge_arc_release(i64)"),
        ("llvm.trap", "declare void @llvm.trap()"),
    ] {
        if !ir.lines().any(|l| l.starts_with("declare ") && l.contains(&format!("@{name}("))) {
            writeln!(ir, "{decl}").unwrap();
        }
    }
    // Nenhum vetor ou endereço observado é raiz: as caixas precisam ter
    // owner real. A leitura dos bits acontece antes da liberação.
    ir.push_str("define void @prova_simd() {\nentry:\n");
    for (n, ty) in types.iter().enumerate() {
        writeln!(ir, "  %vetor{n} = bitcast <2 x i64> <i64 -9223372036854775808, i64 9221120237041090851> to {}", ty.llvm_ir()).unwrap();
        writeln!(
            ir,
            "  %caixa{n} = call i64 @caixa{n}({} %vetor{n})",
            ty.llvm_ir()
        )
        .unwrap();
        if !arc {
            writeln!(ir, "  call void @dartforge_arc_retain(i64 %caixa{n})").unwrap();
        }
        writeln!(ir, "  %estado{n} = call i8 @dartforge_arc_observar_heap_v1(i64 %caixa{n})\n  %owned{n} = icmp eq i8 %estado{n}, 3").unwrap();
        writeln!(ir, "  %dados{n} = add i64 %caixa{n}, 14\n  %ptr{n} = inttoptr i64 %dados{n} to ptr\n  %bits{n} = load <2 x i64>, ptr %ptr{n}, align 8").unwrap();
        writeln!(ir, "  %lo{n} = extractelement <2 x i64> %bits{n}, i32 0\n  %hi{n} = extractelement <2 x i64> %bits{n}, i32 1\n  %oklo{n} = icmp eq i64 %lo{n}, -9223372036854775808\n  %okhi{n} = icmp eq i64 %hi{n}, 9221120237041090851").unwrap();
        writeln!(ir, "  %okbits{n} = and i1 %oklo{n}, %okhi{n}\n  %ok{n} = and i1 %owned{n}, %okbits{n}\n  br i1 %ok{n}, label %liberar{n}, label %falha\nliberar{n}:").unwrap();
        if release {
            writeln!(ir, "  call void @dartforge_arc_release(i64 %caixa{n})").unwrap();
        }
        writeln!(ir, "  call void @dartforge_arc_collect()\n  %final{n} = call i8 @dartforge_arc_observar_heap_v1(i64 %caixa{n})\n  %morta{n} = icmp eq i8 %final{n}, 0\n  br i1 %morta{n}, label %passou{n}, label %falha\npassou{n}:").unwrap();
    }
    ir.push_str("  call void @dartforge_print_handle(i64 3)\n  ret void\nfalha:\n  call void @llvm.trap()\n  unreachable\n}\n");
    if let Some(dir) = exit.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(exit.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &exit,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )
    .map(|_| ())
}
