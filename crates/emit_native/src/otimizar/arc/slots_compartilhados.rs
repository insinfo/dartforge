//! Proveniência de slots Ref emprestados a funções diretas fechadas.
//! O endereço não é um owner: estes fatos não autorizam Load Ref bruto nem
//! substituem retenção, inicialização ou cleanup do quadro proprietário.

use super::super::operandos::{operandos, operandos_do_terminador};
use crate::hir::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Reconstrói os slots Ref locais e parâmetros que somente os repassam.
///
/// Parte de Alloca Ref na entrada e segue argumentos de chamadas diretas,
/// inclusive recursivas. Exige o conjunto fechado no mesmo módulo e confere
/// todos os chamadores desse conjunto para cada parâmetro,
/// tipos reais SSA, leituras, escritas e ausência de escape do endereço.
/// Entradas materializadas como função/closure ou publicadas em tabelas
/// não são fechadas. Não produz contratos de ownership, nem prova que um
/// slot está inicializado ou que seu conteúdo continua vivo.
/// Tracing devolve um inventário vazio. Não altera corpos ou metadados.
///
/// # Erros
/// Símbolos repetidos, SSA inválido, tipo incompatível, chamador sem origem
/// local ou endereço exposto a uma operação não coberta.
///
/// ```
/// use dartforge_emit_native::{hir::Module, otimizar::arc::*};
/// let mut m = Module::new(); m.memoria_arc = true;
/// assert!(auditar_slots_ref_compartilhados(&m)?.is_empty());
/// # Ok::<(), String>(())
/// ```
pub fn auditar_slots_ref_compartilhados(
    m: &Module,
) -> Result<HashMap<String, HashSet<ValueId>>, String> {
    if !m.memoria_arc {
        return Ok(HashMap::new());
    }
    let mut nomes = HashMap::new();
    for (fi, f) in m.functions.iter().enumerate() {
        if nomes.insert(f.symbol.as_str(), fi).is_some() {
            return Err(format!("slots Ref ARC: símbolo repetido {}", f.symbol));
        }
    }
    let mut abertos: HashSet<&str> = m
        .retornos_uniformes_arc
        .iter()
        .map(String::as_str)
        .chain(m.ajudantes.iter().map(|(_, s)| s.as_str()))
        .chain(m.classes.iter().flat_map(|c| {
            c.vtable
                .iter()
                .map(|(_, s)| s.as_str())
                .chain(c.to_string_symbol.iter().map(String::as_str))
        }))
        .chain(m.entry_symbol.iter().map(String::as_str))
        .chain(m.chamar_dart.iter().map(String::as_str))
        .chain(m.ffi_callbacks.iter().map(|c| c.corpo.as_str()))
        .chain(m.raizes_da_fonte.iter().map(String::as_str))
        .chain(m.funcoes_de_tabela.values().map(String::as_str))
        .chain(
            m.tabelas_de_metodos
                .iter()
                .flat_map(|(_, _, t)| t.iter().map(|(_, s)| s.as_str())),
        )
        .collect();
    for f in &m.functions {
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            match i {
                Instruction::Const(Constant::Funcao(s))
                | Instruction::TearOff { code_symbol: s }
                | Instruction::AllocClosure { code_symbol: s, .. } => {
                    abertos.insert(s.as_str());
                }
                Instruction::AllocClosureTipada {
                    code_symbol,
                    tipado,
                    ..
                } => {
                    abertos.insert(code_symbol.as_str());
                    abertos.insert(tipado.as_str());
                }
                Instruction::TabelaDeFuncoes(ss) => {
                    abertos.extend(ss.iter().map(String::as_str));
                }
                _ => {}
            }
        }
    }
    // Inclui constantes de função em qualquer posição, inclusive retornos.
    let mut simbolos_operandos = HashSet::new();
    for f in &m.functions {
        let mut constante = |o: &Operand| {
            if let Operand::Constant(Constant::Funcao(s)) = o {
                simbolos_operandos.insert(s.clone());
            }
        };
        for b in &f.blocks {
            for (_, i, _) in &b.instructions {
                operandos(i, &mut constante);
            }
            operandos_do_terminador(&b.terminator, &mut constante);
        }
    }
    let mut slots = HashSet::new();
    let mut fila = VecDeque::new();
    for (fi, f) in m.functions.iter().enumerate() {
        let raizes: HashSet<_> = f
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter_map(|(v, i, _)| matches!(i, Instruction::Alloca(Type::Ref)).then_some(*v))
            .collect();
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallStatic { args, .. } = i {
                for op in args {
                    if let Operand::Val(v) = op
                        && raizes.contains(v)
                        && slots.insert((fi, *v))
                    {
                        fila.push_back((fi, *v));
                    }
                }
            }
        }
    }
    while let Some((fi, p)) = fila.pop_front() {
        let f = &m.functions[fi];
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallStatic { symbol, args, .. } = i {
                for (ai, op) in args.iter().enumerate() {
                    if op != &Operand::Val(p) {
                        continue;
                    }
                    let ci = *nomes.get(symbol.as_str()).ok_or_else(|| {
                        format!("slots Ref ARC em {}: alvo sem corpo {symbol}", f.symbol)
                    })?;
                    let callee = &m.functions[ci];
                    if abertos.contains(symbol.as_str()) || simbolos_operandos.contains(symbol) {
                        return Err(format!("slots Ref ARC: entrada aberta {symbol}"));
                    }
                    if args.len() != callee.params.len() || callee.params[ai].2 != Type::Ptr {
                        return Err(format!(
                            "slots Ref ARC: assinatura incompatível de {symbol}"
                        ));
                    }
                    let chave = (ci, callee.params[ai].0);
                    if slots.insert(chave) {
                        fila.push_back(chave);
                    }
                }
            }
        }
    }
    for (fi, f) in m.functions.iter().enumerate() {
        let locais: HashSet<_> = slots
            .iter()
            .filter_map(|(i, v)| (*i == fi).then_some(*v))
            .collect();
        if locais.is_empty() {
            continue;
        }
        super::ssa::verificar(f)?;
        let tipos: HashMap<_, _> = f
            .params
            .iter()
            .map(|(v, _, t)| (*v, *t))
            .chain(
                f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(v, _, t)| (*v, *t)),
            )
            .collect();
        let erro = |v: ValueId, msg: &str| format!("slots Ref ARC em {} v{}: {msg}", f.symbol, v.0);
        for (bi, b) in f.blocks.iter().enumerate() {
            for (v, i, ty) in &b.instructions {
                if locais.contains(v)
                    && (!matches!(i, Instruction::Alloca(Type::Ref)) || bi != 0 || *ty != Type::Ptr)
                {
                    return Err(erro(*v, "origem exige Alloca Ref Ptr na entrada"));
                }
                let mut permitidos = Vec::new();
                match i {
                    Instruction::Load {
                        ptr: Operand::Val(p),
                        ty: lido,
                    } if locais.contains(p) => {
                        if *lido != Type::Ref || *ty != Type::Ref {
                            return Err(erro(*v, "leitura exige Ref"));
                        }
                        permitidos.push(*p);
                    }
                    Instruction::Store {
                        ptr: Operand::Val(p),
                        val,
                    } if locais.contains(p) => {
                        let ref_real = match val {
                            Operand::Val(v) => tipos.get(v) == Some(&Type::Ref),
                            Operand::Constant(Constant::Null) => true,
                            _ => false,
                        };
                        if !ref_real || *ty != Type::Void {
                            return Err(erro(*v, "gravação exige Ref real"));
                        }
                        permitidos.push(*p);
                    }
                    Instruction::CallStatic {
                        symbol,
                        args,
                        ret_ty,
                    } => {
                        for (ai, op) in args.iter().enumerate() {
                            if let Operand::Val(p) = op
                                && locais.contains(p)
                            {
                                let ci = nomes[symbol.as_str()];
                                let callee = &m.functions[ci];
                                if *ty != callee.return_ty
                                    || *ret_ty != callee.return_ty
                                    || !slots.contains(&(ci, callee.params[ai].0))
                                {
                                    return Err(erro(*v, "chamada diverge do corpo"));
                                }
                                permitidos.push(*p);
                            }
                        }
                    }
                    _ => {}
                }
                let mut usados = Vec::new();
                operandos(i, &mut |o| {
                    if let Operand::Val(p) = o
                        && locais.contains(p)
                    {
                        usados.push(*p);
                    }
                });
                if usados != permitidos {
                    return Err(erro(*v, "endereço escapa ou ganha alias"));
                }
            }
            let mut escapa = false;
            operandos_do_terminador(&b.terminator, &mut |o| {
                escapa |= matches!(o, Operand::Val(p) if locais.contains(p));
            });
            if escapa {
                return Err(erro(ValueId(0), "endereço escapa pelo terminador"));
            }
        }
    }
    // A origem deve valer em todos os chamadores, não apenas na aresta que
    // descobriu o parâmetro. Uma aresta externa invalida o empréstimo inteiro.
    for (fi, f) in m.functions.iter().enumerate() {
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } = i
                && let Some(&ci) = nomes.get(symbol.as_str())
            {
                let callee = &m.functions[ci];
                for (ai, (p, _, _)) in callee.params.iter().enumerate() {
                    if !slots.contains(&(ci, *p)) {
                        continue;
                    }
                    if args.len() != callee.params.len()
                        || *ret_ty != callee.return_ty
                        || !matches!(args.get(ai), Some(Operand::Val(v)) if slots.contains(&(fi, *v)))
                    {
                        return Err(format!(
                            "slots Ref ARC: chamador {} sem origem fechada para {symbol}",
                            f.symbol
                        ));
                    }
                }
            }
        }
    }
    let mut resultado = HashMap::<String, HashSet<ValueId>>::new();
    for (fi, v) in slots {
        resultado
            .entry(m.functions[fi].symbol.clone())
            .or_default()
            .insert(v);
    }
    Ok(resultado)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn val(n: u32) -> Operand {
        Operand::Val(ValueId(n))
    }

    fn chamar(nome: &str, args: Vec<Operand>) -> Instruction {
        Instruction::CallStatic {
            symbol: nome.into(),
            args,
            ret_ty: Type::Void,
        }
    }

    fn funcao(
        nome: &str,
        parametro: bool,
        instructions: Vec<(ValueId, Instruction, Type)>,
    ) -> Function {
        Function {
            symbol: nome.into(),
            name: nome.into(),
            depuracao: None,
            params: if parametro {
                vec![(ValueId(0), "slot".into(), Type::Ptr)]
            } else {
                vec![]
            },
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions,
                terminator: Terminator::Return(None),
            }],
        }
    }

    fn modulo() -> Module {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions.push(funcao(
            "raiz",
            false,
            vec![
                (ValueId(0), Instruction::Alloca(Type::Ref), Type::Ptr),
                (
                    ValueId(1),
                    Instruction::Store {
                        ptr: val(0),
                        val: Operand::Constant(Constant::Null),
                    },
                    Type::Void,
                ),
                (ValueId(2), chamar("direta", vec![val(0)]), Type::Void),
            ],
        ));
        m.functions.push(funcao(
            "direta",
            true,
            vec![
                (
                    ValueId(1),
                    Instruction::Load {
                        ptr: val(0),
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::Store {
                        ptr: val(0),
                        val: val(1),
                    },
                    Type::Void,
                ),
            ],
        ));
        m
    }

    #[test]
    fn reconstrucao_recursiva_independe_da_ordem_e_nao_produz_owner() {
        let mut m = modulo();
        m.functions[1].blocks[0].instructions.push((
            ValueId(3),
            chamar("repasse", vec![val(0)]),
            Type::Void,
        ));
        m.functions.push(funcao(
            "repasse",
            true,
            vec![(ValueId(1), chamar("direta", vec![val(0)]), Type::Void)],
        ));
        let esperado = HashMap::from([
            ("raiz".into(), HashSet::from([ValueId(0)])),
            ("direta".into(), HashSet::from([ValueId(0)])),
            ("repasse".into(), HashSet::from([ValueId(0)])),
        ]);
        let antes = format!("{m:?}");
        assert_eq!(auditar_slots_ref_compartilhados(&m).unwrap(), esperado);
        assert_eq!(format!("{m:?}"), antes);
        m.functions.reverse();
        assert_eq!(auditar_slots_ref_compartilhados(&m).unwrap(), esperado);
        // Mesmo um ciclo fechado de Ptr com Load Ref não cria uma origem:
        // somente a ativação que aloca o slot ancora este inventário.
        m.functions.retain(|f| f.symbol != "raiz");
        assert!(auditar_slots_ref_compartilhados(&m).unwrap().is_empty());
    }

    #[test]
    fn qualquer_chamador_opaco_invalida_parametro_recursivo() {
        for caso in 0..3 {
            let mut m = modulo();
            let args = match caso {
                0 => vec![val(0)],
                1 => vec![Operand::Constant(Constant::Int(0))],
                _ => vec![],
            };
            m.functions.push(funcao(
                "externa",
                true,
                vec![(ValueId(1), chamar("direta", args), Type::Void)],
            ));
            let antes = format!("{m:?}");
            assert!(
                auditar_slots_ref_compartilhados(&m)
                    .unwrap_err()
                    .contains("chamador"),
                "caso {caso}"
            );
            assert_eq!(format!("{m:?}"), antes);
        }
    }

    #[test]
    fn simbolo_materializado_nao_vira_captura_fechada() {
        for caso in 0..8 {
            let mut m = modulo();
            match caso {
                0 => m.ajudantes.push(("nativo".into(), "direta".into())),
                1 => m.entry_symbol = Some("direta".into()),
                2 => m.functions[0].blocks[0].instructions.push((
                    ValueId(3),
                    Instruction::Const(Constant::Funcao("direta".into())),
                    Type::Ptr,
                )),
                3 => m.functions[0].blocks[0].instructions.push((
                    ValueId(3),
                    Instruction::TearOff {
                        code_symbol: "direta".into(),
                    },
                    Type::Ref,
                )),
                4 => m.functions[0].blocks[0].instructions.push((
                    ValueId(3),
                    Instruction::TabelaDeFuncoes(vec!["direta".into()]),
                    Type::Ptr,
                )),
                5 => {
                    m.retornos_uniformes_arc.insert("direta".into());
                }
                6 => m.raizes_da_fonte.push("direta".into()),
                _ => {
                    m.funcoes_de_tabela.insert(42, "direta".into());
                }
            }
            assert!(
                auditar_slots_ref_compartilhados(&m)
                    .unwrap_err()
                    .contains("entrada aberta"),
                "caso {caso}"
            );
        }
    }

    #[test]
    fn rejeita_alias_escape_e_interpretacao_de_inteiro_como_ref() {
        for caso in 0..8 {
            let mut m = modulo();
            let b = &mut m.functions[1].blocks[0];
            match caso {
                0 => {
                    b.instructions[0].1 = Instruction::Load {
                        ptr: val(0),
                        ty: Type::I64,
                    }
                }
                1 => b.instructions[0].2 = Type::I64,
                2 => {
                    b.instructions[1].1 = Instruction::Store {
                        ptr: val(0),
                        val: Operand::Constant(Constant::Int(0)),
                    }
                }
                3 => {
                    b.instructions[1].1 = Instruction::Store {
                        ptr: val(0),
                        val: val(0),
                    }
                }
                4 => b.instructions.push((
                    ValueId(3),
                    Instruction::Bitcast {
                        op: val(0),
                        to: Type::I64,
                    },
                    Type::I64,
                )),
                5 => b.terminator = Terminator::Return(Some(val(0))),
                6 => b.instructions.push((
                    ValueId(3),
                    Instruction::CallRuntime {
                        name: "opaco".into(),
                        args: vec![(val(0), Type::Ptr)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                _ => {
                    b.instructions
                        .push((ValueId(3), chamar("sem_corpo", vec![val(0)]), Type::Void))
                }
            }
            assert!(auditar_slots_ref_compartilhados(&m).is_err(), "caso {caso}");
        }
    }

    #[test]
    fn assinatura_e_origem_ssa_sao_conferidas_sem_confiar_em_ptr() {
        for caso in 0..5 {
            let mut m = modulo();
            match caso {
                0 => m.functions[1].params[0].2 = Type::I64,
                1 => m.functions[0].blocks[0].instructions[0].2 = Type::I64,
                2 => m.functions[0].blocks[0].instructions[2].2 = Type::Ref,
                3 => {
                    m.functions[0].blocks[0].instructions[2].1 = Instruction::CallStatic {
                        symbol: "direta".into(),
                        args: vec![val(0)],
                        ret_ty: Type::Ref,
                    }
                }
                _ => m.functions[1]
                    .params
                    .push((ValueId(0), "duplicado".into(), Type::Ptr)),
            }
            assert!(auditar_slots_ref_compartilhados(&m).is_err(), "caso {caso}");
        }
    }

    #[test]
    fn tracing_preserva_corpo_sem_inferir_papel_arc() {
        let mut m = modulo();
        m.memoria_arc = false;
        let antes = format!("{m:?}");
        assert!(auditar_slots_ref_compartilhados(&m).unwrap().is_empty());
        assert_eq!(format!("{m:?}"), antes);
    }
}
