//! Prova AOT do literal misto produzido em HIR, com owner e arestas Ref.
//! Tracing conserva sua fábrica e o harness retém o retorno explicitamente.
//! Controles negativos alteram somente o IR já preparado e verificado.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

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
    let controle = args.next();
    if args.next().is_some() {
        return Err("argumentos excedentes".into());
    }
    let mut m = Module::new();
    m.memoria_arc = arc;
    m.entry_symbol = Some("prova_literal".into());
    let bits = 0xfff8_1234_5678_9abc_u64;
    let mut instructions = vec![(
        ValueId(1),
        Instruction::AllocList {
            elements: vec![
                (Operand::Val(ValueId(0)), 3),
                (Operand::Constant(Constant::Int(i64::MAX)), 1),
                (Operand::Constant(Constant::Double(f64::from_bits(bits))), 4),
                (Operand::Constant(Constant::Bool(true)), 2),
                (Operand::Val(ValueId(0)), 3),
            ],
        },
        Type::Ref,
    )];
    if arc {
        instructions.push((
            ValueId(2),
            Instruction::CallRuntime {
                name: "dartforge_arc_collect".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
    }
    m.functions.push(Function {
        symbol: "literal_misto".into(),
        name: "literal_misto".into(),
        depuracao: None,
        params: vec![(ValueId(0), "filho".into(), Type::Ref)],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions,
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    });
    let mut plano = PlanoFuncaoDart::default();
    plano.tokens.retorno = RetornoTokens::Owned;
    let mut planos = HashMap::from([("literal_misto".into(), plano)]);
    preparar_arc_modulo_dart(&mut m, &mut planos)?;
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
    let retencao = if arc {
        ""
    } else {
        "  call void @dartforge_arc_retain(i64 %lista)\n"
    };
    ir.push_str(&format!(
        r#"
define void @prova_literal() {{
entry:
  %filho = call i64 @dartforge_arc_lista_nova_owned_v1(ptr null, i64 0, i64 8, i64 0)
  %lista = call i64 @literal_misto(i64 %filho)
{retencao}  call void @dartforge_arc_release(i64 %filho)
  call void @dartforge_arc_collect()
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %lista)
  %owner = icmp eq i8 %estado, 3
  %fe = call i8 @dartforge_arc_observar_heap_v1(i64 %filho)
  %aresta = icmp eq i8 %fe, 1
  %vivo = and i1 %owner, %aresta
  br i1 %vivo, label %ler, label %falha
ler:
  %n = call i64 @dartforge_arc_lista_len_v1(i64 %lista)
  %n_ok = icmp eq i64 %n, 5
  %r0 = call i64 @dartforge_arc_lista_ref_owned_v1(i64 %lista, i64 0)
  %r1 = call i64 @dartforge_arc_lista_ref_owned_v1(i64 %lista, i64 1)
  %r2 = call i64 @dartforge_arc_lista_ref_owned_v1(i64 %lista, i64 2)
  %r3 = call i64 @dartforge_arc_lista_ref_owned_v1(i64 %lista, i64 3)
  %r4 = call i64 @dartforge_arc_lista_ref_owned_v1(i64 %lista, i64 4)
  %int = call i64 @dartforge_unbox_int(i64 %r1)
  %double = call double @dartforge_unbox_double(i64 %r2)
  %dbits = bitcast double %double to i64
  %bool = call i8 @dartforge_unbox_bool(i64 %r3)
  %a = icmp eq i64 %r0, %filho
  %b = icmp eq i64 %r4, %filho
  %c = icmp eq i64 %int, 9223372036854775807
  %d = icmp eq i64 %dbits, {bits}
  %e = icmp eq i8 %bool, 1
  %ab = and i1 %a, %b
  %cd = and i1 %c, %d
  %abcd = and i1 %ab, %cd
  %abcde = and i1 %abcd, %e
  %ok = and i1 %abcde, %n_ok
  call void @dartforge_arc_release(i64 %r0)
  call void @dartforge_arc_release(i64 %r1)
  call void @dartforge_arc_release(i64 %r2)
  call void @dartforge_arc_release(i64 %r3)
  call void @dartforge_arc_release(i64 %r4)
  br i1 %ok, label %soltar, label %falha
soltar:
  call void @dartforge_arc_release(i64 %lista)
  call void @dartforge_arc_collect()
  %fim = call i8 @dartforge_arc_observar_heap_v1(i64 %lista)
  %ff = call i8 @dartforge_arc_observar_heap_v1(i64 %filho)
  %morta = icmp eq i8 %fim, 0
  %filhomorto = icmp eq i8 %ff, 0
  %fi = call i8 @dartforge_arc_observar_heap_v1(i64 %r1)
  %fd = call i8 @dartforge_arc_observar_heap_v1(i64 %r2)
  %imorto = icmp eq i8 %fi, 0
  %dmorto = icmp eq i8 %fd, 0
  %caixasmortas = and i1 %imorto, %dmorto
  %listamorta = and i1 %morta, %filhomorto
  %liberados = and i1 %listamorta, %caixasmortas
  br i1 %liberados, label %sucesso, label %falha
sucesso:
  call void @dartforge_print_handle(i64 3)
  ret void
falha:
  call void @llvm.trap()
  unreachable
}}
"#
    ));
    match controle.as_deref() {
        None => {}
        Some("sem-owner") => {
            ir = ir.replace("  call void @dartforge_arc_release(i64 %filho)\n",
                "  call void @dartforge_arc_release(i64 %lista)\n  call void @dartforge_arc_release(i64 %filho)\n");
        }
        Some("sem-liberar") => {
            ir = ir.replace(
                "soltar:\n  call void @dartforge_arc_release(i64 %lista)\n",
                "soltar:\n",
            );
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
