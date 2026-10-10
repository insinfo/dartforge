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
fn global_arc_carrega_bits_reais_e_publica_antes_de_transferir_owner() {
    let slot = SlotForte::Global { simbolo: "dfg.recurso".into() };
    let f = funcao("arc_global", vec![(ValueId(0), "x".into(), Type::Ref)], Type::Void,
        vec![BasicBlock { id: BlockId(0), instructions: vec![
            (ValueId(1), Instruction::ArcStoreStrong { slot: slot.clone(), value: Operand::Val(ValueId(0)), modo: ModoStoreForte::Copy }, Type::Void),
            (ValueId(2), Instruction::ArcLoadStrong { slot: slot.clone() }, Type::Ref),
            (ValueId(3), Instruction::ArcStoreStrong { slot, value: Operand::Val(ValueId(2)), modo: ModoStoreForte::Move }, Type::Void),
        ], terminator: Terminator::Return(None) }]);
    let mut m = Module::new();
    m.globais.push((0, Type::Ref, "dfg.recurso".into()));
    m.functions.push(f);
    assert!(crate::lower::verificador::verificar(&m).is_empty());
    crate::otimizar::otimizar(&mut m);
    let ir = LlvmEmitter::new(&m).emit_all();
    let corpo = corpo_de(&ir, "arc_global");
    assert!(corpo.contains("%v2 = load i64, ptr %ga2"));
    assert!(corpo.contains("@dartforge_arc_retain(i64 %v2)"));
    assert!(corpo.find("@dartforge_arc_retain(i64 %v0)").unwrap() < corpo.find("store i64 %v0, ptr %ga1").unwrap());
    assert!(corpo.find("store i64 %v2, ptr %ga3").unwrap() < corpo.find("@dartforge_arc_global_receber_v1").unwrap());
    assert!(!corpo.contains("@dartforge_arc_release(i64 %v2)"));
    m.globais[0].1 = Type::I64;
    assert!(!crate::lower::verificador::verificar(&m).is_empty());
    m.globais.clear();
    assert!(!crate::lower::verificador::verificar(&m).is_empty());
    m.globais.extend([(0, Type::Ref, "dfg.recurso".into()), (1, Type::Ref, "dfg.recurso".into())]);
    assert!(!crate::lower::verificador::verificar(&m).is_empty());
}

