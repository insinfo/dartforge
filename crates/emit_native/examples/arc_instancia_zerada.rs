//! Prova AOT da preparação da fábrica zerada usada pelo lowering de instâncias.
//! Confere token Owned, registro de métodos, chamada por seletor e morte final.
//! A construção é uma fixture HIR explícita; não certifica construtores gerais.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn modulo() -> Result<Module, String> {
    let mut m = Module::new();
    m.memoria_arc = true;
    m.modo_sdk = true;
    m.biblioteca_sdk = true;
    m.entry_symbol = Some("prova_instancia_zerada".into());
    // Emite o thunk pelo protocolo real de tabelas. O registro desta fixture
    // não é chamado na partida: a fábrica deve efetuar o registro preguiçoso.
    m.registro = Some("df.reg.C".into());
    m.classes.push(ClassDef {
        id: 32001,
        name: "C".into(),
        field_count: 1,
        vtable: vec![],
        to_string_symbol: None,
    });
    m.layouts_campos_arc.insert(32001, vec![Type::I64]);
    m.funcoes_de_tabela.insert(32001, "df.mt.C".into());
    m.tabelas_de_metodos.push((
        32001,
        "df.mt.C".into(),
        vec![("c:valor".into(), "metodo".into())],
    ));
    m.functions.push(Function {
        symbol: "criar".into(),
        name: "criar".into(),
        depuracao: None,
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(0),
                Instruction::CallRuntime {
                    name: "dartforge_object_new".into(),
                    args: vec![
                        (Operand::Constant(Constant::Int(32001)), Type::I64),
                        (Operand::Constant(Constant::Int(1)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
    });
    m.functions.push(Function {
        symbol: "metodo".into(),
        name: "metodo".into(),
        depuracao: None,
        params: vec![
            (ValueId(0), "this".into(), Type::Ref),
            (ValueId(1), "args".into(), Type::Ptr),
            (ValueId(2), "desc".into(), Type::Ptr),
        ],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(3),
                Instruction::Box {
                    op: Operand::Constant(Constant::Int(1)),
                    from: Type::I64,
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
        }],
    });
    let mut retorno = PlanoFuncaoDart::default();
    retorno.tokens.retorno = RetornoTokens::Owned;
    let mut metodo = retorno.clone();
    // Endereços nativos do ABI uniforme, sem handles ou slots proprietários.
    metodo.classes.extend([
        (ValueId(1), Ownership::Trivial),
        (ValueId(2), Ownership::Trivial),
    ]);
    let mut planos = HashMap::from([("criar".into(), retorno), ("metodo".into(), metodo)]);
    preparar_arc_modulo_dart(&mut m, &mut planos)?;
    let antes = format!("{m:?}{planos:?}");
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}{planos:?}") != antes
    {
        return Err("preparação não idempotente".into());
    }
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável")?);
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
    let mut m = modulo()?;
    // Tracing usa tokens explícitos somente neste harness de comparação.
    m.memoria_arc = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    let fabrica = "call i64 @dartforge_arc_objeto_owned_t_v1(i64 32001, i64 1, ptr @df.mt.C)";
    if ir.matches(fabrica).count() != 1 {
        return Err("fábrica com tabela ausente/ambígua".into());
    }
    ir.push_str(r#"
@cache.prova = private global [2 x i64] zeroinitializer
@nome.prova = private constant [7 x i8] c"c:valor"
declare void @dartforge_print_handle(i64)
define void @prova_instancia_zerada() {
  %obj = call i64 @criar()
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %owned = icmp eq i8 %estado, 3
  br i1 %owned, label %campo, label %falha
campo:
  %zero = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 0)
  %zerado = icmp eq i64 %zero, 0
  br i1 %zerado, label %seletor, label %falha
seletor:
  %ptr = call ptr @dartforge_seletor(ptr @cache.prova, i64 %obj, i64 HASH_PROVA, ptr @nome.prova, i64 7)
  %tabela = icmp eq ptr %ptr, @metodo
  br i1 %tabela, label %chamar, label %falha
chamar:
  %valor = call i64 %ptr(i64 %obj, ptr null, ptr null)
  %um = icmp eq i64 %valor, 3
  br i1 %um, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %obj)
  call void @dartforge_arc_collect()
  %fim = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %morto = icmp eq i8 %fim, 0
  br i1 %morto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#);
    let hash = b"c:valor".iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    }) as i64;
    ir = ir.replace("HASH_PROVA", &hash.to_string());
    // Emite como biblioteca para que o registro de partida não antecipe a
    // fábrica. A entrada do harness liga somente a política solicitada.
    ir.push_str("\ndefine void @dartforge_entry() {\n");
    if arc {
        ir.push_str("  call void @dartforge_memoria_arc_v1()\n");
    }
    ir.push_str("  call void @prova_instancia_zerada()\n  ret void\n}\n");
    ir.push_str("define i64 @dartforge_dispatch_toString(i64 %obj) {\n  ret i64 0\n}\n");
    if let Some(controle) = args.next() {
        let (antes, depois) = match controle.as_str() {
            "sem-owner" => (
                fabrica,
                "call i64 @dartforge_object_new_t(i64 32001, i64 1, ptr @df.mt.C)",
            ),
            "sem-tabela" => (
                fabrica,
                "call i64 @dartforge_arc_objeto_owned_v1(i64 32001, i64 1)",
            ),
            "sem-drop" => ("  call void @dartforge_arc_release(i64 %obj)\n", ""),
            _ => return Err(format!("controle desconhecido: {controle}")),
        };
        if ir.matches(antes).count() != 1 {
            return Err("controle sem alvo único".into());
        }
        ir = ir.replace(antes, depois);
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
    )?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn instancia_zerada_gera_ir_verificado_com_tabela_owned() {
        let ir = LlvmEmitter::new(&modulo().unwrap()).emit_all();
        assert!(ir.contains("call i64 @dartforge_arc_objeto_owned_t_v1"));
        assert!(ir.contains("define ptr @df.mt.C()"));
    }
}
