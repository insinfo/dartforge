//! AOT proof of keepalive produced before replacing a field edge.
//! The verified HIR reads BorrowArg and returns Owned after removing the strong edge.
//! The LLVM harness observes the addresses without roots, after the function has left.
//! Under tracing, the same body with explicit tokens tests the compatibility of the
//! runtime protocol; it does not enable the ARC preparation in the default tracing pipeline.
//! Does not certify source lowering, pending guards, reload or suspension.
//! The negative controls `no-retain` and `no-release` change only the final IR
//! of the harness, after the check; the execution must fail.

#[path = "support/arc_symbols.rs"]
mod arc_symbols;

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone, Copy)]
enum Scenario {
    Field,
    Capture,
    Neighbor,
    CaptureNeighbor,
    Argument,
    Nullable,
    Mixed,
    Cycle,
    Cell,
    Context,
    Global,
    Late,
}

fn module(scenario: Scenario) -> Result<Module, String> {
    let capture = matches!(scenario, Scenario::Capture | Scenario::CaptureNeighbor);
    let neighbor = matches!(scenario, Scenario::Neighbor | Scenario::CaptureNeighbor);
    let argument = matches!(scenario, Scenario::Argument);
    let nullable = matches!(
        scenario,
        Scenario::Nullable | Scenario::Mixed | Scenario::Cycle
    );
    let mixed = matches!(scenario, Scenario::Mixed);
    let cell = matches!(scenario, Scenario::Cell);
    let global = matches!(scenario, Scenario::Global);
    let mut m = Module::new();
    m.arc_memory = true;
    m.entry_symbol = Some("prova_keepalive".into());
    let mut layout = vec![Type::Ref; 40];
    layout[..4].copy_from_slice(&[Type::I64, Type::F64, Type::I1, Type::I8]);
    m.arc_field_layouts.insert(123, layout.clone());
    if cell {
        m.arc_field_layouts
            .insert(dartforge_runtime::layout::cid::CELL as u32, vec![Type::Ref]);
    }
    if capture {
        m.arc_field_layouts
            .insert(dartforge_runtime::layout::cid::CONTEXT as u32, layout);
    }
    for id in m.arc_field_layouts.keys() {
        m.classes.push(ClassDef {
            id: *id,
            name: format!("C{id}"),
            field_count: m.arc_field_layouts[id].len(),
            vtable: vec![],
            to_string_symbol: None,
        });
    }
    let val = |n| Operand::Val(ValueId(n));
    m.functions.push(Function {
        symbol: "ler_e_substituir".into(),
        name: "ler_e_substituir".into(),
        debug_info: None,
        params: vec![(ValueId(0), "receiver".into(), Type::Ref)],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::GetField {
                        object: val(0),
                        index: 39,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::SetField {
                        object: val(0),
                        index: 39,
                        value: Operand::Constant(Constant::Null),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(Some(val(1))),
        }],
    });
    if global {
        m.globals.push((0, Type::Ref, "dfg.prova_keepalive".into()));
        let f = &mut m.functions[0];
        f.params.clear();
        for (_, inst, _) in &mut f.blocks[0].instructions {
            if let Instruction::GetField { object, .. } | Instruction::SetField { object, .. } =
                inst
            {
                *object = val(9);
            }
        }
        f.blocks[0].instructions.splice(
            0..0,
            [
                (
                    ValueId(9),
                    Instruction::LoadGlobal {
                        symbol: "dfg.prova_keepalive".into(),
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(10),
                    Instruction::StoreGlobal {
                        symbol: "dfg.prova_keepalive".into(),
                        val: Operand::Constant(Constant::Null),
                        ty: Type::Ref,
                        root: Some(0),
                    },
                    Type::Void,
                ),
            ],
        );
    }
    if cell {
        m.functions[0].blocks[0].instructions[0].1 = Instruction::CellGet { cell: val(0) };
        m.functions[0].blocks[0].instructions[1].1 = Instruction::SetField {
            object: val(0),
            index: 0,
            value: Operand::Constant(Constant::Null),
        };
    }
    if matches!(scenario, Scenario::Late) {
        m.functions[0].blocks[0].instructions[0].1 = Instruction::CallRuntime {
            name: "dartforge_arc_read_field_late_ref_v1".into(),
            args: vec![
                (val(0), Type::Ref),
                (Operand::Constant(Constant::Int(39)), Type::I64),
            ],
            ret_ty: Type::Ref,
        };
    }
    if matches!(scenario, Scenario::Context) {
        let f = &mut m.functions[0];
        for (_, inst, _) in &mut f.blocks[0].instructions {
            if let Instruction::GetField { object, .. } | Instruction::SetField { object, .. } =
                inst
            {
                *object = val(9);
            }
        }
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(9),
                Instruction::CallRuntime {
                    name: "dartforge_arc_closure_context_v1".into(),
                    args: vec![(val(0), Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
        );
    }
    if capture {
        // Exercises the EnvGet translation and the real map of the environment, including
        // field 39, before replacing the edge that sustains the borrow.
        m.functions[0].blocks[0].instructions[0].1 = Instruction::EnvGet {
            env: val(0),
            index: 39,
        };
    }
    if neighbor {
        // The child depends on the neighbor and this one on the root. Removing the edge of the
        // root must preserve both until the mutation ends and the child is returned.
        m.functions[0].blocks[0].instructions = vec![
            (
                ValueId(1),
                Instruction::GetField {
                    object: val(0),
                    index: 39,
                },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::GetField {
                    object: val(1),
                    index: 39,
                },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::SetField {
                    object: val(0),
                    index: 39,
                    value: Operand::Constant(Constant::Null),
                },
                Type::Void,
            ),
            (
                ValueId(4),
                Instruction::SetField {
                    object: val(1),
                    index: 39,
                    value: Operand::Constant(Constant::Null),
                },
                Type::Void,
            ),
        ];
        m.functions[0].blocks[0].terminator = Terminator::Return(Some(val(2)));
        if capture {
            m.functions[0].blocks[0].instructions[0].1 = Instruction::EnvGet {
                env: val(0),
                index: 39,
            };
        }
    }
    if argument {
        let f = &mut m.functions[0];
        f.params.extend([
            (ValueId(100), "args".into(), Type::Ptr),
            (ValueId(101), "desc".into(), Type::Ptr),
        ]);
        for (_, i, _) in &mut f.blocks[0].instructions {
            if let Instruction::GetField { object, .. } | Instruction::SetField { object, .. } = i {
                *object = val(9);
            }
        }
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(9),
                Instruction::LoadIndexed {
                    base: val(100),
                    index: Operand::Constant(Constant::Int(0)),
                },
                Type::Ref,
            ),
        );
        m.dart_vector_parameters.insert(
            "ler_e_substituir".into(),
            HashMap::from([
                (ValueId(100), DartEntryVector::Arguments),
                (ValueId(101), DartEntryVector::Descriptor),
            ]),
        );
    }
    if nullable {
        let f = &mut m.functions[0];
        f.params.push((ValueId(100), "presente".into(), Type::I1));
        let mut body = f.blocks[0].clone();
        body.id = BlockId(5);
        for (_, i, _) in &mut body.instructions {
            if let Instruction::GetField { object, .. } | Instruction::SetField { object, .. } = i {
                *object = val(105);
            }
        }
        body.instructions.insert(
            0,
            (
                ValueId(107),
                Instruction::GetField {
                    object: val(105),
                    index: 2,
                },
                Type::I1,
            ),
        );
        f.blocks = vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::CondBranch {
                    cond: val(100),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(104),
                    Instruction::ArcCopy { value: val(0) },
                    Type::Ref,
                )],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![(ValueId(108), Instruction::Const(Constant::Null), Type::Ref)],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![
                    (
                        ValueId(105),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(1), val(104)), (BlockId(2), val(108))],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(106),
                        Instruction::ICmp(ICmpOp::Eq, val(105), Operand::Constant(Constant::Null)),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: val(106),
                    then_block: BlockId(4),
                    else_block: BlockId(5),
                },
            },
            BasicBlock {
                id: BlockId(4),
                instructions: vec![],
                terminator: Terminator::Return(Some(val(105))),
            },
            body,
        ];
        if mixed {
            let mut fields = vec![Operand::Constant(Constant::Null); 40];
            fields[..4].clone_from_slice(&[
                Operand::Constant(Constant::Int(42)),
                Operand::Constant(Constant::Double(-0.0)),
                Operand::Constant(Constant::Bool(true)),
                val(109),
            ]);
            f.blocks[2].instructions = vec![
                (
                    ValueId(109),
                    Instruction::Trunc {
                        op: Operand::Constant(Constant::Int(255)),
                        from: Type::I64,
                        to: Type::I8,
                    },
                    Type::I8,
                ),
                (
                    ValueId(108),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields,
                    },
                    Type::Ref,
                ),
            ];
        }
    }
    if matches!(scenario, Scenario::Cycle) {
        let f = &mut m.functions[0];
        let exit = f.blocks[3].terminator.clone();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 {
            incoming.push((BlockId(6), val(110)));
        }
        f.blocks[3].instructions.insert(
            1,
            (
                ValueId(111),
                Instruction::Phi {
                    ty: Type::I64,
                    incoming: vec![
                        (BlockId(1), Operand::Constant(Constant::Int(0))),
                        (BlockId(2), Operand::Constant(Constant::Int(0))),
                        (BlockId(6), val(112)),
                    ],
                },
                Type::I64,
            ),
        );
        f.blocks[3].instructions.push((
            ValueId(113),
            Instruction::ICmp(ICmpOp::Slt, val(111), Operand::Constant(Constant::Int(2))),
            Type::I1,
        ));
        f.blocks[3].terminator = Terminator::CondBranch {
            cond: val(113),
            then_block: BlockId(6),
            else_block: BlockId(7),
        };
        f.blocks.extend([
            BasicBlock {
                id: BlockId(6),
                instructions: vec![
                    (
                        ValueId(110),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(3), val(105))],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(112),
                        Instruction::Add(val(111), Operand::Constant(Constant::Int(1))),
                        Type::I64,
                    ),
                ],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(7),
                instructions: vec![],
                terminator: exit,
            },
        ]);
    }
    let mut plan = DartFunctionPlan::default();
    plan.tokens.ret = TokenReturn::Owned;
    m.functions.push(Function {
        symbol: "inicializar".into(),
        name: "inicializar".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "this".into(), Type::Ref),
            (ValueId(1), "filho".into(), Type::Ref),
        ],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::SetField {
                        object: val(0),
                        index: 39,
                        value: val(1),
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::Trunc {
                        op: Operand::Constant(Constant::Int(255)),
                        from: Type::I64,
                        to: Type::I8,
                    },
                    Type::I8,
                ),
                (
                    ValueId(4),
                    Instruction::SetField {
                        object: val(0),
                        index: 0,
                        value: Operand::Constant(Constant::Int(42)),
                    },
                    Type::Void,
                ),
                (
                    ValueId(5),
                    Instruction::SetField {
                        object: val(0),
                        index: 1,
                        value: Operand::Constant(Constant::Double(-0.0)),
                    },
                    Type::Void,
                ),
                (
                    ValueId(6),
                    Instruction::SetField {
                        object: val(0),
                        index: 2,
                        value: Operand::Constant(Constant::Bool(true)),
                    },
                    Type::Void,
                ),
                (
                    ValueId(7),
                    Instruction::SetField {
                        object: val(0),
                        index: 3,
                        value: val(3),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    });
    m.functions.push(Function {
        symbol: "ler_inteiro".into(),
        name: "ler_inteiro".into(),
        debug_info: None,
        params: vec![(ValueId(0), "this".into(), Type::Ref)],
        return_ty: Type::I64,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(1),
                Instruction::GetField {
                    object: val(0),
                    index: 0,
                },
                Type::I64,
            )],
            terminator: Terminator::Return(Some(val(1))),
        }],
    });
    if cell {
        m.functions[1].blocks[0].instructions = vec![(
            ValueId(2),
            Instruction::SetField {
                object: val(0),
                index: 0,
                value: val(1),
            },
            Type::Void,
        )];
    }
    let mut plans = HashMap::from([
        ("ler_e_substituir".into(), plan),
        ("inicializar".into(), DartFunctionPlan::default()),
        ("ler_inteiro".into(), DartFunctionPlan::default()),
    ]);
    if global {
        m.functions.push(Function {
            symbol: "guardar_global".into(),
            name: "guardar_global".into(),
            debug_info: None,
            params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::StoreGlobal {
                        symbol: "dfg.prova_keepalive".into(),
                        val: val(0),
                        ty: Type::Ref,
                        root: Some(0),
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        });
        plans.insert("guardar_global".into(), DartFunctionPlan::default());
    }
    let produced = prepare_arc_dart_module(&mut m, &mut plans)?;
    let expected = if neighbor {
        (2, 1)
    } else if argument || nullable || global {
        (1, 1)
    } else {
        (1, 0)
    };
    let ret = ValueId(if neighbor { 2 } else { 1 });
    if produced != expected || plans["ler_e_substituir"].classes[&ret] != Ownership::Owned {
        return Err(format!(
            "expected keepalive {expected:?}, found {produced:?}"
        ));
    }
    let prepared = format!("{m:?}");
    let prepared_plans = plans.clone();
    if prepare_arc_dart_module(&mut m, &mut plans)? != (0, 0)
        || format!("{m:?}") != prepared
        || plans != prepared_plans
    {
        return Err("keepalive preparation was not idempotent".into());
    }
    let errors = dartforge_emit_native::lower::verifier::verify(&m);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    if argument {
        m.arc_uniform_returns.insert("ler_e_substituir".into());
        certify_arc_aot_ports(&mut m, &plans)?;
    }
    Ok(m)
}

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
    let mut control = args.next();
    let scenario = match control.as_deref() {
        Some("--capture") => Scenario::Capture,
        Some("--neighbor") => Scenario::Neighbor,
        Some("--capture-neighbor") => Scenario::CaptureNeighbor,
        Some("--argument") => Scenario::Argument,
        Some("--nullable") => Scenario::Nullable,
        Some("--mixed") => Scenario::Mixed,
        Some("--cycle") => Scenario::Cycle,
        Some("--cell") => Scenario::Cell,
        Some("--context") => Scenario::Context,
        Some("--global") => Scenario::Global,
        Some("--late") => Scenario::Late,
        _ => Scenario::Field,
    };
    if !matches!(scenario, Scenario::Field) {
        control = args.next();
    }
    let capture = matches!(scenario, Scenario::Capture | Scenario::CaptureNeighbor);
    let neighbor = matches!(scenario, Scenario::Neighbor | Scenario::CaptureNeighbor);
    let argument = matches!(scenario, Scenario::Argument);
    if args.next().is_some() {
        return Err("argumentos excedentes".into());
    }
    let mut m = module(scenario)?;
    m.arc_memory = arc;
    if !arc {
        m.arc_aot_ports = None;
    }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // No observed Ref goes through the HIR of the harness: an already dead address
    // is only a native diagnostic. There is no allocation between death and observation,
    // avoiding confusing address reuse with survival.
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
define void @prova_keepalive() {
  %obj = call i64 @dartforge_arc_object_owned_v1(i64 123, i64 40)
  %filho = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775807)
  call void @inicializar(i64 %obj, i64 %filho)
  call void @dartforge_arc_release(i64 %filho)
  %aresta = call i8 @dartforge_arc_observe_heap_v1(i64 %filho)
  %ok_aresta = icmp eq i8 %aresta, 1
  %inteiro = call i64 @ler_inteiro(i64 %obj)
  %double = call i64 @dartforge_arc_read_field_scalar_v1(i64 %obj, i64 1)
  %bool = call i64 @dartforge_arc_read_field_scalar_v1(i64 %obj, i64 2)
  %byte = call i64 @dartforge_arc_read_field_scalar_v1(i64 %obj, i64 3)
  %ok_int = icmp eq i64 %inteiro, 42
  %ok_double = icmp eq i64 %double, -9223372036854775808
  %ok_bool = icmp eq i64 %bool, 1
  %ok_byte = icmp eq i64 %byte, 255
  %ok_a = and i1 %ok_int, %ok_double
  %ok_b = and i1 %ok_bool, %ok_byte
  %ok_campos = and i1 %ok_a, %ok_b
  %ok_inicio = and i1 %ok_campos, %ok_aresta
  br i1 %ok_inicio, label %ler, label %falha