#[test]
fn slots_arc_emitidos_preservam_descritor_e_modos_apos_otimizar() {
    let slot = SlotForte::Quadro {
        quadro: Operand::Val(ValueId(0)),
        indice: 7,
    };
    let f = funcao(
        "arc_slots",
        vec![
            (ValueId(0), "q".into(), Type::I64),
            (ValueId(1), "x".into(), Type::Ref),
        ],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::ArcStoreStrong {
                        slot: slot.clone(),
                        value: Operand::Val(ValueId(1)),
                        modo: ModoStoreForte::Copy,
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::ArcLoadStrong { slot: slot.clone() },
                    Type::Ref,
                ),
                (
                    ValueId(4),
                    Instruction::ArcStoreStrong {
                        slot,
                        value: Operand::Val(ValueId(3)),
                        modo: ModoStoreForte::Move,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions.push(f);
    assert!(crate::lower::verificador::verificar(&m).is_empty());
    crate::otimizar::otimizar(&mut m);
    let ir = LlvmEmitter::new(&m).emit_all();
    let corpo = corpo_de(&ir, "arc_slots");
    assert!(corpo.contains("@dartforge_arc_quadro_copiar_v1(i64 %v0, i64 7, i64 %v1)"));
    assert!(corpo.contains("%v3 = call i64 @dartforge_arc_quadro_carregar_v1(i64 %v0, i64 7)"));
    assert!(corpo.contains("@dartforge_arc_quadro_receber_v1(i64 %v0, i64 7, i64 %v3)"));
    assert!(!corpo.contains("@dartforge_arc_retain"));
    assert!(!corpo.contains("@dartforge_arc_release"));
    m.functions[0].params[0].2 = Type::Ptr;
    assert!(!crate::lower::verificador::verificar(&m).is_empty());
}

#[test]
fn operacoes_arc_preservam_contagem_e_movimento_na_otimizacao() {
    let f = funcao(
        "arc_linear",
        vec![(ValueId(0), "x".into(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::ArcMove {
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(2)),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions.push(f);
    assert!(crate::lower::verificador::verificar(&m).is_empty());
    crate::otimizar::otimizar(&mut m);
    assert!(crate::lower::verificador::verificar(&m).is_empty());
    let ir = LlvmEmitter::new(&m).emit_all();
    let corpo = corpo_de(&ir, "arc_linear");
    assert_eq!(corpo.matches("call void @dartforge_arc_retain").count(), 1);
    assert_eq!(corpo.matches("call void @dartforge_arc_release").count(), 1);
    assert!(corpo.contains("%v2 = add i64 %v1, 0"));
    assert!(corpo.contains("@dartforge_arc_release(i64 %v2)"));
    assert!(!corpo.contains("@dartforge_box"));
}

#[test]
fn inlining_remapeia_operando_arc_e_preserva_token_de_retorno() {
    let doar = funcao(
        "arc_doar",
        vec![(ValueId(0), "x".into(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(1),
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(0)),
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    );
    let chamar = funcao(
        "arc_chamar",
        vec![(ValueId(10), "x".into(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(11),
                    Instruction::CallStatic {
                        symbol: "arc_doar".into(),
                        args: vec![Operand::Val(ValueId(10))],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(12),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(11)),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions = vec![doar, chamar];
    crate::otimizar::otimizar(&mut m);
    assert!(crate::lower::verificador::verificar(&m).is_empty());
    let ir = LlvmEmitter::new(&m).emit_all();
    let corpo = corpo_de(&ir, "arc_chamar");
    assert!(!corpo.contains("call i64 @arc_doar"));
    assert!(corpo.contains("@dartforge_arc_retain(i64 %v10)"));
    assert_eq!(corpo.matches("call void @dartforge_arc_retain").count(), 1);
    assert_eq!(corpo.matches("call void @dartforge_arc_release").count(), 1);
}

#[test]
fn operacoes_arc_recusam_escalar_literal_nao_avaliado_e_resultado_errado() {
    for operacao in 0..3 {
        let esperado = if operacao == 2 { Type::Void } else { Type::Ref };
        let errado = if esperado == Type::Void {
            Type::Ref
        } else {
            Type::Void
        };
        for (value, parametro, resultado) in [
            (Operand::Val(ValueId(0)), Type::I64, esperado),
            (
                Operand::Constant(Constant::String("literal".into())),
                Type::Ref,
                esperado,
            ),
            (Operand::Val(ValueId(0)), Type::Ref, errado),
        ] {
            let inst = match operacao {
                0 => Instruction::ArcCopy { value },
                1 => Instruction::ArcMove { value },
                _ => Instruction::ArcDrop { value },
            };
            assert_eq!(LlvmEmitter::tipo_do_resultado(&inst, resultado), esperado);
            let f = funcao(
                "arc_invalida",
                vec![(ValueId(0), "x".into(), parametro)],
                Type::Void,
                vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), inst, resultado)],
                    terminator: Terminator::Return(None),
                }],
            );
            let mut m = Module::new();
            m.functions.push(f);
            let erros = crate::lower::verificador::verificar(&m);
            assert!(
                erros.iter().any(|e| e.contains("operação ARC")),
                "{erros:?}"
            );
        }
    }
}

#[test]
fn entrada_arc_recusa_abi_incompativel_antes_dos_registros() {
    for sdk in [false, true] {
        for arc in [false, true] {
            let mut m = Module::new();
            m.modo_sdk = sdk;
            m.memoria_arc = arc;
            let ir = LlvmEmitter::new(&m).emit_all();
            let corpo = corpo_de(&ir, if sdk { "df.preparar_isolado" } else { "dartforge_entry" });
            if !arc {
                assert!(!corpo.contains("@dartforge_arc_verificar_abi"));
                continue;
            }
            let ativar = corpo.find("@dartforge_memoria_arc_v1()").unwrap();
            let conferir = corpo.find("@dartforge_arc_verificar_abi(i64 1)").unwrap();
            let compativel = corpo.find("df.arc.compativel:\n").unwrap();
            assert!(ativar < conferir && conferir < compativel);
            assert!(corpo.contains("icmp eq i8 %df.arc.abi, 1"));
            assert!(corpo.contains("label %df.arc.compativel, label %df.arc.incompativel"));
            assert!(corpo.contains("df.arc.incompativel:\n  call void @llvm.trap()\n  unreachable"));
            assert!(ir.contains("declare void @llvm.trap()"));
        }
    }
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
    // A saída da conferência da pilha (C22) volta antes de o quadro ser
    // encadeado: nada a fechar ali.
    let (prologo, resto) = corpo.split_once("pilha.ok:").expect("conferência da pilha no prólogo");
    assert!(prologo.contains("pilha.estouro:") && prologo.contains("call void @dartforge_estouro_de_pilha()"), "{corpo}");
    assert!(!prologo.contains("store ptr %gcq, ptr %ctxtopo"), "o quadro não pode estar encadeado no estouro: {corpo}");
    assert_eq!(prologo.matches("\n  ret ").count(), 1, "{corpo}");
    let rets = resto.matches("\n  ret ").count();
    let pops = resto.matches("store ptr %gcvolta").count();
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

// Testa o emissor do corpo também no host Windows, sem fingir suporte AOT Unix.
#[test]
fn cleanup_retoma_par_original_e_recusa_metadados_incoerentes() {
    let f = funcao("retoma", vec![(ValueId(0), "x".into(), Type::Ref)], Type::Void, vec![
        BasicBlock { id: BlockId(0), instructions: vec![
            (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref),
            (ValueId(2), Instruction::CallStatic { symbol: "falha".into(), args: vec![], ret_ty: Type::Void }, Type::Void),
        ], terminator: Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(false)), then_block: BlockId(2), else_block: BlockId(1) } },
        BasicBlock { id: BlockId(1), instructions: vec![(ValueId(4), Instruction::ArcDrop { value: Operand::Val(ValueId(1)) }, Type::Void)], terminator: Terminator::Return(None) },
        BasicBlock { id: BlockId(2), instructions: vec![
            (ValueId(3), Instruction::ArcDrop { value: Operand::Val(ValueId(1)) }, Type::Void),
        ], terminator: Terminator::Return(None) },
    ]);
    let t = TabelasDaFuncao {
        invocacoes: [(ValueId(2), BlockId(2))].into(),
        pousos: [BlockId(2)].into(),
        saidas: [(BlockId(2), SaidaPorExcecao::Retoma)].into(),
        ..Default::default()
    };
    crate::otimizar::arc::conferir_retomas(&f, &t).unwrap();
    let m = Module::new();
    let mut e = LlvmEmitter::new(&m);
    e.tab = Some(&t);
    e.emit_function(&f);
    let ir = e.out;
    assert!(ir.contains("personality ptr @dartforge_personalidade_cleanup_itanium"));
    #[cfg(feature = "llvm-embutido")]
    conferir_objetos_cleanup(&ir);
    let pouso = &ir[ir.find("b2:").unwrap()..];
    assert!(pouso.contains("%lpad2 = landingpad { ptr, i32 } cleanup"));
    assert!(pouso.contains("resume { ptr, i32 } %lpad2"));
    assert!(!pouso.contains("@df.lancar"));
    assert!(pouso.find("@dartforge_arc_release").unwrap() < pouso.find("resume").unwrap());
    let mut ruim = t.clone();
    ruim.pousos.clear();
    assert!(crate::otimizar::arc::conferir_retomas(&f, &ruim).unwrap_err().contains("pouso"));
    ruim = t.clone();
    ruim.invocacoes.insert(ValueId(1000), BlockId(2));
    assert!(crate::otimizar::arc::conferir_retomas(&f, &ruim).unwrap_err().contains("exatamente um invoke"));
    let mut corpo = f.clone();
    corpo.blocks[0].terminator = Terminator::Branch(BlockId(2));
    assert!(crate::otimizar::arc::conferir_retomas(&corpo, &t).unwrap_err().contains("sem invoke"));
    corpo = f.clone();
    corpo.blocks[2].instructions.push((ValueId(5), Instruction::CallRuntime { name: "dartforge_exception_clear".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
    assert!(crate::otimizar::arc::conferir_retomas(&corpo, &t).unwrap_err().contains("só admite drops"));
    let mut e = LlvmEmitter::new(&m);
    e.tab = Some(&t);
    e.mapas = true;
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| e.emit_function(&f))).is_err());
}

#[cfg(windows)]
#[test]
#[should_panic(expected = "Windows precisa de funclets SEH")]
fn retoma_nao_promete_suporte_seh_no_emissor_publico() {
    let mut m = Module::new();
    m.tabelas.push(TabelasDaFuncao { saidas: [(BlockId(1), SaidaPorExcecao::Retoma)].into(), ..Default::default() });
    LlvmEmitter::new(&m).emit_all();
}

#[cfg(feature = "llvm-embutido")]
fn conferir_objetos_cleanup(ir: &str) {
    // O corpo emitido é verificado e gera código Itanium, sem usar o
    // cabeçalho Windows nem prometer cross-compilation de módulos Dart.
    let triple = if cfg!(target_arch = "aarch64") { "aarch64-unknown-linux-gnu" } else { "x86_64-unknown-linux-gnu" };
    let completo = format!(r#"target triple = "{triple}"
        declare i32 @dartforge_personalidade_cleanup_itanium(...)
        declare i32 @llvm.eh.typeid.for(ptr)
        declare ptr @dartforge_contexto()
        declare void @dartforge_estouro_de_pilha()
        declare i1 @llvm.expect.i1(i1, i1)
        declare void @df.lancar()
        declare void @dartforge_arc_retain(i64)
        declare void @dartforge_arc_release(i64)
        declare i64 @dartforge_efeitos_nivel() nounwind
        declare void @dartforge_efeitos_restaurar(i64) nounwind
        declare void @falha()
        {ir}"#);
    for otimizar in [false, true] {
        let objeto = dartforge_llvm::gerar("cleanup-retoma", &completo, &dartforge_llvm::Opcoes {
            otimizar, formato: dartforge_llvm::Formato::Objeto, cpu: None,
        }).unwrap_or_else(|erro| panic!("{erro}\n{completo}"));
        assert!(!objeto.is_empty());
    }
}

#[test]
fn catch_no_perfil_cleanup_confere_seletor_e_remapeia_phi_do_pouso() {
    let chamada = |v| (ValueId(v), Instruction::CallStatic { symbol: "falha".into(), args: vec![], ret_ty: Type::Void }, Type::Void);
    let drop = |v| (ValueId(v), Instruction::ArcDrop { value: Operand::Val(ValueId(1)) }, Type::Void);
    let desvio = |p, n| Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(false)), then_block: BlockId(p), else_block: BlockId(n) };
    let f = funcao("misto", vec![(ValueId(0), "x".into(), Type::Ref)], Type::Void, vec![
        BasicBlock { id: BlockId(0), instructions: vec![
            (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref), chamada(2),
        ], terminator: desvio(4, 1) },
        BasicBlock { id: BlockId(1), instructions: vec![chamada(5)], terminator: desvio(2, 3) },
        BasicBlock { id: BlockId(2), instructions: vec![drop(3)], terminator: Terminator::Return(None) },
        BasicBlock { id: BlockId(3), instructions: vec![drop(4)], terminator: Terminator::Return(None) },
        BasicBlock { id: BlockId(4), instructions: vec![], terminator: Terminator::Branch(BlockId(5)) },
        BasicBlock { id: BlockId(5), instructions: vec![
            (ValueId(6), Instruction::Phi { incoming: vec![(BlockId(4), Operand::Constant(Constant::Int(1)))], ty: Type::I64 }, Type::I64), drop(7),
        ], terminator: Terminator::Return(None) },
    ]);
    let t = TabelasDaFuncao {
        invocacoes: [(ValueId(2), BlockId(4)), (ValueId(5), BlockId(2))].into(),
        pousos: [BlockId(2), BlockId(4)].into(),
        saidas: [(BlockId(2), SaidaPorExcecao::Retoma)].into(), ..Default::default()
    };
    let falha = funcao("falha", vec![], Type::Void, vec![BasicBlock {
        id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None),
    }]);
    let mut funcoes = vec![f, falha];
    let mut plano = crate::otimizar::arc::PlanoFuncaoDart::default();
    plano.tabelas = t;
    let mut folha = crate::otimizar::arc::PlanoFuncaoDart::default();
    folha.tabelas.confere_pilha = true;
    let mut planos = std::collections::HashMap::from([("misto".into(), plano), ("falha".into(), folha)]);
    assert_eq!(crate::otimizar::arc::inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(), (0, 0));
    let f = &funcoes[0];
    let t = &planos["misto"].tabelas;
    let certificado = t.cleanup_estrangeiro.as_ref().unwrap();
    assert_eq!(certificado.owners(BlockId(4)), &[ValueId(1)]);
    let mut obsoleta = f.clone();
    obsoleta.name.push_str(" alterada");
    assert!(certificado.conferir(&obsoleta, t).is_err());
    let mut tabelas_obsoletas = t.clone();
    tabelas_obsoletas.invocacoes.remove(&ValueId(2));
    assert!(certificado.conferir(f, &tabelas_obsoletas).is_err());
    let m = Module::new();
    let mut e = LlvmEmitter::new(&m);
    e.tab = Some(t);
    e.emit_function(f);
    let ir = e.out;
    if super::externs::conferir_efeitos() {
        assert!(ir.contains("%efnivel = call i64 @dartforge_efeitos_nivel()"));
        let restauracao = ir.find("call void @dartforge_efeitos_restaurar(i64 %efnivel)").unwrap();
        assert!(restauracao < ir.find("lpad4.estrangeira:").unwrap());
    }
    assert!(ir.contains("%lpseletor4 = extractvalue { ptr, i32 } %lpad4, 1"));
    assert!(ir.contains("%lptipo4 = call i32 @llvm.eh.typeid.for(ptr null)"));
    assert!(ir.contains("lpad4.estrangeira:\n  call void @dartforge_arc_release(i64 %v1)"));
    let estrangeira = ir.split("lpad4.estrangeira:\n").nth(1).unwrap().split("lpad4.dart:").next().unwrap();
    assert!(estrangeira.contains("resume { ptr, i32 } %lpad4"));
    assert_eq!(estrangeira.matches("@dartforge_arc_release").count(), 1);
    if ir.contains("%gcant =") {
        assert!(estrangeira.contains("store ptr %gcant, ptr %ctxtopo"));
    }
    assert!(ir.contains("%lpad4.dart ]"), "{ir}");
    assert!(!ir.contains("%lpseletor2"));
    #[cfg(feature = "llvm-embutido")]
    conferir_objetos_cleanup(&ir);
}
