//! Prova AOT da preparação de AllocObject com layout completo de 40 campos.
//! Confere bits escalares, aresta Ref e morte após liberação do retorno Owned.
//! Tracing usa tokens explícitos apenas no harness de comparação.
//! Não certifica lowering de fonte Dart, guardas de getters ou recarga.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn modulo(quadro: bool, tipada: bool) -> Result<Module, String> {
    let mut m = Module::new();
    m.memoria_arc = true;
    m.entry_symbol = Some("prova_alocacao_campos".into());
    m.classes.push(ClassDef {
        id: 123,
        name: "C".into(),
        field_count: 40,
        vtable: vec![],
        to_string_symbol: None,
    });
    let mut layout = vec![Type::I64; 40];
    layout[1] = Type::F64;
    layout[2] = Type::I1;
    layout[3] = Type::I8;
    layout[39] = Type::Ref;
    m.layouts_campos_arc.insert(123, layout);
    let mut fields = vec![Operand::Constant(Constant::Int(85)); 40];
    fields[1] = Operand::Constant(Constant::Double(-0.0));
    fields[2] = Operand::Constant(Constant::Bool(true));
    fields[3] = Operand::Val(ValueId(3));
    fields[39] = Operand::Val(ValueId(0));
    if quadro {
        m.classes.clear(); m.layouts_campos_arc.clear();
        fields[0] = Operand::Constant(Constant::Int(0));
        fields[6] = fields[1].clone(); fields[7] = fields[2].clone(); fields[8] = fields[3].clone();
        fields[1..5].fill(Operand::Constant(Constant::Null));
        fields[5] = Operand::Constant(Constant::Int(0));
    }
    m.functions.push(Function {
        symbol: "criar_objeto".into(),
        name: "criar_objeto".into(),
        depuracao: None,
        params: if tipada { vec![(ValueId(10), "contexto".into(), Type::Ref)] } else { vec![] },
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(0),
                    Instruction::Box {
                        op: Operand::Constant(Constant::Int(i64::MAX)),
                        from: Type::I64,
                    },
                    Type::Ref,
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
                    ValueId(1),
                    Instruction::AllocObject {
                        class_id: if quadro { dartforge_emit_native::lower::async_sm::ID_QUADRO_ASYNC } else { 123 },
                        fields,
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    });
    let mut plano = PlanoFuncaoDart::default();
    plano.tokens.retorno = RetornoTokens::Owned;
    if quadro {
        m.functions.push(Function { symbol: "ler_quadro".into(), name: "ler_quadro".into(), depuracao: None,
            params: vec![(ValueId(0), "quadro".into(), Type::Ref)], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 39 }, Type::Ref),
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
    }
    let mut planos = HashMap::from([("criar_objeto".into(), plano)]);
    if quadro {
        let mut p = PlanoFuncaoDart::default(); p.tokens.retorno = RetornoTokens::Owned;
        planos.insert("ler_quadro".into(), p);
    }
    let produzido = preparar_arc_modulo_dart(&mut m, &mut planos)?;
    let esperado = if quadro { (1, 1) } else { (0, 1) };
    if produzido != esperado {
        return Err(format!("esperado {esperado:?}, encontrado {produzido:?}"));
    }
    let antes = format!("{m:?}/{planos:?}");
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}/{planos:?}") != antes
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
    let mut controle = args.next();
    let quadro = controle.as_deref() == Some("--quadro");
    let tipada = controle.as_deref() == Some("--tipada");
    if quadro || tipada { controle = args.next(); }
    if args.next().is_some() { return Err("argumentos excedentes".into()); }
    let mut m = modulo(quadro, tipada)?;
    m.memoria_arc = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    let cid = if quadro { dartforge_emit_native::lower::async_sm::ID_QUADRO_ASYNC } else { 123 };
    let owned = format!("call i64 @dartforge_arc_objeto_owned_v1(i64 {cid}, i64 40)");
    let sem_owner = format!("call i64 @dartforge_object_new(i64 {cid}, i64 40)");
    let reter_leitura = m.functions.iter().find(|f| f.symbol == "ler_quadro")
        .and_then(|f| f.blocks.iter().flat_map(|b| &b.instructions).find_map(|(_, i, _)|
            if let Instruction::ArcCopy { value: Operand::Val(v) } = i {
                Some(format!("  call void @dartforge_arc_retain(i64 %v{})\n", v.0))
            } else { None })).unwrap_or_default();
    if let Some(controle) = controle {
        let (antes, depois) = match controle.as_str() {
            "sem-drop-filho" => ("  call void @dartforge_arc_release(i64 %v0)\n", ""),
            "sem-retencao-leitura" if quadro => (reter_leitura.as_str(), ""),
            "sem-owner-objeto" => (
                owned.as_str(),
                sem_owner.as_str(),
            ),
            _ => return Err(format!("controle desconhecido: {controle}")),
        };
        if ir.matches(antes).count() != 1 {
            return Err(format!("controle sem alvo único: {controle}"));
        }
        ir = ir.replace(antes, depois);
    }
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
define void @prova_alocacao_campos() {
  %obj = call i64 @criar_objeto()
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %owned = icmp eq i8 %estado, 3
  br i1 %owned, label %campos, label %falha
campos:
  %a = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 0)
  %b = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 1)
  %c = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 2)
  %d = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 3)
  %aok = icmp eq i64 %a, 85
  %bok = icmp eq i64 %b, -9223372036854775808
  %cok = icmp eq i64 %c, 1
  %dok = icmp eq i64 %d, 255
  %ab = and i1 %aok, %bok
  %cd = and i1 %cok, %dok
  %bits = and i1 %ab, %cd
  br i1 %bits, label %filho, label %falha
