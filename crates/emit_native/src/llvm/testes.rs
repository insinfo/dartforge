//! Testes do contrato no IR emitido (docs/NATIVO-PLANO.md §6: E1, G1–G3).

use super::LlvmEmitter;
use crate::hir::*;

fn funcao(
    symbol: &str,
    params: Vec<(ValueId, String, Type)>,
    return_ty: Type,
    blocks: Vec<BasicBlock>,
) -> Function {
    Function {
        symbol: symbol.to_string(),
        name: symbol.to_string(),
        params,
        return_ty,
        blocks,
    }
}

fn emitir(f: Function) -> String {
    let mut m = Module::new();
    m.functions.push(f);
    LlvmEmitter::new(&m).emit_all()
}

fn corpo_de<'a>(ir: &'a str, symbol: &str) -> &'a str {
    let ini = ir.find(&format!("@{symbol}(")).expect("função emitida");
    let fim = ir[ini..].find("\n}\n").map_or(ir.len(), |f| ini + f);
    &ir[ini..fim]
}

#[test]
fn literal_wtf8_preserva_surrogate_isolado_no_ir() {
    let f = funcao(
        "literal_wtf8",
        vec![],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(0),
                Instruction::Const(Constant::StringWtf8(vec![0xED, 0xA0, 0xBD])),
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
    );
    let ir = emitir(f);
    assert!(ir.contains("[3 x i8] c\"\\ED\\A0\\BD\""), "{ir}");
    assert!(ir.contains("@dartforge_string_new(ptr @.str.0, i64 3)"), "{ir}");
    assert!(!ir.contains("\\EF\\BF\\BD"), "surrogate foi substituído: {ir}");
}

/// G1/G3: função com `Ref` abre o quadro, enraíza parâmetro e resultado de
/// chamada, e fecha o quadro antes de TODO `ret` — inclusive o da saída
/// por exceção.
#[test]
fn quadro_de_raizes_em_todo_ret() {
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let f = funcao(
        "f",
        vec![(v0, "s".to_string(), Type::Ref)],
        Type::Ref,
        vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    v1,
                    Instruction::CallRuntime {
                        name: "dartforge_string_concat".to_string(),
                        args: vec![(Operand::Val(v0), Type::Ref), (Operand::Val(v0), Type::Ref)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(true)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(v1))),
            },
            // A saída por exceção devolve o valor padrão pelo mesmo `ret`.
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ],
    );
    let ir = emitir(f);
    let corpo = corpo_de(&ir, "f");
    assert!(
        corpo.contains("%gcq = alloca { ptr, i64, [2 x i64] }")
            && corpo.contains("call void @dartforge_gc_empilhar(ptr %gcq)"),
        "{corpo}"
    );
    assert!(
        corpo.contains("store i64 %v0, ptr %gcs0"),
        "{corpo}"
    );
    assert!(
        corpo.contains("store i64 %v1, ptr %gcs1"),
        "{corpo}"
    );
    let rets = corpo.matches("\n  ret ").count();
    let pops = corpo.matches("@dartforge_gc_desempilhar(ptr %gcq)").count();
    assert_eq!(rets, 2, "{corpo}");
    assert_eq!(pops, rets, "todo ret fecha o quadro: {corpo}");
}

/// G: função sem valor `Ref` não abre quadro.
#[test]
fn sem_ref_sem_quadro() {
    let v0 = ValueId(0);
    let f = funcao(
        "g",
        vec![(v0, "n".to_string(), Type::I64)],
        Type::I64,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![],
            terminator: Terminator::Return(Some(Operand::Val(v0))),
        }],
    );
    let ir = emitir(f);
    assert!(!corpo_de(&ir, "g").contains("gc_empilhar"));
}

/// G2: `store` num local `Ref` atualiza o slot do `alloca`.
#[test]
fn local_ref_tem_slot_proprio() {
    let (p, v0, v1) = (ValueId(0), ValueId(1), ValueId(2));
    let f = funcao(
        "h",
        vec![],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (p, Instruction::Alloca(Type::Ref), Type::Ptr),
                (
                    v0,
                    Instruction::Const(Constant::String("x".to_string())),
                    Type::Ref,
                ),
                (
                    v1,
                    Instruction::Store {
                        ptr: Operand::Val(p),
                        val: Operand::Val(v0),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let ir = emitir(f);
    let corpo = corpo_de(&ir, "h");
    assert!(corpo.contains("store i64 %v1, ptr %v0"), "{corpo}");
    assert!(
        corpo.contains("store i64 %v1, ptr %gcs0"),
        "{corpo}"
    );
}

/// E1: gravar um `Ref` num campo leva `is_ref = 1`; um escalar, 0.
#[test]
fn campo_ref_leva_is_ref() {
    let (v0, v1) = (ValueId(0), ValueId(1));
    let f = funcao(
        "k",
        vec![
            (v0, "o".to_string(), Type::Ref),
            (v1, "s".to_string(), Type::Ref),
        ],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::SetField {
                        object: Operand::Val(v0),
                        index: 0,
                        value: Operand::Val(v1),
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::SetField {
                        object: Operand::Val(v0),
                        index: 1,
                        value: Operand::Constant(Constant::Int(7)),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let ir = emitir(f);
    let corpo = corpo_de(&ir, "k");
    assert!(
        corpo.contains("@dartforge_object_set(i64 %v0, i64 0, i64 %v1, i8 1)"),
        "{corpo}"
    );
    assert!(
        corpo.contains("@dartforge_object_set(i64 %v0, i64 1, i64 7, i8 0)"),
        "{corpo}"
    );
}

/// E3: o verificador recusa constante inteira numa posição `Ref` e tag
/// incoerente com a representação.
#[test]
fn verificador_recusa_inteiro_como_ref_e_tag_errada() {
    let v0 = ValueId(0);
    let f = funcao(
        "m",
        vec![(v0, "l".to_string(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "dartforge_print_handle".to_string(),
                        args: vec![(Operand::Constant(Constant::Int(0)), Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "dartforge_list_push".to_string(),
                        args: vec![
                            (Operand::Val(v0), Type::Ref),
                            (Operand::Constant(Constant::Int(5)), Type::I64),
                            (Operand::Constant(Constant::Int(3)), Type::I8),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions.push(f);
    let erros = crate::lower::verificador::verificar(&m);
    assert!(
        erros
            .iter()
            .any(|e| e.contains("constante inteira numa posição Ref")),
        "{erros:?}"
    );
    assert!(
        erros.iter().any(|e| e.contains("tag 3 para um valor I64")),
        "{erros:?}"
    );
}
