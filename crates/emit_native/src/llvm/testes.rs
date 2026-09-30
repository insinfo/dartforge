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
        depuracao: None,
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

/// §2.11: no AOT (`objetos_estaticos`), o literal é um objeto estático do
/// módulo — `_TwoByteString` (cid 7) com o surrogate solto como unidade, o
/// cabeçalho `PERMANENTE | BRUTO | w | cid` e o hash da VM —, sem chamada nem
/// cache no ponto de uso.
#[test]
fn literal_estatico_no_aot() {
    use dartforge_runtime::layout;
    let f = funcao(
        "literal_estatico",
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
    let mut m = Module::new();
    m.functions.push(f);
    let ir = LlvmEmitter::new(&m).com_objetos_estaticos(true).emit_all();
    let corpo = corpo_de(&ir, "literal_estatico");
    assert!(!corpo.contains("@dartforge_string_new("), "{corpo}");
    // O nome do global é `@"df.s.<blake3>"` (com aspas: tem pontos).
    assert!(ir.contains("df.s."), "{ir}");
    assert!(ir.contains("linkonce_odr"), "{ir}");
    let cabecalho = layout::palavra_do_cabecalho(
        layout::estado::PERMANENTE,
        layout::flags::BRUTO,
        layout::palavras_de_texto(1, true),
        layout::cid::TWO_BYTE_STRING,
    );
    assert!(ir.contains(&format!("i64 {}", cabecalho as i64)), "cabeçalho {cabecalho:#x}: {ir}");
    let hash = layout::hash_de_texto([0xD83Du16]);
    assert!(ir.contains(&format!("i64 {hash}")), "hash {hash}: {ir}");
}

/// G1/G3: função com `Ref` vivo num ponto de coleta abre o quadro, enraíza
/// o parâmetro (argumento da chamada que aloca) e fecha o quadro antes de
/// TODO `ret` — inclusive o da saída por exceção. O resultado da chamada só
/// é usado no `ret`, sem coleta no meio: não ganha raiz (`raizes.rs`).
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
        corpo.contains("%gcq = alloca { ptr, i64, [1 x i64] }")
            && corpo.contains("%ctx = call ptr @dartforge_contexto()")
            && corpo.contains("store ptr %gcq, ptr %ctxtopo"),
        "{corpo}"
    );
    assert!(corpo.contains("store i64 %v0, ptr %gcs0"), "{corpo}");
    assert!(!corpo.contains("store i64 %v1, ptr"), "{corpo}");
    let rets = corpo.matches("\n  ret ").count();
    let pops = corpo.matches("store ptr %gcvolta").count();
    assert_eq!(rets, 2, "{corpo}");
    assert_eq!(pops, rets, "todo ret fecha o quadro: {corpo}");
}

/// Uma chamada que aloca.
fn concat(a: ValueId, b: ValueId) -> Instruction {
    Instruction::CallRuntime {
        name: "dartforge_string_concat".to_string(),
        args: vec![(Operand::Val(a), Type::Ref), (Operand::Val(b), Type::Ref)],
        ret_ty: Type::Ref,
    }
}