ler:
  %retorno = call i64 @ler_e_substituir(i64 %obj)
  %mesmo = icmp eq i64 %retorno, %filho
  br i1 %mesmo, label %coletar, label %falha
coletar:
  call void @dartforge_arc_release(i64 %obj)
  call void @dartforge_arc_collect()
  %obj_morto = call i8 @dartforge_arc_observe_heap_v1(i64 %obj)
  %filho_vivo = call i8 @dartforge_arc_observe_heap_v1(i64 %retorno)
  %ok_obj = icmp eq i8 %obj_morto, 0
  %ok_filho = icmp eq i8 %filho_vivo, 3
  %ok = and i1 %ok_obj, %ok_filho
  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %retorno)
  call void @dartforge_arc_collect()
  %final = call i8 @dartforge_arc_observe_heap_v1(i64 %retorno)
  %morto = icmp eq i8 %final, 0
  br i1 %morto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    if matches!(scenario, Scenario::Cell) {
        ir = ir.replace(
            "@dartforge_arc_object_owned_v1(i64 123, i64 40)",
            &format!(
                "@dartforge_arc_object_owned_v1(i64 {}, i64 1)",
                dartforge_runtime::layout::cid::CELL
            ),
        );
        let start = ir
            .find("  %inteiro = call i64 @ler_inteiro(i64 %obj)\n")
            .ok_or("missing start of the scalar check")?;
        let end = ir[start..]
            .find("  br i1 %ok_inicio,")
            .map(|n| start + n)
            .ok_or("missing end of the scalar check")?;
        ir.replace_range(start..end, "  %ok_inicio = icmp eq i8 %aresta, 1\n");
    }
    if matches!(scenario, Scenario::Late) {
        // The empty probe does not change the late state. The harness marks only
        // after initializing; the certified body copies before removing
        // the edge and returns Owned, as in the other scenarios.
        for declaration in [
            "declare i8 @dartforge_late_field_initialized(i64, i64)",
            "declare void @dartforge_late_field_mark_initialized(i64, i64)",
        ] {
            if !ir.contains(declaration) {
                ir.push_str(&format!("\n{declaration}\n"));
            }
        }
        ir = ir.replace("  call void @inicializar(i64 %obj, i64 %filho)\n",
            "  %late_vazio = call i64 @dartforge_arc_read_field_late_ref_v1(i64 %obj, i64 39)\n  %late_estado = call i8 @dartforge_late_field_initialized(i64 %obj, i64 39)\n  %late_null = icmp eq i64 %late_vazio, 0\n  %late_sem_marca = icmp eq i8 %late_estado, 0\n  %late_ok = and i1 %late_null, %late_sem_marca\n  call void @inicializar(i64 %obj, i64 %filho)\n  call void @dartforge_late_field_mark_initialized(i64 %obj, i64 39)\n");
        ir = ir.replace("  %ok_inicio = and i1 %ok_campos, %ok_aresta\n",
            "  %ok_inicializado = and i1 %ok_campos, %ok_aresta\n  %ok_inicio = and i1 %ok_inicializado, %late_ok\n");
    }
    if matches!(scenario, Scenario::Context) {
        // The address stays in the AOT image and is never invoked in this proof;
        // only the strong edge of the closure and the borrow of the context are used.
        ir.push_str(
            "\ndefine i64 @entrada_contexto(i64 %clo, ptr %args, ptr %desc) { ret i64 0 }\n",
        );
        ir = ir.replace("  %retorno = call i64 @ler_e_substituir(i64 %obj)\n",
            "  %codigo = ptrtoint ptr @entrada_contexto to i64\n  %closure = call i64 @dartforge_arc_closure_owned_v1(i64 %codigo, i64 %obj, i64 0, i64 0)\n  %retorno = call i64 @ler_e_substituir(i64 %closure)\n  call void @dartforge_arc_release(i64 %closure)\n");
        ir = ir.replace("  %ok = and i1 %ok_obj, %ok_filho\n",
            "  %closure_estado = call i8 @dartforge_arc_observe_heap_v1(i64 %closure)\n  %closure_morta = icmp eq i8 %closure_estado, 0\n  %ok_par = and i1 %ok_obj, %ok_filho\n  %ok = and i1 %ok_par, %closure_morta\n");
    }
    if capture {
        ir = ir.replace(
            "@dartforge_arc_object_owned_v1(i64 123, i64 40)",
            &format!(
                "@dartforge_arc_object_owned_v1(i64 {}, i64 40)",
                dartforge_runtime::layout::cid::CONTEXT
            ),
        );
    }
    if neighbor {
        ir = ir.replace("  call void @inicializar(i64 %obj, i64 %filho)\n",
            "  %vizinho = call i64 @dartforge_arc_object_owned_v1(i64 123, i64 40)\n  call void @inicializar(i64 %vizinho, i64 %filho)\n  call void @inicializar(i64 %obj, i64 %vizinho)\n  call void @dartforge_arc_release(i64 %vizinho)\n");
        ir = ir.replace("  %ok = and i1 %ok_obj, %ok_filho\n",
            "  %vizinho_morto = call i8 @dartforge_arc_observe_heap_v1(i64 %vizinho)\n  %ok_vizinho = icmp eq i8 %vizinho_morto, 0\n  %ok_par = and i1 %ok_obj, %ok_filho\n  %ok = and i1 %ok_par, %ok_vizinho\n");
    }
    if matches!(scenario, Scenario::Global) {
        ir = ir.replace("  %retorno = call i64 @ler_e_substituir(i64 %obj)\n",
            "  call void @guardar_global(i64 %obj)\n  call void @dartforge_arc_release(i64 %obj)\n  %retorno = call i64 @ler_e_substituir()\n");
        ir = ir.replace(
            "  call void @dartforge_arc_release(i64 %obj)\n  call void @dartforge_arc_collect()\n",
            "  call void @dartforge_arc_collect()\n",
        );
    }
    if argument {
        ir.push_str("\n@desc.argumento = private constant [2 x i64] [i64 1, i64 0]\n");
        ir = ir.replace("  %retorno = call i64 @ler_e_substituir(i64 %obj)\n",
            "  %argumentos = alloca [1 x i64], align 8\n  store i64 %obj, ptr %argumentos\n  %retorno = call i64 @ler_e_substituir(i64 %obj, ptr %argumentos, ptr @desc.argumento)\n");
    }
    if matches!(
        scenario,
        Scenario::Nullable | Scenario::Mixed | Scenario::Cycle
    ) {
        ir = ir.replace(
            "  %retorno = call i64 @ler_e_substituir(i64 %obj)\n",
            "  %retorno = call i64 @ler_e_substituir(i64 %obj, i1 true)\n",
        );
        ir = ir.replace("passou:\n  call void @dartforge_print_handle(i64 3)",
            "passou:\n  %obj_nulo = call i64 @dartforge_arc_object_owned_v1(i64 123, i64 40)\n  %filho_nulo = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775807)\n  call void @inicializar(i64 %obj_nulo, i64 %filho_nulo)\n  call void @dartforge_arc_release(i64 %filho_nulo)\n  %ret_nulo = call i64 @ler_e_substituir(i64 %obj_nulo, i1 false)\n  %eh_nulo = icmp eq i64 %ret_nulo, 0\n  call void @dartforge_arc_release(i64 %obj_nulo)\n  call void @dartforge_arc_collect()\n  %obj_nulo_estado = call i8 @dartforge_arc_observe_heap_v1(i64 %obj_nulo)\n  %filho_nulo_estado = call i8 @dartforge_arc_observe_heap_v1(i64 %filho_nulo)\n  %obj_nulo_morto = icmp eq i8 %obj_nulo_estado, 0\n  %filho_nulo_morto = icmp eq i8 %filho_nulo_estado, 0\n  %ambos_nulos_mortos = and i1 %obj_nulo_morto, %filho_nulo_morto\n  %ok_nulo = and i1 %ambos_nulos_mortos, %eh_nulo\n  br i1 %ok_nulo, label %passou_nulo, label %falha\npassou_nulo:\n  call void @dartforge_print_handle(i64 3)");
    }
    if matches!(scenario, Scenario::Mixed) {
        // Records only the native address, without creating a root or an owner. The
        // later observation must find the local allocation already destroyed.
        let line = ir
            .lines()
            .find(|l| l.contains("%v108 = call i64 @dartforge_arc_object_owned_v1"))
            .ok_or("local allocation of the mixed proof not found")?
            .to_owned();
        ir = ir.replace(
            &line,
            &format!("{line}\n  store i64 %v108, ptr @misto.local"),
        );
        ir.push_str("\n@misto.local = internal global i64 0\n");
        ir = ir.replace("  %ok_nulo = and i1 %ambos_nulos_mortos, %eh_nulo",
            "  %local = load i64, ptr @misto.local\n  %local_estado = call i8 @dartforge_arc_observe_heap_v1(i64 %local)\n  %local_morto = icmp eq i8 %local_estado, 0\n  %originais_mortos = and i1 %ambos_nulos_mortos, %eh_nulo\n  %ok_nulo = and i1 %originais_mortos, %local_morto");
    }
    match control.as_deref() {
        None => {}
        Some("no-retain") => {
            let borrowed = m.functions[0]
                .blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .find_map(|(v, i, _)| match i {
                    Instruction::ArcCopy {
                        value: Operand::Val(of),
                    } if *v == ValueId(if neighbor { 2 } else { 1 }) => Some(of.0),
                    _ => None,
                })
                .ok_or("return copy not found")?;
            let source_retain = arc_symbols::source_symbol(&ir, "retain");
            let call = format!("  call void @{source_retain}(i64 %v{borrowed})\n");
            if ir.matches(&call).count() != 1 {
                return Err("produced retention was not located uniquely in the IR".into());
            }
            ir = ir.replace(&call, "");
        }
        Some("no-release") => {
            ir = ir.replace("  call void @dartforge_arc_release(i64 %retorno)\n", "");
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
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn audited_context_sustains_fields_and_owned_return() {
        let m = super::module(super::Scenario::Context).unwrap();
        super::LlvmEmitter::new(&m).emit_all();
    }

    #[test]
    fn ref_cell_promotes_owner_before_clearing_slot() {
        let m = super::module(super::Scenario::Cell).unwrap();
        super::LlvmEmitter::new(&m).emit_all();
    }

    #[test]
    fn phi_cycle_preserves_owner_and_field_guards() {
        let m = super::module(super::Scenario::Cycle).unwrap();
        super::LlvmEmitter::new(&m).emit_all();
    }
    use super::*;

    #[test]
    fn mixed_phi_preserves_local_layout_and_checks_borrowed_receiver() {
        let m = module(Scenario::Mixed).unwrap();
        let f = &m.functions[0];
        assert!(f.blocks[2].instructions.iter().any(|(v, i, _)| *v == ValueId(108)
            && matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_object_owned_v1")));
        let b = f.blocks.iter().find(|b| b.id == BlockId(5)).unwrap();
        assert!(b.instructions.iter().any(
            |(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_read_field_scalar_checked_v1")
        ));
        assert!(b.instructions.iter().any(
            |(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_write_field_ref_checked_v1")
        ));
    }

    #[test]
    fn nullable_phi_checks_fields_and_transfers_owner_on_both_exits() {
        let m = module(Scenario::Nullable).unwrap();
        let b = m.functions[0]
            .blocks
            .iter()
            .find(|b| b.id == BlockId(5))
            .unwrap();
        let save = b
            .instructions
            .iter()
            .position(|(v, _, _)| *v == ValueId(2))
            .unwrap();
        let copy = b
            .instructions
            .iter()
            .position(|(v, i, _)| *v == ValueId(1) && matches!(i, Instruction::ArcCopy { .. }))
            .unwrap();
        assert!(copy < save);
        assert!(b.instructions.iter().any(
            |(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_write_field_ref_checked_v1")
        ));
        let null = m.functions[0]
            .blocks
            .iter()
            .find(|b| b.id == BlockId(4))
            .unwrap();
        assert!(matches!(
            null.terminator,
            Terminator::Return(Some(Operand::Val(ValueId(105))))
        ));
        assert!(b.instructions.iter().any(
            |(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_read_field_scalar_checked_v1")
        ));
    }

    #[test]
    fn owned_argument_sustains_receiver_and_certifies_readonly_vectors() {
        let m = module(Scenario::Argument).unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(matches!(&ops[0].1, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_argument_owned_v1"));
        let release_ref = ops
            .iter()
            .position(|(_, i, _)| {
                matches!(
                    i,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(9))
                    }
                )
            })
            .unwrap();
        let store = ops.iter().position(|(v, _, _)| *v == ValueId(2)).unwrap();
        assert!(release_ref > store);
        assert!(m.arc_aot_ports.is_some());
    }

    #[test]
    fn derived_capture_keeps_owners_before_mutating_env() {
        let m = module(Scenario::CaptureNeighbor).unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(
            matches!(&ops[0].1, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_env_ref_v1")
        );
        let remove = ops.iter().position(|(v, _, _)| *v == ValueId(3)).unwrap();
        for id in [ValueId(1), ValueId(2)] {
            let retain = ops
                .iter()
                .position(|(v, i, _)| *v == id && matches!(i, Instruction::ArcCopy { .. }))
                .unwrap();
            assert!(retain < remove);
        }
    }

    #[test]
    fn child_and_neighbor_survive_root_edge_removal() {
        let m = module(Scenario::Neighbor).unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        let remove_root = ops.iter().position(|(v, _, _)| *v == ValueId(3)).unwrap();
        for id in [ValueId(1), ValueId(2)] {
            let retain = ops
                .iter()
                .position(|(v, i, _)| *v == id && matches!(i, Instruction::ArcCopy { .. }))
                .unwrap();
            assert!(retain < remove_root);
        }
        let remove_child = ops.iter().position(|(v, _, _)| *v == ValueId(4)).unwrap();
        let release_neighbor = ops
            .iter()
            .position(|(_, i, _)| {
                matches!(
                    i,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1))
                    }
                )
            })
            .unwrap();
        assert!(release_neighbor > remove_child);
    }

    #[test]
    fn produced_retention_precedes_edge_replacement() {
        let m = module(Scenario::Field).unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(matches!(
            ops[1],
            (ValueId(1), Instruction::ArcCopy { .. }, Type::Ref)
        ));
        let ir = LlvmEmitter::new(&m).emit_all();
        assert!(
            ir.find(&format!(
                "call void @{}(",
                arc_symbols::source_symbol(&ir, "retain")
            ))
            .unwrap()
                < ir.find("call void @dartforge_arc_write_field_ref_checked_v1")
                    .unwrap()
        );
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;

    #[test]
    fn audited_capture_is_retained_before_replacing_edge() {
        let m = module(Scenario::Capture).unwrap();
        let ir = LlvmEmitter::new(&m).emit_all();
        let read = ir.find("call i64 @dartforge_arc_env_ref_v1").unwrap();
        let retain = ir
            .find(&format!(
                "call void @{}(",
                arc_symbols::source_symbol(&ir, "retain")
            ))
            .unwrap();
        let store = ir
            .find("call void @dartforge_arc_write_field_ref_checked_v1")
            .unwrap();
        assert!(read < retain && retain < store);
    }
}