filho:
  %ref = call i64 @dartforge_arc_ler_campo_ref_v1(i64 %obj, i64 39)
  %valor = call i64 @dartforge_unbox_int(i64 %ref)
  %mint = icmp eq i64 %valor, 9223372036854775807
  %ef = call i8 @dartforge_arc_observar_heap_v1(i64 %ref)
  %sem_owner = icmp eq i8 %ef, 1
  %ok = and i1 %mint, %sem_owner
  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %obj)
  call void @dartforge_arc_collect()
  %eo = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %em = call i8 @dartforge_arc_observar_heap_v1(i64 %ref)
  %morto = icmp eq i8 %eo, 0
  %filho_morto = icmp eq i8 %em, 0
  %fim = and i1 %morto, %filho_morto
  br i1 %fim, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    if quadro {
        // O cabeçalho reserva quatro Ref e a tupla RTI; os escalares desta
        // fixture ficam na cauda. O heap observa o mapa real da instância.
        for (de, para) in [(1, 6), (2, 7), (3, 8)] {
            ir = ir.replace(&format!("i64 %obj, i64 {de})"), &format!("i64 %obj, i64 {para})"));
        }
        ir = ir.replace("%aok = icmp eq i64 %a, 85", "%aok = icmp eq i64 %a, 0");
        ir = ir.replace("%ref = call i64 @dartforge_arc_ler_campo_ref_v1(i64 %obj, i64 39)",
            "%ref = call i64 @ler_quadro(i64 %obj)");
        ir = ir.replace("%sem_owner = icmp eq i8 %ef, 1", "%sem_owner = icmp eq i8 %ef, 3");
        ir = ir.replace("  call void @dartforge_arc_release(i64 %obj)\n  call void @dartforge_arc_collect()",
            "  call void @dartforge_arc_release(i64 %obj)\n  call void @dartforge_arc_collect()\n  %sobreviveu = call i8 @dartforge_arc_observar_heap_v1(i64 %ref)\n  %ainda_owned = icmp eq i8 %sobreviveu, 3\n  br i1 %ainda_owned, label %soltar_leitura, label %falha\nsoltar_leitura:\n  call void @dartforge_arc_release(i64 %ref)\n  call void @dartforge_arc_collect()");
    }
    if tipada {
        // A fixture verificou o corpo e mantém a imagem estática. O registro
        // declara essa ABI, e a consulta confere os campos antes de chamar.
        ir = ir.replace("  %obj = call i64 @criar_objeto()",
            "  %pc = ptrtoint ptr @criar_objeto to i64\n  %versao = call i64 @dartforge_arc_registrar_tipada_v1(i64 %pc, i64 6)\n  %closure = call i64 @dartforge_arc_closure_owned_v1(i64 0, i64 0, i64 %pc, i64 6)\n  %alvo = call i64 @dartforge_arc_alvo_tipada_v1(i64 %closure, i64 6)\n  %funcao = inttoptr i64 %alvo to ptr\n  %obj = call i64 %funcao(i64 0)\n  call void @dartforge_arc_release(i64 %closure)\n  call void @dartforge_arc_retirar_tipada_v1(i64 %pc, i64 6, i64 %versao)");
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
    fn preparacao_de_alocacao_e_campos_gera_ir_verificado() {
        for (quadro, tipada) in [(false, false), (true, false), (false, true)] {
            LlvmEmitter::new(&modulo(quadro, tipada).unwrap()).emit_all();
        }
    }
}
