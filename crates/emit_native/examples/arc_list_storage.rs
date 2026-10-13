//! AOT proof of Ref storage, compact writes and the owner before the edge replacement.
//! The HIR body is prepared under ARC; tracing runs the same explicit protocol.
//! Negative controls remove retention or release only after verification.
//! `--grow` forces growth and decompaction with a collection during the call.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, fmt::Write, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("provide the output executable")?);
    let arc = match args.next().as_deref() {
        Some("--memory=arc") => true,
        Some("--memory=tracing") | None => false,
        Some(other) => return Err(format!("unknown mode: {other}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(other) => return Err(format!("unknown level: {other}")),
    };
    let first_one = args.next();
    let grow = first_one.as_deref() == Some("--grow");
    let control = if grow { args.next() } else { first_one };
    if args.next().is_some() {
        return Err("argumentos excedentes".into());
    }
    let val = |n| Operand::Val(ValueId(n));
    let mut m = Module::new();
    m.arc_memory = true;
    m.entry_symbol = Some("prova_listas".into());
    m.dart_ref_returns.insert("trocar_armazenamento".into());
    let call = |name: &str, args: Vec<(Operand, Type)>, ret_ty| Instruction::CallRuntime {
        name: name.into(),
        args,
        ret_ty,
    };
    m.functions.push(Function {
        symbol: "trocar_armazenamento".into(),
        name: "trocar_armazenamento".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "lista".into(), Type::Ref),
            (ValueId(1), "novo".into(), Type::Ref),
            (ValueId(2), "bits".into(), Type::I64),
            (ValueId(3), "forma".into(), Type::I64),
        ],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(4),
                    call(
                        "dartforge_arc_list_storage_ref_v1",
                        vec![(val(0), Type::Ref)],
                        Type::Ref,
                    ),
                    Type::Ref,
                ),
                (
                    ValueId(5),
                    call(
                        "dartforge_arc_list_write_bits_v1",
                        vec![
                            (val(4), Type::Ref),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                            (val(2), Type::I64),
                            (val(3), Type::I64),
                        ],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(6),
                    call(
                        "dartforge_arc_list_write_data_v1",
                        vec![(val(0), Type::Ref), (val(1), Type::Ref)],
                        Type::I64,
                    ),
                    Type::I64,
                ),
                (
                    ValueId(7),
                    call(
                        "dartforge_arc_list_write_len_v1",
                        vec![
                            (val(0), Type::Ref),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                        ],
                        Type::Void,
                    ),
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(Some(val(4))),
        }],
    });
    if grow {
        let insts = &mut m.functions[0].blocks[0].instructions;
        insts[2] = (
            ValueId(6),
            call(
                "dartforge_list_append",
                vec![(val(0), Type::Ref), (val(1), Type::Ref)],
                Type::Ref,
            ),
            Type::Ref,
        );
        insts.remove(3);
    }
    let mut plan = DartFunctionPlan::default();
    plan.tokens.ret = TokenReturn::Owned;
    for v in [ValueId(2), ValueId(3)] {
        plan.classes.insert(v, Ownership::Trivial);
    }
    let mut plans = HashMap::from([("trocar_armazenamento".into(), plan)]);
    if prepare_arc_dart_module(&mut m, &mut plans)? != (1, 0) {
        return Err("expected an owning copy of the storage before the mutation".into());
    }
    let borrowed = m.functions[0].blocks[0]
        .instructions
        .iter()
        .find_map(|(v, i, _)| match i {
            Instruction::ArcCopy {
                value: Operand::Val(of),
            } if *v == ValueId(4) => Some(of.0),
            _ => None,
        })
        .ok_or("missing storage copy")?;
    m.arc_memory = arc;
    if !arc {
        m.arc_aot_ports = None;
    }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    if !ir
        .lines()
        .any(|l| l.starts_with("declare ") && l.contains("@dartforge_print_handle("))
    {
        ir.push_str("declare void @dartforge_print_handle(i64)\n");
    }
    if !ir.contains("declare void @llvm.trap()") {
        ir.push_str("declare void @llvm.trap()\n");
    }
    ir.push_str("define void @prova_listas() {\nentry:\n  %palavra = alloca i64, align 8\n  store i64 0, ptr %palavra\n");
    for (n, code, shape, bits) in [
        (0, 1, 0x22, i64::MIN),
        (1, 2, 0x42, 0xfff8_1234_5678_9abc_u64 as i64),
        (2, 3, 0x62, 1),
    ] {
        writeln!(ir, "  %pai{n} = call i64 @dartforge_list_new(ptr %palavra, i64 1, i64 10, i64 {code})\n  call void @dartforge_arc_retain(i64 %pai{n})\n  %novo{n} = call i64 @dartforge_list_new(ptr null, i64 0, i64 8, i64 {code})\n  call void @dartforge_arc_retain(i64 %novo{n})\n  %retorno{n} = call i64 @trocar_armazenamento(i64 %pai{n}, i64 %novo{n}, i64 {bits}, i64 {shape})").unwrap();
        writeln!(ir, "  %len{n} = call i64 @dartforge_arc_list_len_v1(i64 %pai{n})\n  %cap{n} = call i64 @dartforge_arc_list_capacity_v1(i64 %pai{n})\n  %lenzero{n} = icmp eq i64 %len{n}, 0\n  %capzero{n} = icmp eq i64 %cap{n}, 0\n  %encolheu{n} = and i1 %lenzero{n}, %capzero{n}\n  br i1 %encolheu{n}, label %coletar{n}, label %falha\ncoletar{n}:").unwrap();
        writeln!(ir, "  call void @dartforge_arc_release(i64 %pai{n})\n  call void @dartforge_arc_release(i64 %novo{n})\n  call void @dartforge_arc_collect()\n  %estado{n} = call i8 @dartforge_arc_observe_heap_v1(i64 %retorno{n})\n  %vivo{n} = icmp eq i8 %estado{n}, 3\n  br i1 %vivo{n}, label %ler{n}, label %falha\nler{n}:").unwrap();
        // The raw observation only happens after proving that the owner kept the block.
        writeln!(ir, "  %endereco{n} = add i64 %retorno{n}, 22\n  %ptr{n} = inttoptr i64 %endereco{n} to ptr\n  %bits{n} = load i64, ptr %ptr{n}, align 8\n  %iguais{n} = icmp eq i64 %bits{n}, {bits}\n  br i1 %iguais{n}, label %liberar{n}, label %falha\nliberar{n}:\n  call void @dartforge_arc_release(i64 %retorno{n})\n  call void @dartforge_arc_collect()\n  %final{n} = call i8 @dartforge_arc_observe_heap_v1(i64 %retorno{n})\n  %morto{n} = icmp eq i8 %final{n}, 0\n  br i1 %morto{n}, label %passou{n}, label %falha\npassou{n}:").unwrap();
    }
    ir.push_str("  call void @dartforge_print_handle(i64 3)\n  ret void\nfalha:\n  call void @llvm.trap()\n  unreachable\n}\n");
    if grow {
        for n in 0..3 {
            ir = ir.replace(
                &format!("icmp eq i64 %len{n}, 0"),
                &format!("icmp eq i64 %len{n}, 2"),
            );
            ir = ir.replace(
                &format!("icmp eq i64 %cap{n}, 0"),
                &format!("icmp eq i64 %cap{n}, 3"),
            );
            let release = format!(
                "  call void @dartforge_arc_release(i64 %pai{n})\n  call void @dartforge_arc_release(i64 %novo{n})\n"
            );
            if ir.matches(&release).count() != 1 {
                return Err("release of the pair not located uniquely".into());
            }
            // The appended value must live only through the copied edge, before killing the list.
            let check = format!(
                "  call void @dartforge_arc_release(i64 %novo{n})\n  call void @dartforge_arc_collect()\n  %forma{n} = call i64 @dartforge_arc_list_shape_v1(i64 %pai{n})\n  %geral{n} = icmp eq i64 %forma{n}, 4\n  %filhoestado{n} = call i8 @dartforge_arc_observe_heap_v1(i64 %novo{n})\n  %filhovivo{n} = icmp eq i8 %filhoestado{n}, 1\n  %aresta{n} = and i1 %geral{n}, %filhovivo{n}\n  br i1 %aresta{n}, label %conferir{n}, label %falha\nconferir{n}:\n  %ultimo{n} = call i64 @dartforge_arc_list_ref_owned_v1(i64 %pai{n}, i64 1)\n  %elemento{n} = icmp eq i64 %ultimo{n}, %novo{n}\n  call void @dartforge_arc_release(i64 %ultimo{n})\n  br i1 %elemento{n}, label %soltarpai{n}, label %falha\nsoltarpai{n}:\n  call void @dartforge_arc_release(i64 %pai{n})\n"
            );
            ir = ir.replace(&release, &check);
        }
    }
    match control.as_deref() {
        None => {}
        Some("no-retain") => {
            let call = format!("  call void @dartforge_arc_retain(i64 %v{borrowed})\n");
            if ir.matches(&call).count() != 1 {
                return Err("retention not located uniquely".into());
            }
            ir = ir.replace(&call, "");
        }
        Some("no-release") => {
            for n in 0..3 {
                ir = ir.replace(
                    &format!("  call void @dartforge_arc_release(i64 %retorno{n})\n"),
                    "",
                );
            }
        }
        Some(other) => return Err(format!("unknown control: {other}")),
    }
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