/// Vivacidade: `v1` atravessa a segunda chamada (é usado depois dela) e
/// ganha raiz; `v0` morre na primeira e `v1` nasce dela, então os dois
/// dividem o slot; `v2` só vai ao `ret`.
#[test]
fn raizes_por_vivacidade_e_slot_compartilhado() {
    let (v0, v1, v2, v3) = (ValueId(0), ValueId(1), ValueId(2), ValueId(3));
    let f = funcao(
        "g",
        vec![(v0, "s".to_string(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (v1, concat(v0, v0), Type::Ref),
                (v2, concat(v1, v1), Type::Ref),
                (v3, concat(v2, v1), Type::Ref),
            ],
            terminator: Terminator::Return(Some(Operand::Val(v3))),
        }],
    );
    let ir = emitir(f);
    let corpo = corpo_de(&ir, "g");
    // v0 e v1 no slot 0 (não interferem: v0 morre na definição de v1); v2
    // é argumento da terceira chamada junto com v1, então interfere e vai
    // ao slot 1; v3 não passa por coleta.
    assert!(corpo.contains("[2 x i64]"), "{corpo}");
    assert!(corpo.contains("store i64 %v0, ptr %gcs0"), "{corpo}");
    assert!(corpo.contains("store i64 %v1, ptr %gcs0"), "{corpo}");
    assert!(corpo.contains("store i64 %v2, ptr %gcs1"), "{corpo}");
    assert!(!corpo.contains("store i64 %v3, ptr"), "{corpo}");
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

/// E1: gravar um `Ref` num campo leva `is_ref = 1`; um escalar, 0 — em
/// linha, no par `(bits, is_ref)` do vetor de campos.
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
    // O endereço dos campos vem do próprio objeto (`h + 14`), sem chamada.
    assert!(!corpo.contains("@dartforge_object_campos("), "{corpo}");
    // O cabeçalho em `h - 2`; os campos, palavras de 8 bytes depois dele; o
    // `is_ref` é o bit do campo no mapa do cabeçalho.
    assert!(corpo.contains("%fxa2 = add i64 %v0, -2"), "{corpo}");
    assert!(corpo.contains("%fp2 = getelementptr inbounds i8, ptr %fcb2, i64 16"), "{corpo}");
    assert!(corpo.contains("ptr %fp2, i64 0\n  store i64 %v1, ptr %fg2"), "{corpo}");
    assert!(corpo.contains("%fms2 = or i32 %fm2, 1"), "{corpo}");
    assert!(corpo.contains("store i64 7, ptr %fg3"), "{corpo}");
    assert!(corpo.contains("%fmc3 = and i32 %fm3, -3\n  store i32 %fmc3"), "{corpo}");
    assert!(!corpo.contains("@dartforge_object_set("), "{corpo}");
}

/// E3: o verificador recusa constante inteira numa posição `Ref` e tag
/// incoerente com a representação (a ABI plana `(bits, tag)` do
/// lançamento).
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
                        name: "dartforge_exception_throw".to_string(),
                        args: vec![
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

/// Risco 12 (docs/NATIVO-ESPACO-UNIFICADO.md §6): a captura lida na
/// representação com que foi gravada; o record posicional só com `Ref`; e
/// as instruções que saíram com o espaço unificado (`AllocMap`, `AllocSet`).
#[test]
fn verificador_confere_capturas_records_e_instrucoes_que_sairam() {
    let (x, env, clo) = (ValueId(0), ValueId(1), ValueId(2));
    // `f(x)` cria a closure `g` com o ambiente `[x: I64, célula de I64]` e um
    // record com um `I64`.
    let cel = ValueId(3);
    let criadora = funcao(
        "f",
        vec![(x, "x".to_string(), Type::I64)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (cel, Instruction::AllocCell { value: Operand::Val(x) }, Type::Ref),
                (env, Instruction::AllocEnv { values: vec![Operand::Val(x), Operand::Val(cel)] }, Type::Ref),
                (clo, Instruction::AllocClosure { code_symbol: "g".to_string(), env: Operand::Val(env) }, Type::Ref),
                (ValueId(4), Instruction::AllocRecord { elements: vec![(Operand::Val(x), 1)] }, Type::Ref),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    // `g` lê a captura 0 como `Ref` (gravada `I64`) e a célula como `F64`.
    let (e, c0, c1, c2) = (ValueId(0), ValueId(1), ValueId(2), ValueId(3));
    let corpo = funcao(
        "g",
        vec![(e, "env".to_string(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (c0, Instruction::EnvGet { env: Operand::Val(e), index: 0 }, Type::Ref),
                (c1, Instruction::EnvGet { env: Operand::Val(e), index: 1 }, Type::Ref),
                (c2, Instruction::CellGet { cell: Operand::Val(c1) }, Type::F64),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions.push(criadora);
    m.functions.push(corpo);
    let erros = crate::lower::verificador::verificar(&m);
    let tem = |s: &str| erros.iter().any(|e| e.contains(s));
    assert!(tem("captura 0 do ambiente: lida como Ref, gravada como I64"), "{erros:?}");
    assert!(tem("célula: lida como F64, gravada como I64"), "{erros:?}");
    assert!(tem("record posicional: campo I64"), "{erros:?}");
    // A captura 1 (a célula, `Ref`) lida como `Ref`: sem erro.
    assert!(!tem("captura 1"), "{erros:?}");
}
