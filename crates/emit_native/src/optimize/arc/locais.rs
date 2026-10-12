//! Contratos de locais escalares privados da ativação, sem aliases ou escape.
//! Campos, locais Ref e endereços importados exigem outros produtores.

use super::{EfeitoTokens, Ownership, PlanoFuncaoDart, PlanoTokens};
use crate::hir::*;
use crate::optimize::{
    cfg::Cfg,
    operandos::{operandos, operandos_do_terminador},
};
use std::collections::{HashMap, HashSet};

pub(super) fn produzir(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
) -> Result<HashMap<ValueId, Type>, String> {
    let (locais, produzidas) = analisar(f, classes, plano)?;
    for v in produzidas {
        classes.insert(v, Ownership::Trivial);
        plano.instrucoes.insert(v, EfeitoTokens::default());
    }
    for p in plano.enderecos_recebidos.keys() {
        classes.insert(*p, Ownership::Trivial);
    }
    Ok(locais)
}

/// Representações sem handle de um local privado: escalares e vetores SIMD.
fn escalar(t: &Type) -> bool {
    matches!(t, Type::I1 | Type::I8 | Type::I64 | Type::F64) || t.e_vetor()
}

/// Captura por endereço das funções locais diretas (`lower/funcoes_diretas.rs`):
/// o parâmetro `Ptr` da direta aponta o `alloca` escalar de quem a declara,
/// que existe enquanto ela roda. O tipo de cada parâmetro vem dos usos dele
/// (leitura, gravação, repasse a outra direta) e dos endereços que os
/// chamadores cedem, em ponto fixo. Grava nos planos os endereços recebidos
/// e, por chamada, os cedidos; `analisar` confere cada corpo contra eles.
///
/// # Erros
/// Tipos divergentes para o mesmo parâmetro, ou chamada que passa a um
/// parâmetro escalar algo que não é um local escalar do mesmo tipo.
pub(super) fn produzir_enderecos(
    m: &Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<(), String> {
    let diretas = &m.funcoes_locais_diretas_arc;
    if diretas.is_empty() {
        return Ok(());
    }
    let por_simbolo: HashMap<&str, &Function> =
        m.functions.iter().map(|f| (f.symbol.as_str(), f)).collect();
    // (direta, posição do parâmetro Ptr) → tipo do local apontado.
    let mut tipos: HashMap<(&str, usize), Type> = HashMap::new();
    let ptr_de = |simbolo: &str, i: usize| {
        diretas.contains(simbolo)
            && por_simbolo.get(simbolo).is_some_and(|g| g.params.get(i).is_some_and(|p| p.2 == Type::Ptr))
    };
    // Endereços de locais escalares visíveis em `f`: os allocas dele e, numa
    // direta, os parâmetros já tipados.
    let enderecos = |f: &Function, tipos: &HashMap<(&str, usize), Type>| -> HashMap<ValueId, Type> {
        let mut e: HashMap<ValueId, Type> = f.blocks.first().into_iter()
            .flat_map(|b| &b.instructions)
            .filter_map(|(v, inst, ty)| match inst {
                Instruction::Alloca(t) if escalar(t) && *ty == Type::Ptr => Some((*v, *t)),
                _ => None,
            })
            .collect();
        for (i, (v, _, _)) in f.params.iter().enumerate() {
            if let Some(t) = tipos.get(&(f.symbol.as_str(), i)) {
                e.insert(*v, *t);
            }
        }
        e
    };
    fn definir<'a>(tipos: &mut HashMap<(&'a str, usize), Type>, chave: (&'a str, usize), t: Type) -> Result<bool, String> {
        match tipos.get(&chave) {
            Some(atual) if *atual != t => Err(format!(
                "captura por endereço em {}: parâmetro {} lido como {atual:?} e {t:?}",
                chave.0, chave.1
            )),
            Some(_) => Ok(false),
            None => {
                tipos.insert(chave, t);
                Ok(true)
            }
        }
    }
    loop {
        let mut mudou = false;
        for f in &m.functions {
            let tipos_valores: HashMap<ValueId, Type> = f.params.iter().map(|(v, _, t)| (*v, *t))
                .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t)))
                .collect();
            let visiveis = enderecos(f, &tipos);
            let parametro = |v: &ValueId| f.params.iter().position(|(p, _, t)| p == v && *t == Type::Ptr);
            for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                match inst {
                    Instruction::Load { ptr: Operand::Val(p), ty } if diretas.contains(&f.symbol) => {
                        if let Some(i) = parametro(p) {
                            mudou |= definir(&mut tipos, (f.symbol.as_str(), i), *ty)?;
                        }
                    }
                    Instruction::Store { ptr: Operand::Val(p), val: Operand::Val(x) } if diretas.contains(&f.symbol) => {
                        if let (Some(i), Some(t)) = (parametro(p), tipos_valores.get(x)) {
                            mudou |= definir(&mut tipos, (f.symbol.as_str(), i), *t)?;
                        }
                    }
                    Instruction::CallStatic { symbol, args, .. } if diretas.contains(symbol) => {
                        for (j, a) in args.iter().enumerate() {
                            let Operand::Val(a) = a else { continue };
                            if !ptr_de(symbol, j) {
                                continue;
                            }
                            // O chamador cede um local de tipo conhecido; um
                            // parâmetro repassado herda o tipo do callee.
                            if let Some(t) = visiveis.get(a) {
                                mudou |= definir(&mut tipos, (symbol.as_str(), j), *t)?;
                            } else if let (Some(i), Some(t)) = (parametro(a).filter(|_| diretas.contains(&f.symbol)), tipos.get(&(symbol.as_str(), j)).copied()) {
                                mudou |= definir(&mut tipos, (f.symbol.as_str(), i), t)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if !mudou {
            break;
        }
    }
    for f in &m.functions {
        let visiveis = enderecos(f, &tipos);
        let plano = planos.get_mut(&f.symbol).ok_or_else(|| format!("função Dart sem plano: {}", f.symbol))?;
        for (i, (v, _, _)) in f.params.iter().enumerate() {
            if let Some(t) = tipos.get(&(f.symbol.as_str(), i)).filter(|t| escalar(t)) {
                plano.tokens.enderecos_recebidos.insert(*v, *t);
            }
        }
        for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let Instruction::CallStatic { symbol, args, .. } = inst else { continue };
            let mut cedidos = Vec::new();
            for (j, a) in args.iter().enumerate() {
                let Some(t) = tipos.get(&(symbol.as_str(), j)).filter(|t| escalar(t)) else { continue };
                if !matches!(a, Operand::Val(a) if visiveis.get(a) == Some(t)) {
                    return Err(format!(
                        "captura por endereço em {}: v{} passa ao parâmetro {j} de {symbol} algo que não é local {t:?}",
                        f.symbol, v.0
                    ));
                }
                cedidos.push((j, *t));
            }
            if !cedidos.is_empty() {
                plano.tokens.enderecos_cedidos.insert(*v, cedidos);
            }
        }
    }
    Ok(())
}

/// Reconstrói o contrato no corpo atual sem completar metadados ausentes.
pub(super) fn verificar(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plano: &PlanoTokens,
) -> Result<(), String> {
    let (locais, produzidas) = analisar(f, classes, plano)?;
    for v in produzidas {
        if classes.get(&v) != Some(&Ownership::Trivial)
            || plano.instrucoes.get(&v) != Some(&EfeitoTokens::default())
        {
            return Err(format!("v{}: local escalar sem contrato completo", v.0));
        }
    }
    for p in plano.enderecos_recebidos.keys() {
        if classes.get(p) != Some(&Ownership::Trivial) {
            return Err(format!("v{}: endereço capturado sem contrato Trivial", p.0));
        }
    }
    verificar_valores(f, &locais, classes)
}

fn analisar(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plano: &PlanoTokens,
) -> Result<(HashMap<ValueId, Type>, HashSet<ValueId>), String> {
    let mut locais = HashMap::new();
    for (bi, b) in f.blocks.iter().enumerate() {
        for (v, inst, ty) in &b.instructions {
            if let Instruction::Alloca(t) = inst && escalar(t) {
                if bi != 0 || *ty != Type::Ptr {
                    return Err(format!(
                        "v{}: local escalar exige alloca Ptr na entrada",
                        v.0
                    ));
                }
                locais.insert(*v, *t);
            }
        }
    }
    // Endereços recebidos de quem declara a função direta: escalares, já
    // inicializados, com as mesmas regras de uso de um alloca próprio.
    let mut recebidos = HashSet::new();
    for (p, t) in &plano.enderecos_recebidos {
        if !f.params.iter().any(|(v, _, ty)| v == p && *ty == Type::Ptr)
            || !escalar(t)
        {
            return Err(format!("v{}: endereço capturado exige parâmetro Ptr escalar", p.0));
        }
        locais.insert(*p, *t);
        recebidos.insert(*p);
    }
    let mut cedidos_vistos = HashSet::new();
    if locais.is_empty() {
        if let Some(v) = plano.enderecos_cedidos.keys().min_by_key(|v| v.0) {
            return Err(format!("v{}: chamada cede endereço sem local escalar", v.0));
        }
        return Ok((locais, HashSet::new()));
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
    let local = |op: &Operand| match op {
        Operand::Val(v) if locais.contains_key(v) => Some(*v),
        _ => None,
    };
    let mut produzidas = HashSet::new();
    for b in &f.blocks {
        for (v, inst, ty) in &b.instructions {
            let mut permitido = None;
            let mut cedidos: Vec<ValueId> = Vec::new();
            let mut esperado = None;
            match inst {
                Instruction::CallStatic { args, .. } if plano.enderecos_cedidos.contains_key(v) => {
                    cedidos_vistos.insert(*v);
                    for (i, t) in &plano.enderecos_cedidos[v] {
                        match args.get(*i).and_then(local) {
                            Some(p) if locais[&p] == *t && !cedidos.contains(&p) => cedidos.push(p),
                            _ => {
                                return Err(format!(
                                    "v{}: argumento {i} não cede um local escalar {t:?} distinto",
                                    v.0
                                ));
                            }
                        }
                    }
                }
                Instruction::Alloca(_) if locais.contains_key(v) => esperado = Some(Type::Ptr),
                Instruction::Load { ptr, ty: lido } => {
                    if let Some(p) = local(ptr) {
                        permitido = Some(p);
                        if *lido != locais[&p] {
                            return Err(format!("v{}: leitura diverge do tipo do local", v.0));
                        }
                        esperado = Some(*lido);
                    }
                }
                Instruction::Store { ptr, val } => {
                    if let Some(p) = local(ptr) {
                        permitido = Some(p);
                        let tipo = match val {
                            Operand::Val(v) => tipos.get(v).copied(),
                            // Estado de late usa bytes literais (0/1/2). O
                            // tipo vem do slot, com faixa conferida antes da
                            // coerção textual; não se aplica a valores SSA.
                            Operand::Constant(Constant::Int(n))
                                if locais[&p] == Type::I8 && (0..=255).contains(n) =>
                            {
                                Some(Type::I8)
                            }
                            Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                            Operand::Constant(Constant::Double(_)) => Some(Type::F64),
                            Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
                            _ => None,
                        };
                        if tipo != Some(locais[&p]) {
                            return Err(format!("v{}: gravação diverge do tipo do local", v.0));
                        }
                        esperado = Some(Type::Void);
                    }
                }
                _ => {}
            }
            // Apenas o operando ptr de Load/Store pode usar o endereço. Uma
            // segunda ocorrência (p.ex. Store como valor) também é escape.
            // Uma chamada a função direta pode ceder cada local uma vez.
            let mut usos = 0;
            let mut inesperado = false;
            operandos(inst, &mut |op| {
                if let Some(p) = local(op) {
                    usos += 1;
                    inesperado |= permitido != Some(p) && !cedidos.contains(&p);
                }
            });
            if inesperado || usos != usize::from(permitido.is_some()) + cedidos.len() {
                return Err(format!(
                    "v{}: endereço de local escalar escapa ou tem alias",
                    v.0
                ));
            }
            if let Some(esperado) = esperado {
                if *ty != esperado
                    || classes.get(v).is_some_and(|c| *c != Ownership::Trivial)
                    || plano
                        .instrucoes
                        .get(v)
                        .is_some_and(|e| *e != EfeitoTokens::default())
                {
                    return Err(format!("v{}: contrato incompatível com local escalar", v.0));
                }
                produzidas.insert(*v);
            }
        }
        let mut escapa = false;
        operandos_do_terminador(&b.terminator, &mut |op| {
            escapa |= local(op).is_some();
        });
        if escapa {
            return Err(format!(
                "b{}: endereço de local escalar escapa no terminador",
                b.id.0
            ));
        }
    }
    if let Some(v) = plano.enderecos_cedidos.keys().filter(|v| !cedidos_vistos.contains(v)).min_by_key(|v| v.0) {
        return Err(format!("v{}: endereços cedidos fora de uma chamada direta", v.0));
    }
    let cfg = Cfg::novo(f);
    let universo: HashSet<_> = locais.keys().copied().collect();
    // Análise must: a interseção preserva apenas inicializações presentes
    // em todos os predecessores. O topo nos laços decresce até o ponto fixo.
    let mut entradas = vec![universo.clone(); f.blocks.len()];
    let mut saidas = entradas.clone();
    let transferir = |bi: usize, mut estado: HashSet<ValueId>| {
        for (v, inst, _) in &f.blocks[bi].instructions {
            match inst {
                Instruction::Alloca(_) if locais.contains_key(v) => {
                    estado.remove(v);
                }
                Instruction::Store { ptr, .. } => {
                    if let Some(p) = local(ptr) {
                        estado.insert(p);
                    }
                }
                _ => {}
            }
        }
        estado
    };
    loop {
        let mut mudou = false;
        for &bi in &cfg.rpo {
            let mut entrada = if bi == 0 {
                recebidos.clone()
            } else {
                universo.clone()
            };
            if bi != 0 {
                for &p in &cfg.predecessores[bi] {
                    if cfg.alcancavel(p) {
                        entrada.retain(|v| saidas[p].contains(v));
                    }
                }
            }
            let saida = transferir(bi, entrada.clone());
            mudou |= entradas[bi] != entrada || saidas[bi] != saida;
            entradas[bi] = entrada;
            saidas[bi] = saida;
        }
        if !mudou {
            break;
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        let mut estado = if cfg.alcancavel(bi) {
            entradas[bi].clone()
        } else {
            HashSet::new()
        };
        for (v, inst, _) in &b.instructions {
            match inst {
                Instruction::Alloca(_) if locais.contains_key(v) => {
                    estado.remove(v);
                }
                Instruction::Store { ptr, .. } => {
                    if let Some(p) = local(ptr) {
                        estado.insert(p);
                    }
                }
                Instruction::Load { ptr, .. } => {
                    if let Some(p) = local(ptr)
                        && !estado.contains(&p)
                    {
                        return Err(format!(
                            "v{}: local escalar não inicializado em todos os caminhos",
                            v.0
                        ));
                    }
                }
                // A função direta pode ler o local cedido: como uma leitura.
                Instruction::CallStatic { args, .. } if plano.enderecos_cedidos.contains_key(v) => {
                    for (i, _) in &plano.enderecos_cedidos[v] {
                        if let Some(p) = args.get(*i).and_then(local)
                            && !estado.contains(&p)
                        {
                            return Err(format!(
                                "v{}: local escalar cedido não inicializado em todos os caminhos",
                                v.0
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok((locais, produzidas))
}

pub(super) fn verificar_valores(
    f: &Function,
    locais: &HashMap<ValueId, Type>,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<(), String> {
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::Store {
            ptr: Operand::Val(p),
            val: Operand::Val(origem),
        } = inst
            && locais.contains_key(p)
            && classes.get(origem) != Some(&Ownership::Trivial)
        {
            return Err(format!(
                "v{}: gravação de local escalar exige origem Trivial",
                v.0
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::optimize::arc::{
        OrigemOwner, PlanoEscopos, produzir_contratos_arc, produzir_e_verificar_tokens,
    };

    fn diamante() -> Function {
        Function {
            symbol: "local".into(),
            name: "local".into(),
            depuracao: None,
            params: vec![(ValueId(0), "cond".into(), Type::I1)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), Instruction::Alloca(Type::I64), Type::Ptr)],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            ValueId(4),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(1)),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(5),
                            Instruction::IntToDouble(Operand::Val(ValueId(4))),
                            Type::F64,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(3),
                        Instruction::Store {
                            ptr: Operand::Val(ValueId(1)),
                            val: Operand::Constant(Constant::Int(9)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Store {
                            ptr: Operand::Val(ValueId(1)),
                            val: Operand::Constant(Constant::Int(7)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
            ],
        }
    }

    #[test]
    fn local_escalar_exige_inicializacao_em_todas_as_arestas() {
        let mut f = diamante();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plano = PlanoTokens::default();
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 6);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        f.blocks[2].instructions.clear();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let antes = classes.clone();
        let mut plano = PlanoTokens::default();
        assert!(
            produzir_contratos_arc(&f, &mut classes, &mut plano)
                .unwrap_err()
                .contains("não inicializado")
        );
        assert_eq!(classes, antes);
        assert!(plano.instrucoes.is_empty());
    }

    #[test]
    fn local_no_laco_exige_gravacao_antes_da_primeira_volta() {
        let mut f = Function {
            symbol: "laco".into(),
            name: "laco".into(),
            depuracao: None,
            params: vec![(ValueId(0), "cond".into(), Type::I1)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), Instruction::Alloca(Type::I64), Type::Ptr)],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![
                        (
                            ValueId(2),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(1)),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(3),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(1)),
                                val: Operand::Constant(Constant::Int(9)),
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ],
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plano = PlanoTokens::default();
        assert!(
            produzir_contratos_arc(&f, &mut classes, &mut plano)
                .unwrap_err()
                .contains("não inicializado")
        );
        assert_eq!(classes.len(), 1);
        assert!(plano.instrucoes.is_empty());
        f.blocks[0].instructions.push((
            ValueId(4),
            Instruction::Store {
                ptr: Operand::Val(ValueId(1)),
                val: Operand::Constant(Constant::Int(7)),
            },
            Type::Void,
        ));
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 5);
    }

    #[test]
    fn locais_tipados_alimentam_produtores_sem_inferir_por_largura() {
        for (ty, valor) in [
            (Type::I64, Instruction::Const(Constant::Int(7))),
            (Type::F64, Instruction::Const(Constant::Double(-0.0))),
            (Type::I1, Instruction::Const(Constant::Bool(true))),
            (
                Type::I8,
                Instruction::ZExt {
                    op: Operand::Constant(Constant::Bool(true)),
                    from: Type::I1,
                    to: Type::I8,
                },
            ),
        ] {
            let f = Function {
                symbol: "tipo_local".into(),
                name: "tipo_local".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(0), Instruction::Alloca(ty), Type::Ptr),
                        (ValueId(1), valor, ty),
                        (
                            ValueId(2),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(0)),
                                val: Operand::Val(ValueId(1)),
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(3),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(0)),
                                ty,
                            },
                            ty,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::new();
            let mut plano = PlanoTokens::default();
            produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default(),
            )
            .unwrap();
            assert_eq!(classes.len(), 4);
            assert!(classes.values().all(|c| *c == Ownership::Trivial));
            let mut modulo = Module::default();
            modulo.memoria_arc = true;
            modulo.functions.push(f);
            let ir = crate::llvm::LlvmEmitter::new(&modulo).emit_all();
            assert!(ir.contains(&format!("load {}, ptr %v0", ty.llvm_ir())));
        }
    }

    #[test]
    fn modulo_liga_leitura_escalar_a_boxing_owned_sem_mudar_tracing() {
        use crate::optimize::arc::{PlanoFuncaoDart, preparar_arc_modulo_dart};
        for arc in [false, true] {
            let mut f = diamante();
            f.blocks[1].instructions.push((
                ValueId(6),
                Instruction::Box {
                    op: Operand::Val(ValueId(4)),
                    from: Type::I64,
                },
                Type::Ref,
            ));
            let mut modulo = Module::default();
            modulo.memoria_arc = arc;
            modulo.functions.push(f);
            let mut planos = HashMap::from([("local".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{modulo:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut modulo, &mut planos).unwrap(),
                if arc { (0, 1) } else { (0, 0) }
            );
            if arc {
                assert_eq!(planos["local"].classes[&ValueId(4)], Ownership::Trivial);
                assert_eq!(planos["local"].classes[&ValueId(6)], Ownership::Owned);
                assert!(matches!(
                    modulo.functions[0].blocks[1].instructions.last().unwrap().1,
                    Instruction::ArcDrop { .. }
                ));
                assert_eq!(
                    preparar_arc_modulo_dart(&mut modulo, &mut planos).unwrap(),
                    (0, 0)
                );
                modulo.functions[0].blocks[2].instructions.clear();
                let antes = format!("{modulo:?}/{planos:?}");
                assert!(preparar_arc_modulo_dart(&mut modulo, &mut planos).is_err());
                assert_eq!(format!("{modulo:?}/{planos:?}"), antes);
            } else {
                assert_eq!(format!("{modulo:?}/{planos:?}"), antes);
            }
        }
    }

    #[test]
    fn verificador_recusa_plano_local_obsoleto_apos_mudar_o_corpo() {
        use crate::optimize::arc::verificar_tokens;
        let base = diamante();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plano = PlanoTokens::default();
        produzir_contratos_arc(&base, &mut classes, &mut plano).unwrap();
        verificar_tokens(&base, &classes, &TabelasDaFuncao::default(), &plano).unwrap();
        for caso in 0..5 {
            let mut f = base.clone();
            let mut classes = classes.clone();
            let mut plano = plano.clone();
            match caso {
                0 => {
                    f.blocks[2].instructions.clear();
                    classes.remove(&ValueId(3));
                    plano.instrucoes.remove(&ValueId(3));
                }
                1 => {
                    f.blocks[1].instructions.push((
                        ValueId(6),
                        Instruction::CallRuntime {
                            name: "extern_sem_ownership".into(),
                            args: vec![(Operand::Val(ValueId(1)), Type::Ptr)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                    classes.insert(ValueId(6), Ownership::Trivial);
                    plano.instrucoes.insert(ValueId(6), EfeitoTokens::default());
                }
                2 => {
                    f.blocks[1].instructions[0].1 = Instruction::Load {
                        ptr: Operand::Val(ValueId(1)),
                        ty: Type::F64,
                    }
                }
                3 => {
                    f.blocks[2].instructions[0].1 = Instruction::Store {
                        ptr: Operand::Val(ValueId(1)),
                        val: Operand::Constant(Constant::Null),
                    }
                }
                4 => {
                    plano.instrucoes.get_mut(&ValueId(4)).unwrap().pode_falhar = true;
                    plano.pendencias.insert(ValueId(4), BlockId(2));
                }
                _ => unreachable!(),
            }
            let antes = format!("{classes:?}/{plano:?}");
            assert!(
                verificar_tokens(&f, &classes, &TabelasDaFuncao::default(), &plano).is_err(),
                "caso {caso}"
            );
            assert_eq!(format!("{classes:?}/{plano:?}"), antes);
        }
    }

    #[test]
    fn byte_literal_tem_faixa_explicita_no_slot() {
        for n in [0, 1, 2, 255, -1, 256] {
            let f = Function {
                symbol: "byte".into(),
                name: "byte".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(0), Instruction::Alloca(Type::I8), Type::Ptr),
                        (
                            ValueId(1),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(0)),
                                val: Operand::Constant(Constant::Int(n)),
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(2),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(0)),
                                ty: Type::I8,
                            },
                            Type::I8,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::new();
            let mut plano = PlanoTokens::default();
            let resultado = produzir_contratos_arc(&f, &mut classes, &mut plano);
            assert_eq!(resultado.is_ok(), (0..=255).contains(&n));
            if resultado.is_err() {
                assert!(classes.is_empty());
                assert!(plano.instrucoes.is_empty());
            }
        }
    }

    #[test]
    fn local_escalar_recusa_escape_e_gravacao_gerenciada_atomicamente() {
        let base = diamante();
        for caso in 0..6 {
            let mut f = base.clone();
            match caso {
                0 => f.blocks[1].instructions.push((
                    ValueId(6),
                    Instruction::CallRuntime {
                        name: "sem_contrato".into(),
                        args: vec![(Operand::Val(ValueId(1)), Type::Ptr)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                1 => f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(1)))),
                2 => {
                    f.blocks[1].instructions[0].1 = Instruction::Load {
                        ptr: Operand::Val(ValueId(1)),
                        ty: Type::Ref,
                    }
                }
                3 => {
                    f.blocks[2].instructions[0].1 = Instruction::Store {
                        ptr: Operand::Val(ValueId(1)),
                        val: Operand::Constant(Constant::Null),
                    }
                }
                4 => f.blocks[0].instructions[0].2 = Type::I64,
                5 => {
                    let a = f.blocks[0].instructions.remove(0);
                    f.blocks[3].instructions.insert(0, a);
                }
                _ => unreachable!(),
            }
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            let antes = classes.clone();
            let mut plano = PlanoTokens::default();
            assert!(
                produzir_contratos_arc(&f, &mut classes, &mut plano).is_err(),
                "caso {caso}"
            );
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
        }
        let mut f = base;
        f.params.push((ValueId(6), "valor".into(), Type::I64));
        f.blocks[2].instructions[0].1 = Instruction::Store {
            ptr: Operand::Val(ValueId(1)),
            val: Operand::Val(ValueId(6)),
        };
        for classe in [
            None,
            Some(Ownership::Owned),
            Some(Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            }),
        ] {
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            if let Some(c) = classe {
                classes.insert(ValueId(6), c);
            }
            let antes = classes.clone();
            let mut plano = PlanoTokens::default();
            assert!(
                produzir_contratos_arc(&f, &mut classes, &mut plano)
                    .unwrap_err()
                    .contains("origem Trivial")
            );
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
        }
    }

    #[test]
    fn captura_por_endereco_liga_o_local_do_chamador_ao_parametro_da_direta() {
        use crate::optimize::arc::{PlanoFuncaoDart, preparar_arc_modulo_dart};
        let val = |n| Operand::Val(ValueId(n));
        for caso in 0..4 {
            let mut m = Module::new();
            m.memoria_arc = true;
            if caso != 3 {
                m.funcoes_locais_diretas_arc.insert("direta".into());
            }
            let local = if caso == 1 { Type::F64 } else { Type::I64 };
            let mut fora = vec![
                (ValueId(0), Instruction::Alloca(local), Type::Ptr),
                (ValueId(1), Instruction::Store { ptr: val(0), val: Operand::Constant(if caso == 1 {
                    Constant::Double(1.0) } else { Constant::Int(1) }) }, Type::Void),
                (ValueId(2), Instruction::CallStatic { symbol: "direta".into(), args: vec![val(0)], ret_ty: Type::Void }, Type::Void),
            ];
            if caso == 2 {
                // Cedido antes da primeira gravação.
                fora.swap(1, 2);
            }
            m.functions.push(Function {
                symbol: "fora".into(), name: "fora".into(), depuracao: None, params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: fora, terminator: Terminator::Return(None) }],
            });
            m.functions.push(Function {
                symbol: "direta".into(), name: "direta".into(), depuracao: None,
                params: vec![(ValueId(0), "x".into(), Type::Ptr)], return_ty: Type::Void,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::Load { ptr: val(0), ty: Type::I64 }, Type::I64),
                    (ValueId(2), Instruction::Add(val(1), Operand::Constant(Constant::Int(1))), Type::I64),
                    (ValueId(3), Instruction::Store { ptr: val(0), val: val(2) }, Type::Void),
                ], terminator: Terminator::Return(None) }],
            });
            let mut planos: HashMap<String, PlanoFuncaoDart> =
                ["fora", "direta"].map(|s| (s.to_string(), PlanoFuncaoDart::default())).into();
            let antes = format!("{m:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso != 0 {
                let erro = resultado.unwrap_err();
                assert!(match caso {
                    1 => erro.contains("lido como"),
                    2 => erro.contains("cedido não inicializado"),
                    _ => erro.contains("escapa"),
                }, "{caso}: {erro}");
                assert_eq!(format!("{m:?}"), antes);
                continue;
            }
            resultado.unwrap();
            assert_eq!(planos["direta"].tokens.enderecos_recebidos, HashMap::from([(ValueId(0), Type::I64)]));
            assert_eq!(planos["direta"].classes[&ValueId(0)], Ownership::Trivial);
            assert_eq!(planos["fora"].tokens.enderecos_cedidos, HashMap::from([(ValueId(2), vec![(0, Type::I64)])]));
        }
    }
}
