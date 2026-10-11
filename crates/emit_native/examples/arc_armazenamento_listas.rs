//! Prova AOT de storage Ref, escrita compacta e owner antes da troca da aresta.
//! O corpo HIR é preparado em ARC; tracing executa o mesmo protocolo explícito.
//! Controles negativos retiram retenção ou liberação somente após verificação.
//! `--crescer` força crescimento e descompactação com coleta durante a chamada.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, fmt::Write, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") | None => false,
        Some(outro) => return Err(format!("modo desconhecido: {outro}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(outro) => return Err(format!("nível desconhecido: {outro}")),
    };
    let primeiro = args.next();
    let crescer = primeiro.as_deref() == Some("--crescer");
    let controle = if crescer { args.next() } else { primeiro };
    if args.next().is_some() {
        return Err("argumentos excedentes".into());
    }
    let val = |n| Operand::Val(ValueId(n));
    let mut m = Module::new();
    m.memoria_arc = true;
    m.entry_symbol = Some("prova_listas".into());
    m.retornos_ref_dart.insert("trocar_armazenamento".into());
    let chamada = |nome: &str, args: Vec<(Operand, Type)>, ret_ty| Instruction::CallRuntime {
        name: nome.into(),
        args,
        ret_ty,
    };
    m.functions.push(Function {
        symbol: "trocar_armazenamento".into(),
        name: "trocar_armazenamento".into(),
        depuracao: None,
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
                    chamada(
                        "dartforge_arc_lista_armazenamento_ref_v1",
                        vec![(val(0), Type::Ref)],
                        Type::Ref,
                    ),
                    Type::Ref,
                ),
                (
                    ValueId(5),
                    chamada(
                        "dartforge_arc_lista_gravar_bits_v1",
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
                    chamada(
                        "dartforge_arc_lista_gravar_dados_v1",
                        vec![(val(0), Type::Ref), (val(1), Type::Ref)],
                        Type::I64,
                    ),
                    Type::I64,
                ),
                (
                    ValueId(7),
                    chamada(
                        "dartforge_arc_lista_gravar_len_v1",
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
    if crescer {
        let insts = &mut m.functions[0].blocks[0].instructions;
        insts[2] = (
            ValueId(6),
            chamada(
                "dartforge_lista_acrescentar",
                vec![(val(0), Type::Ref), (val(1), Type::Ref)],
                Type::Ref,
            ),
            Type::Ref,
        );
        insts.remove(3);
    }
    let mut plano = PlanoFuncaoDart::default();
    plano.tokens.retorno = RetornoTokens::Owned;
    for v in [ValueId(2), ValueId(3)] {
        plano.classes.insert(v, Ownership::Trivial);
    }
    let mut planos = HashMap::from([("trocar_armazenamento".into(), plano)]);
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (1, 0) {
        return Err("esperada uma cópia proprietária do armazenamento antes da mutação".into());
    }
    let emprestado = m.functions[0].blocks[0]
        .instructions
        .iter()
        .find_map(|(v, i, _)| match i {
            Instruction::ArcCopy {
                value: Operand::Val(de),
            } if *v == ValueId(4) => Some(de.0),
            _ => None,
        })
        .ok_or("cópia do armazenamento ausente")?;
    m.memoria_arc = arc;
    if !arc {
        m.portas_arc_aot = None;
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
    for (n, codigo, forma, bits) in [
        (0, 1, 0x22, i64::MIN),
        (1, 2, 0x42, 0xfff8_1234_5678_9abc_u64 as i64),
        (2, 3, 0x62, 1),
    ] {
        writeln!(ir, "  %pai{n} = call i64 @dartforge_lista_nova(ptr %palavra, i64 1, i64 10, i64 {codigo})\n  call void @dartforge_arc_retain(i64 %pai{n})\n  %novo{n} = call i64 @dartforge_lista_nova(ptr null, i64 0, i64 8, i64 {codigo})\n  call void @dartforge_arc_retain(i64 %novo{n})\n  %retorno{n} = call i64 @trocar_armazenamento(i64 %pai{n}, i64 %novo{n}, i64 {bits}, i64 {forma})").unwrap();
        writeln!(ir, "  %len{n} = call i64 @dartforge_arc_lista_len_v1(i64 %pai{n})\n  %cap{n} = call i64 @dartforge_arc_lista_capacidade_v1(i64 %pai{n})\n  %lenzero{n} = icmp eq i64 %len{n}, 0\n  %capzero{n} = icmp eq i64 %cap{n}, 0\n  %encolheu{n} = and i1 %lenzero{n}, %capzero{n}\n  br i1 %encolheu{n}, label %coletar{n}, label %falha\ncoletar{n}:").unwrap();
        writeln!(ir, "  call void @dartforge_arc_release(i64 %pai{n})\n  call void @dartforge_arc_release(i64 %novo{n})\n  call void @dartforge_arc_collect()\n  %estado{n} = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno{n})\n  %vivo{n} = icmp eq i8 %estado{n}, 3\n  br i1 %vivo{n}, label %ler{n}, label %falha\nler{n}:").unwrap();
        // A observação crua só acontece depois de provar que o owner manteve o bloco.
        writeln!(ir, "  %endereco{n} = add i64 %retorno{n}, 22\n  %ptr{n} = inttoptr i64 %endereco{n} to ptr\n  %bits{n} = load i64, ptr %ptr{n}, align 8\n  %iguais{n} = icmp eq i64 %bits{n}, {bits}\n  br i1 %iguais{n}, label %liberar{n}, label %falha\nliberar{n}:\n  call void @dartforge_arc_release(i64 %retorno{n})\n  call void @dartforge_arc_collect()\n  %final{n} = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno{n})\n  %morto{n} = icmp eq i8 %final{n}, 0\n  br i1 %morto{n}, label %passou{n}, label %falha\npassou{n}:").unwrap();
    }
    ir.push_str("  call void @dartforge_print_handle(i64 3)\n  ret void\nfalha:\n  call void @llvm.trap()\n  unreachable\n}\n");
    if crescer {
        for n in 0..3 {
            ir = ir.replace(
                &format!("icmp eq i64 %len{n}, 0"),
                &format!("icmp eq i64 %len{n}, 2"),
            );
            ir = ir.replace(
                &format!("icmp eq i64 %cap{n}, 0"),
                &format!("icmp eq i64 %cap{n}, 3"),
            );
            let liberar = format!(
                "  call void @dartforge_arc_release(i64 %pai{n})\n  call void @dartforge_arc_release(i64 %novo{n})\n"
            );
            if ir.matches(&liberar).count() != 1 {
                return Err("liberação do par não localizada unicamente".into());
            }
            // O valor anexado deve viver só pela aresta copiada, antes de matar a lista.
            let conferir = format!(
                "  call void @dartforge_arc_release(i64 %novo{n})\n  call void @dartforge_arc_collect()\n  %forma{n} = call i64 @dartforge_arc_lista_forma_v1(i64 %pai{n})\n  %geral{n} = icmp eq i64 %forma{n}, 4\n  %filhoestado{n} = call i8 @dartforge_arc_observar_heap_v1(i64 %novo{n})\n  %filhovivo{n} = icmp eq i8 %filhoestado{n}, 1\n  %aresta{n} = and i1 %geral{n}, %filhovivo{n}\n  br i1 %aresta{n}, label %conferir{n}, label %falha\nconferir{n}:\n  %ultimo{n} = call i64 @dartforge_arc_lista_ref_owned_v1(i64 %pai{n}, i64 1)\n  %elemento{n} = icmp eq i64 %ultimo{n}, %novo{n}\n  call void @dartforge_arc_release(i64 %ultimo{n})\n  br i1 %elemento{n}, label %soltarpai{n}, label %falha\nsoltarpai{n}:\n  call void @dartforge_arc_release(i64 %pai{n})\n"
            );
            ir = ir.replace(&liberar, &conferir);
        }
    }
    match controle.as_deref() {
        None => {}
        Some("sem-retencao") => {
            let chamada = format!("  call void @dartforge_arc_retain(i64 %v{emprestado})\n");
            if ir.matches(&chamada).count() != 1 {
                return Err("retenção não localizada unicamente".into());
            }
            ir = ir.replace(&chamada, "");
        }
        Some("sem-liberar") => {
            for n in 0..3 {
                ir = ir.replace(
                    &format!("  call void @dartforge_arc_release(i64 %retorno{n})\n"),
                    "",
                );
            }
        }
        Some(outro) => return Err(format!("controle desconhecido: {outro}")),
    }
    if let Some(dir) = saida.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &saida,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )
    .map(|_| ())
}
