//! Inclusões de arestas fortes produzidas por stores parametrizados (§28.3).
//! Não são cobertura de vida/publicação: campos opacos e caminhos implícitos
//! continuam exigindo ARC geral mesmo quando estas inclusões são conhecidas.

use super::resumos::*;
use crate::hir::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CaminhoParametro {
    pub parametro: usize,
    pub campos: Vec<usize>,
}

impl CaminhoParametro {
    fn raiz(parametro: usize) -> Self {
        Self {
            parametro,
            campos: Vec::new(),
        }
    }

    fn continuar(&self, campos: &[usize], limite: usize) -> Option<Self> {
        if self.campos.len().checked_add(campos.len())? > limite {
            return None;
        }
        let mut caminho = self.clone();
        caminho.campos.extend_from_slice(campos);
        Some(caminho)
    }
}

// Bottom sem evidência de resultado normal fica interno à solução da SCC.
// Null é separado dos caminhos: nunca ganha identidade de objeto abstrato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RetornoCaminhos {
    pub caminhos: Option<BTreeSet<CaminhoParametro>>,
    pub nulo: bool,
}

impl RetornoCaminhos {
    pub(super) fn vazio() -> Self {
        Self {
            caminhos: Some(BTreeSet::new()),
            nulo: false,
        }
    }

    pub(super) fn desconhecido() -> Self {
        Self {
            caminhos: None,
            nulo: true,
        }
    }

    pub(super) fn unir(&mut self, outro: &Self, limite: usize) -> bool {
        let antes = self.clone();
        unir_aliases(&mut self.caminhos, outro.caminhos.clone(), limite);
        self.nulo |= outro.nulo || self.caminhos.is_none();
        *self != antes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct EscritaParametro {
    pub objeto: CaminhoParametro,
    pub indice: usize,
    pub valor: CaminhoParametro,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InclusoesEscritas {
    Conhecidas(BTreeSet<EscritaParametro>),
    Desconhecidas,
}

impl InclusoesEscritas {
    pub(super) fn vazio() -> Self {
        Self::Conhecidas(BTreeSet::new())
    }

    // Topo é absorvente; orçamento vale para o resumo inteiro, não por store.
    pub(super) fn unir(&mut self, outro: &Self, limite: usize) -> bool {
        let antes = self.clone();
        match (&mut *self, outro) {
            (Self::Conhecidas(a), Self::Conhecidas(b)) => {
                a.extend(b.iter().cloned());
                if a.len() > limite {
                    *self = Self::Desconhecidas;
                }
            }
            (_, Self::Desconhecidas) => *self = Self::Desconhecidas,
            _ => {}
        }
        *self != antes
    }
}

// Caminhos representam leituras no superestado fraco do chamador: sua
// instanciação deve incluir todos os stores, sem apagar valores anteriores.
// O comprimento e a quantidade de aliases são limitados; crescer além do
// orçamento produz topo, inclusive em Phis ou chamadas recursivas.
type AliasesCaminhos = Option<BTreeSet<CaminhoParametro>>;

// Apenas inclusões explícitas: mapa vazio não certifica ausência de
// publicação por SDK, callback, finalizador ou emissão implícita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InclusoesGlobais {
    Conhecidas(BTreeMap<String, AliasesCaminhos>),
    Desconhecidas,
}

impl InclusoesGlobais {
    pub(super) fn vazio() -> Self {
        Self::Conhecidas(BTreeMap::new())
    }

    pub(super) fn unir(&mut self, outro: &Self, limite: usize) -> bool {
        let antes = self.clone();
        match (&mut *self, outro) {
            (Self::Conhecidas(a), Self::Conhecidas(b)) => {
                for (simbolo, caminhos) in b {
                    unir_aliases(
                        a.entry(simbolo.clone())
                            .or_insert_with(|| Some(BTreeSet::new())),
                        caminhos.clone(),
                        limite,
                    );
                }
                // Nomes também consomem orçamento, inclusive publicação de null.
                let tamanho = a.values().try_fold(a.len(), |n, c| {
                    n.checked_add(c.as_ref().map_or(0, BTreeSet::len))
                });
                if tamanho.is_none_or(|n| n > limite) {
                    *self = Self::Desconhecidas;
                }
            }
            (_, Self::Desconhecidas) => *self = Self::Desconhecidas,
            _ => {}
        }
        *self != antes
    }
}

fn unir_aliases(destino: &mut AliasesCaminhos, origem: AliasesCaminhos, limite: usize) -> bool {
    let antes = destino.clone();
    match (&mut *destino, origem) {
        (Some(a), Some(b)) => {
            a.extend(b);
            if a.len() > limite {
                *destino = None;
            }
        }
        (_, None) => *destino = None,
        _ => {}
    }
    *destino != antes
}

fn prolongar(aliases: AliasesCaminhos, campos: &[usize], limite: usize) -> AliasesCaminhos {
    aliases?
        .into_iter()
        .map(|c| c.continuar(campos, limite))
        .collect()
}

#[cfg(test)]
fn extrair<'a>(
    f: &Function,
    limite: usize,
    chamada: impl Fn(&str) -> Option<(&'a Function, &'a RetornoHeapArc, &'a InclusoesEscritas)>,
) -> Result<InclusoesEscritas, String> {
    Ok(extrair_com_resultado(f, limite, false, |s| {
        chamada(s).map(|(callee, retorno, escritas)| {
            let caminhos = match retorno {
                RetornoHeapArc::Aliases { parametros, nulo } => RetornoCaminhos {
                    caminhos: Some(
                        parametros
                            .iter()
                            .copied()
                            .map(CaminhoParametro::raiz)
                            .collect(),
                    ),
                    nulo: *nulo,
                },
                RetornoHeapArc::Desconhecido => RetornoCaminhos::desconhecido(),
            };
            (callee, caminhos, escritas)
        })
    })?
    .1)
}

#[cfg(test)]
fn extrair_com_resultado<'a>(
    f: &Function,
    limite: usize,
    interno: bool,
    chamada: impl Fn(&str) -> Option<(&'a Function, RetornoCaminhos, &'a InclusoesEscritas)>,
) -> Result<(RetornoCaminhos, InclusoesEscritas), String> {
    let (r, e, _) = extrair_com_efeitos(f, limite, interno, |s| {
        chamada(s).map(|(f, r, e)| (f, r, e, InclusoesGlobais::vazio()))
    })?;
    Ok((r, e))
}

pub(super) fn extrair_com_efeitos<'a>(
    f: &Function,
    limite: usize,
    interno: bool,
    chamada: impl Fn(
        &str,
    ) -> Option<(
        &'a Function,
        RetornoCaminhos,
        &'a InclusoesEscritas,
        InclusoesGlobais,
    )>,
) -> Result<(RetornoCaminhos, InclusoesEscritas, InclusoesGlobais), String> {
    super::super::ssa::verificar(f)?;
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
    let mut valores: Vec<_> = tipos.keys().copied().collect();
    valores.sort_by_key(|v| v.0);
    let ids: HashMap<_, _> = valores.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let mut aliases = vec![Some(BTreeSet::new()); ids.len()];
    let mut nulos = vec![false; ids.len()];
    let mut escritas_opacas = false;
    for (p, (v, _, t)) in f.params.iter().enumerate() {
        aliases[ids[v]] = if *t == Type::Ref && limite > 0 {
            Some(BTreeSet::from([CaminhoParametro::raiz(p)]))
        } else {
            None
        };
    }
    let operando = |o: &Operand, aliases: &[AliasesCaminhos]| match o {
        Operand::Val(v) => aliases[ids[v]].clone(),
        Operand::Constant(Constant::Null) => Some(BTreeSet::new()),
        _ => None,
    };
    let nulo = |o: &Operand, nulos: &[bool]| match o {
        Operand::Val(v) => nulos[ids[v]],
        Operand::Constant(Constant::Null) => true,
        _ => false,
    };
    // Bottom fica interno até a convergência. Stores só são extraídos após
    // estabilizar cópias, caminhos e aliases de resultados normais locais.
    let instrucoes: Vec<_> = f.blocks.iter().flat_map(|b| &b.instructions).collect();
    let mut dependentes = vec![Vec::new(); ids.len()];
    for (indice, (_, i, _)) in instrucoes.iter().enumerate() {
        crate::otimizar::operandos::operandos(i, &mut |o| {
            if let Operand::Val(v) = o {
                dependentes[ids[v]].push(indice);
            }
        });
    }
    let mut fila: VecDeque<_> = (0..instrucoes.len()).collect();
    let mut enfileirados = vec![true; instrucoes.len()];
    while let Some(indice) = fila.pop_front() {
        enfileirados[indice] = false;
        let (v, i, t) = instrucoes[indice];
        let mut novo_nulo = false;
        let novo = match i {
            Instruction::Const(Constant::Null) if *t == Type::Ref => {
                novo_nulo = true;
                Some(BTreeSet::new())
            }
            Instruction::ArcCopy { value } | Instruction::ArcMove { value }
                if *t == Type::Ref && argumento_exato(value, Type::Ref, &tipos) =>
            {
                novo_nulo = nulo(value, &nulos);
                operando(value, &aliases)
            }
            Instruction::Bitcast { op, to: Type::Ref }
                if *t == Type::Ref && argumento_exato(op, Type::Ref, &tipos) =>
            {
                novo_nulo = nulo(op, &nulos);
                operando(op, &aliases)
            }
            Instruction::Phi {
                incoming,
                ty: Type::Ref,
            } if *t == Type::Ref
                && incoming
                    .iter()
                    .all(|(_, o)| argumento_exato(o, Type::Ref, &tipos)) =>
            {
                let mut a = Some(BTreeSet::new());
                for (_, o) in incoming {
                    novo_nulo |= nulo(o, &nulos);
                    unir_aliases(&mut a, operando(o, &aliases), limite);
                }
                a
            }
            Instruction::GetField { object, index }
                if *t == Type::Ref && argumento_exato(object, Type::Ref, &tipos) =>
            {
                let a = prolongar(operando(object, &aliases), &[*index], limite);
                // Campo pode ser null, mas bottom sem receiver não é uma
                // evidência de execução normal em recursão sem base.
                novo_nulo = a.as_ref().is_none_or(|c| !c.is_empty()) || nulo(object, &nulos);
                a
            }
            Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } => {
                let callee = chamada(symbol);
                if let Some((callee, retorno, _, _)) = callee
                    && *t == *ret_ty
                    && *ret_ty == callee.return_ty
                    && args.len() == callee.params.len()
                    && args
                        .iter()
                        .zip(&callee.params)
                        .all(|(o, (_, _, ty))| argumento_exato(o, *ty, &tipos))
                {
                    if *t == Type::Ref {
                        novo_nulo = retorno.nulo;
                        if let Some(caminhos) = &retorno.caminhos {
                            let mut a = Some(BTreeSet::new());
                            for caminho in caminhos {
                                let arg = args
                                    .get(caminho.parametro)
                                    .ok_or("escritas ARC: parâmetro de retorno inválido")?;
                                novo_nulo |= nulo(arg, &nulos);
                                unir_aliases(
                                    &mut a,
                                    prolongar(operando(arg, &aliases), &caminho.campos, limite),
                                    limite,
                                );
                            }
                            a
                        } else {
                            None
                        }
                    } else {
                        Some(BTreeSet::new())
                    }
                } else {
                    escritas_opacas = true;
                    None
                }
            }
            Instruction::SetField { object, value, .. }
                if *t == Type::Void
                    && argumento_exato(object, Type::Ref, &tipos)
                    && argumento_exato(value, Type::Ref, &tipos) =>
            {
                Some(BTreeSet::new())
            }
            Instruction::Const(c) if argumento_exato(&Operand::Constant(c.clone()), *t, &tipos) => {
                None
            }
            i if super::hir::escalar_puro(i, *t, &tipos) => None,
            _ => {
                escritas_opacas = true;
                None
            }
        };
        novo_nulo |= *t == Type::Ref && novo.is_none();
        let mudou_alias = unir_aliases(&mut aliases[ids[v]], novo, limite);
        let mudou_nulo = novo_nulo && !nulos[ids[v]];
        nulos[ids[v]] |= novo_nulo;
        if mudou_alias || mudou_nulo {
            for &dependente in &dependentes[ids[v]] {
                if !enfileirados[dependente] {
                    enfileirados[dependente] = true;
                    fila.push_back(dependente);
                }
            }
        }
    }
    let mut retorno = if f.return_ty == Type::Ref {
        RetornoCaminhos::vazio()
    } else {
        RetornoCaminhos::desconhecido()
    };
    let mut normal = false;
    for b in &f.blocks {
        if let Terminator::Return(Some(o)) = &b.terminator {
            normal = true;
            retorno.unir(
                &RetornoCaminhos {
                    caminhos: if f.return_ty == Type::Ref && argumento_exato(o, Type::Ref, &tipos) {
                        operando(o, &aliases)
                    } else {
                        None
                    },
                    nulo: nulo(o, &nulos),
                },
                limite,
            );
        }
    }
    if !interno && (!normal || retorno == RetornoCaminhos::vazio()) {
        retorno = RetornoCaminhos::desconhecido();
    }
    let mut globais = InclusoesGlobais::vazio();
    for (_, i, t) in &instrucoes {
        let publicacao = match i {
            Instruction::StoreGlobal {
                simbolo,
                val,
                ty: Type::Ref,
                ..
            } if *t == Type::Void && argumento_exato(val, Type::Ref, &tipos) => {
                Some((simbolo, val))
            }
            Instruction::ArcStoreStrong {
                slot: SlotForte::Global { simbolo },
                value,
                ..
            } if *t == Type::Void && argumento_exato(value, Type::Ref, &tipos) => {
                Some((simbolo, value))
            }
            _ => None,
        };
        if let Some((simbolo, valor)) = publicacao {
            globais.unir(
                &InclusoesGlobais::Conhecidas(BTreeMap::from([(
                    simbolo.clone(),
                    operando(valor, &aliases),
                )])),
                limite,
            );
        }
        if let Instruction::CallStatic {
            symbol,
            args,
            ret_ty,
        } = i
            && let Some((callee, _, _, inclusoes)) = chamada(symbol)
            && *t == *ret_ty
            && *ret_ty == callee.return_ty
            && args.len() == callee.params.len()
            && args
                .iter()
                .zip(&callee.params)
                .all(|(o, (_, _, ty))| argumento_exato(o, *ty, &tipos))
        {
            match inclusoes {
                InclusoesGlobais::Desconhecidas => {
                    globais.unir(&InclusoesGlobais::Desconhecidas, limite);
                }
                InclusoesGlobais::Conhecidas(inclusoes) => {
                    for (simbolo, caminhos) in inclusoes {
                        let mut valores = Some(BTreeSet::new());
                        if let Some(caminhos) = caminhos {
                            for caminho in caminhos {
                                let arg = args
                                    .get(caminho.parametro)
                                    .ok_or("publicacoes ARC: parâmetro inválido")?;
                                unir_aliases(
                                    &mut valores,
                                    prolongar(operando(arg, &aliases), &caminho.campos, limite),
                                    limite,
                                );
                            }
                        } else {
                            valores = None;
                        }
                        globais.unir(
                            &InclusoesGlobais::Conhecidas(BTreeMap::from([(simbolo, valores)])),
                            limite,
                        );
                    }
                }
            }
        }
    }
    if escritas_opacas {
        return Ok((retorno, InclusoesEscritas::Desconhecidas, globais));
    }
    let mut resultado = InclusoesEscritas::vazio();
    let mut incluir = |origens: AliasesCaminhos, indice, destinos: AliasesCaminhos| -> bool {
        let (Some(origens), Some(destinos)) = (origens, destinos) else {
            return false;
        };
        for origem in origens {
            for destino in &destinos {
                resultado.unir(
                    &InclusoesEscritas::Conhecidas(BTreeSet::from([EscritaParametro {
                        objeto: origem.clone(),
                        indice,
                        valor: destino.clone(),
                    }])),
                    limite,
                );
            }
        }
        true
    };
    for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        match i {
            Instruction::SetField {
                object,
                index,
                value,
            } => {
                if !incluir(
                    operando(object, &aliases),
                    *index,
                    operando(value, &aliases),
                ) {
                    return Ok((retorno, InclusoesEscritas::Desconhecidas, globais));
                }
            }
            Instruction::CallStatic { symbol, args, .. } => {
                let (_, _, escritas, _) = chamada(symbol).unwrap();
                let InclusoesEscritas::Conhecidas(escritas) = escritas else {
                    return Ok((retorno, InclusoesEscritas::Desconhecidas, globais));
                };
                for escrita in escritas {
                    let object = args
                        .get(escrita.objeto.parametro)
                        .ok_or("escritas ARC: receiver inválido")?;
                    let value = args
                        .get(escrita.valor.parametro)
                        .ok_or("escritas ARC: valor inválido")?;
                    if !incluir(
                        prolongar(operando(object, &aliases), &escrita.objeto.campos, limite),
                        escrita.indice,
                        prolongar(operando(value, &aliases), &escrita.valor.campos, limite),
                    ) {
                        return Ok((retorno, InclusoesEscritas::Desconhecidas, globais));
                    }
                }
            }
            _ => {}
        }
    }
    Ok((retorno, resultado, globais))
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashSet;

    fn getter(nome: &str, indice: usize) -> Function {
        Function {
            symbol: nome.into(),
            name: nome.into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::GetField {
                        object: Operand::Val(ValueId(0)),
                        index: indice,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        }
    }

    #[test]
    fn publicacoes_globais_compostas_preservam_caminhos_e_null() {
        let mut g = getter("get", 2);
        g.blocks[0].instructions.push((
            ValueId(2),
            Instruction::ArcStoreStrong {
                slot: SlotForte::Global {
                    simbolo: "g".into(),
                },
                value: Operand::Val(ValueId(1)),
                modo: ModoStoreForte::Copy,
            },
            Type::Void,
        ));
        let mut wrapper = getter("wrapper", 9);
        wrapper.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "get".into(),
                args: vec![Operand::Val(ValueId(1))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        wrapper.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let mut m = Module::new();
        m.memoria_arc = true;
        m.globais.push((0, Type::Ref, "g".into()));
        m.functions = vec![wrapper, g];
        for ordem in [false, true] {
            if ordem {
                m.functions.reverse();
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["wrapper"]), 8).unwrap();
            assert_eq!(
                r["wrapper"].1.inclusoes_globais,
                InclusoesGlobais::Conhecidas(BTreeMap::from([(
                    "g".into(),
                    Some(BTreeSet::from([CaminhoParametro {
                        parametro: 0,
                        campos: vec![9, 2]
                    }]))
                )]))
            );
            assert_eq!(
                r["wrapper"].1.publicacoes(),
                PublicacoesHeapArc::Desconhecidas
            );
        }
        let g = m.functions.iter_mut().find(|f| f.symbol == "get").unwrap();
        if let Instruction::ArcStoreStrong { value, .. } = &mut g.blocks[0].instructions[1].1 {
            *value = Operand::Constant(Constant::Null);
        }
        let r = super::super::chamadas::resolver(&m, &HashSet::from(["wrapper"]), 8).unwrap();
        assert_eq!(
            r["wrapper"].1.inclusoes_globais,
            InclusoesGlobais::Conhecidas(BTreeMap::from([("g".into(), Some(BTreeSet::new()))]))
        );
    }

    #[test]
    fn scc_publica_ambos_os_argumentos_e_orcamento_nao_esconde_abi_invalida() {
        for variante in 0..3 {
            let mut f = ligar("pub", false);
            f.blocks[0].instructions[0].1 = match variante {
                0 => Instruction::StoreGlobal {
                    simbolo: "g".into(),
                    val: Operand::Val(ValueId(0)),
                    ty: Type::Ref,
                    raiz: Some(0),
                },
                _ => Instruction::ArcStoreStrong {
                    slot: SlotForte::Global {
                        simbolo: "g".into(),
                    },
                    value: Operand::Val(ValueId(0)),
                    modo: if variante == 1 {
                        ModoStoreForte::Copy
                    } else {
                        ModoStoreForte::Move
                    },
                },
            };
            f.blocks[0].instructions.push((
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "pub".into(),
                    args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ));
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions.push(f);
            m.globais.push((0, Type::Ref, "g".into()));
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["pub"]), 3).unwrap();
            assert_eq!(
                r["pub"].1.inclusoes_globais,
                InclusoesGlobais::Conhecidas(BTreeMap::from([(
                    "g".into(),
                    Some(BTreeSet::from([
                        CaminhoParametro::raiz(0),
                        CaminhoParametro::raiz(1)
                    ]))
                )]))
            );
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["pub"]), 2).unwrap();
            assert_eq!(
                r["pub"].1.inclusoes_globais,
                InclusoesGlobais::Desconhecidas
            );
            m.globais.push((0, Type::Ref, "g".into()));
            assert!(super::super::chamadas::resolver(&m, &HashSet::from(["pub"]), 0).is_err());
            m.globais.pop();
            m.globais[0].1 = Type::I64;
            assert!(super::super::chamadas::resolver(&m, &HashSet::from(["pub"]), 8).is_err());
            m.globais.clear();
            assert!(super::super::chamadas::resolver(&m, &HashSet::from(["pub"]), 8).is_err());
        }
    }

    #[test]
    fn alias_do_resultado_nao_apaga_efeito_externo_e_budget_zero_conserva_topo() {
        let mut f = getter("externo", 0);
        f.blocks[0].instructions = vec![(
            ValueId(1),
            Instruction::CallStatic {
                symbol: "desconhecido".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Void,
            },
            Type::Void,
        )];
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        let (retorno, escritas) = extrair_com_resultado(&f, 8, false, |_| None).unwrap();
        assert_eq!(
            retorno,
            RetornoCaminhos {
                caminhos: Some(BTreeSet::from([CaminhoParametro::raiz(0)])),
                nulo: false,
            }
        );
        assert_eq!(escritas, InclusoesEscritas::Desconhecidas);
        let (retorno, escritas) = extrair_com_resultado(&f, 0, false, |_| None).unwrap();
        assert_eq!(retorno, RetornoCaminhos::desconhecido());
        assert_eq!(escritas, InclusoesEscritas::Desconhecidas);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        let (retorno, escritas) = extrair_com_resultado(&f, 0, false, |_| None).unwrap();
        assert_eq!(
            retorno,
            RetornoCaminhos {
                caminhos: Some(BTreeSet::new()),
                nulo: true
            }
        );
        assert_eq!(escritas, InclusoesEscritas::Desconhecidas);
    }

    #[test]
    fn retorno_de_campo_composto_alimenta_escritas_sem_promover_cobertura() {
        let g = getter("get", 2);
        let mut wrapper = getter("wrapper", 9);
        wrapper.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "get".into(),
                args: vec![Operand::Val(ValueId(1))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        wrapper.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let mut writer = ligar("writer", false);
        writer.blocks[0].instructions.insert(
            0,
            (
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "wrapper".into(),
                    args: vec![Operand::Val(ValueId(0))],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
        );
        if let Instruction::SetField { object, .. } = &mut writer.blocks[0].instructions[1].1 {
            *object = Operand::Val(ValueId(3));
        }
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![writer, wrapper, g];
        for ordem in [false, true] {
            if ordem {
                m.functions.reverse();
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["writer"]), 8).unwrap();
            assert_eq!(
                r["wrapper"].1.retorno_caminhos,
                RetornoCaminhos {
                    caminhos: Some(BTreeSet::from([CaminhoParametro {
                        parametro: 0,
                        campos: vec![9, 2]
                    }])),
                    nulo: true,
                }
            );
            assert_eq!(
                r["writer"].1.inclusoes_escritas,
                InclusoesEscritas::Conhecidas(BTreeSet::from([EscritaParametro {
                    objeto: CaminhoParametro {
                        parametro: 0,
                        campos: vec![9, 2]
                    },
                    indice: 0,
                    valor: CaminhoParametro::raiz(1),
                }]))
            );
            for (_, resumo) in r.values() {
                assert_eq!(resumo.campos(), CamposHeapArc::Desconhecidos);
                assert_eq!(resumo.publicacoes(), PublicacoesHeapArc::Desconhecidas);
            }
            // A API sem estado de campos continua conservadora.
            assert_eq!(r["wrapper"].1.retorno(), &RetornoHeapArc::Desconhecido);
        }
    }

    #[test]
    fn retorno_de_campo_recursivo_cresce_ate_topo_e_ciclo_sem_base_nao_expoe_bottom() {
        let mut f = getter("rec", 0);
        f.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "rec".into(),
                args: vec![Operand::Val(ValueId(1))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::Phi {
                incoming: vec![
                    (BlockId(1), Operand::Val(ValueId(1))),
                    (BlockId(2), Operand::Val(ValueId(2))),
                ],
                ty: Type::Ref,
            },
            Type::Ref,
        ));
        // As duas saídas alimentam o Phi com predecessores SSA reais.
        let mut corpo = f.blocks.remove(0);
        let phi = corpo.instructions.pop().unwrap();
        corpo.terminator = Terminator::CondBranch {
            cond: Operand::Val(ValueId(4)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        f.params.push((ValueId(4), "cond".into(), Type::I1));
        if let Instruction::CallStatic { args, .. } = &mut corpo.instructions[1].1 {
            args.push(Operand::Val(ValueId(4)));
        }
        f.blocks = vec![
            corpo,
            BasicBlock {
                id: BlockId(1),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![phi],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            },
        ];
        let mut sem_base = getter("sem_base", 0);
        sem_base.blocks[0].instructions = vec![(
            ValueId(1),
            Instruction::CallStatic {
                symbol: "sem_base".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )];
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![f, sem_base];
        for ordem in [false, true] {
            if ordem {
                m.functions.reverse();
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["rec", "sem_base"]), 3)
                .unwrap();
            assert_eq!(r["rec"].1.retorno_caminhos, RetornoCaminhos::desconhecido());
            assert_eq!(
                r["sem_base"].1.retorno_caminhos,
                RetornoCaminhos::desconhecido()
            );
        }
    }

    fn ligar(symbol: &str, invertido: bool) -> Function {
        Function {
            symbol: symbol.into(),
            name: symbol.into(),
            depuracao: None,
            params: vec![
                (ValueId(0), "a".into(), Type::Ref),
                (ValueId(1), "b".into(), Type::Ref),
            ],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(2),
                    Instruction::SetField {
                        object: Operand::Val(ValueId(u32::from(invertido))),
                        index: 0,
                        value: Operand::Val(ValueId(u32::from(!invertido))),
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        }
    }

    fn esperadas(pares: &[(usize, usize)]) -> InclusoesEscritas {
        InclusoesEscritas::Conhecidas(
            pares
                .iter()
                .map(|&(objeto, valor)| EscritaParametro {
                    objeto: CaminhoParametro::raiz(objeto),
                    indice: 0,
                    valor: CaminhoParametro::raiz(valor),
                })
                .collect(),
        )
    }

    #[test]
    fn leituras_encadeadas_e_wrapper_preservam_caminhos_dos_argumentos() {
        let mut f = ligar("profundo", false);
        f.blocks[0].instructions = vec![
            (
                ValueId(2),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::GetField {
                    object: Operand::Val(ValueId(2)),
                    index: 1,
                },
                Type::Ref,
            ),
            (
                ValueId(4),
                Instruction::GetField {
                    object: Operand::Val(ValueId(1)),
                    index: 2,
                },
                Type::Ref,
            ),
            (
                ValueId(5),
                Instruction::SetField {
                    object: Operand::Val(ValueId(3)),
                    index: 4,
                    value: Operand::Val(ValueId(4)),
                },
                Type::Void,
            ),
        ];
        let esperado = |prefixo: Vec<usize>| {
            InclusoesEscritas::Conhecidas(BTreeSet::from([EscritaParametro {
                objeto: CaminhoParametro {
                    parametro: 0,
                    campos: prefixo,
                },
                indice: 4,
                valor: CaminhoParametro {
                    parametro: 1,
                    campos: vec![2],
                },
            }]))
        };
        assert_eq!(extrair(&f, 8, |_| None).unwrap(), esperado(vec![0, 1]));
        assert_eq!(
            extrair(&f, 1, |_| None).unwrap(),
            InclusoesEscritas::Desconhecidas
        );
        let mut wrapper = ligar("wrapper", false);
        wrapper.blocks[0].instructions = vec![
            (
                ValueId(2),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 9,
                },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "profundo".into(),
                    args: vec![Operand::Val(ValueId(2)), Operand::Val(ValueId(1))],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ];
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![wrapper, f];
        for ordem in [false, true] {
            if ordem {
                m.functions.reverse();
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["wrapper"]), 8).unwrap();
            assert_eq!(r["wrapper"].1.inclusoes_escritas, esperado(vec![9, 0, 1]));
            assert_eq!(r["wrapper"].1.campos(), CamposHeapArc::Desconhecidos);
            assert_eq!(
                r["wrapper"].1.publicacoes(),
                PublicacoesHeapArc::Desconhecidas
            );
        }
    }

    #[test]
    fn caminhos_crescentes_em_scc_e_phi_atingem_topo_sem_truncar() {
        let mut f = ligar("recursivo", false);
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::GetField {
                object: Operand::Val(ValueId(0)),
                index: 0,
            },
            Type::Ref,
        ));
        f.blocks[0].instructions.push((
            ValueId(4),
            Instruction::CallStatic {
                symbol: "recursivo".into(),
                args: vec![Operand::Val(ValueId(3)), Operand::Val(ValueId(1))],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions.push(f);
        let r = super::super::chamadas::resolver(&m, &HashSet::from(["recursivo"]), 4).unwrap();
        assert_eq!(
            r["recursivo"].1.inclusoes_escritas,
            InclusoesEscritas::Desconhecidas
        );
        let mut f = ligar("phi_crescente", false);
        f.params.push((ValueId(5), "cond".into(), Type::I1));
        f.blocks = vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (
                        ValueId(2),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(1), Operand::Val(ValueId(3))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(5)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];
        assert_eq!(
            extrair(&f, 3, |_| None).unwrap(),
            InclusoesEscritas::Desconhecidas
        );
    }

    #[test]
    fn escrita_parametrica_nao_promove_cobertura_e_null_nao_cria_aresta() {
        let mut f = ligar("ligar", false);
        assert_eq!(extrair(&f, 8, |_| None).unwrap(), esperadas(&[(0, 1)]));
        let r = super::super::resumos::extrair(&f, 8).unwrap();
        assert_eq!(r.campos(), CamposHeapArc::Desconhecidos);
        assert_eq!(r.publicacoes(), PublicacoesHeapArc::Desconhecidas);
        let Instruction::SetField { value, .. } = &mut f.blocks[0].instructions[0].1 else {
            unreachable!()
        };
        *value = Operand::Constant(Constant::Null);
        assert_eq!(extrair(&f, 8, |_| None).unwrap(), esperadas(&[]));
        assert_eq!(
            super::super::resumos::extrair(&f, 8).unwrap().campos(),
            CamposHeapArc::Desconhecidos
        );
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallRuntime {
                name: "externo".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        assert_eq!(
            extrair(&f, 8, |_| None).unwrap(),
            InclusoesEscritas::Desconhecidas
        );
    }

    #[test]
    fn aliases_por_copy_e_retorno_local_instanciam_os_argumentos() {
        let mut f = ligar("ligar", false);
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(3),
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(1)),
                },
                Type::Ref,
            ),
        );
        f.blocks[0].instructions.insert(
            1,
            (
                ValueId(4),
                Instruction::CallStatic {
                    symbol: "id".into(),
                    args: vec![Operand::Val(ValueId(3))],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
        );
        let Instruction::SetField { value, .. } = &mut f.blocks[0].instructions[2].1 else {
            unreachable!()
        };
        *value = Operand::Val(ValueId(4));
        let id = Function {
            symbol: "id".into(),
            name: "id".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![f, id];
        let roots = HashSet::from(["ligar"]);
        let r = super::super::chamadas::resolver(&m, &roots, 8).unwrap();
        assert_eq!(r["ligar"].1.inclusoes_escritas, esperadas(&[(0, 1)]));
        // Representação igual em bits não autoriza converter um argumento
        // I64 em Ref para sustentar o resumo de stores.
        m.functions[0].params[1].2 = Type::I64;
        assert!(super::super::chamadas::resolver(&m, &roots, 8).is_err());
        m.functions[0].params[1].2 = Type::Ref;
        let Instruction::CallStatic { args, .. } = &mut m.functions[0].blocks[0].instructions[1].1
        else {
            unreachable!()
        };
        args[0] = Operand::Constant(Constant::Int(7));
        let r = super::super::chamadas::resolver(&m, &roots, 8).unwrap();
        assert_eq!(
            r["ligar"].1.inclusoes_escritas,
            InclusoesEscritas::Desconhecidas
        );
    }

    #[test]
    fn phi_conserva_produto_de_aliases_e_budget_do_resumo_inteiro() {
        let mut f = ligar("phi", false);
        f.params.push((ValueId(2), "cond".into(), Type::I1));
        f.blocks = vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(2)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(0))),
                                (BlockId(2), Operand::Val(ValueId(1))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(3)),
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(3)),
                            index: 0,
                            value: Operand::Val(ValueId(4)),
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            },
        ];
        assert_eq!(
            extrair(&f, 4, |_| None).unwrap(),
            esperadas(&[(0, 0), (0, 1), (1, 0), (1, 1)])
        );
        assert_eq!(
            extrair(&f, 3, |_| None).unwrap(),
            InclusoesEscritas::Desconhecidas
        );
        // Um caminho null não se torna uma alocação ou argumento fresco.
        let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else {
            unreachable!()
        };
        incoming[1].1 = Operand::Constant(Constant::Null);
        assert_eq!(extrair(&f, 4, |_| None).unwrap(), esperadas(&[(0, 0)]));
    }

    #[test]
    fn cadeia_de_mil_escritores_compõe_sem_depender_da_ordem_dos_corpos() {
        let mut m = Module::new();
        m.memoria_arc = true;
        for n in 0..1000 {
            let mut f = ligar(&format!("s{n:04}"), false);
            if n != 999 {
                f.blocks[0].instructions[0].1 = Instruction::CallStatic {
                    symbol: format!("s{:04}", n + 1),
                    args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                    ret_ty: Type::Void,
                };
            }
            m.functions.push(f);
        }
        for ordem in [false, true] {
            if ordem {
                m.functions.reverse();
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["s0000"]), 8).unwrap();
            assert_eq!(r.len(), 1000);
            for n in 0..1000 {
                let esperado = if (999 - n) % 2 == 0 { (0, 1) } else { (1, 0) };
                assert_eq!(
                    r[&format!("s{n:04}")].1.inclusoes_escritas,
                    esperadas(&[esperado])
                );
            }
        }
    }

    #[test]
    fn scc_de_escritores_compõe_permutacao_e_eleva_budget_a_topo() {
        let mut f = ligar("f", false);
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallStatic {
                symbol: "g".into(),
                args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let mut g = ligar("g", false);
        g.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "f".into(),
            args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(1))],
            ret_ty: Type::Void,
        };
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![f, g];
        for ordem in [false, true] {
            if ordem {
                m.functions.reverse();
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["f"]), 8).unwrap();
            for (_, resumo) in r.values() {
                assert_eq!(resumo.inclusoes_escritas, esperadas(&[(0, 1), (1, 0)]));
                assert_eq!(resumo.campos(), CamposHeapArc::Desconhecidos);
                assert_eq!(resumo.publicacoes(), PublicacoesHeapArc::Desconhecidas);
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["f"]), 1).unwrap();
            assert!(
                r.values()
                    .all(|(_, r)| r.inclusoes_escritas == InclusoesEscritas::Desconhecidas)
            );
        }
        let f = m.functions.iter_mut().find(|f| f.symbol == "f").unwrap();
        f.blocks[0].instructions.push((
            ValueId(9),
            Instruction::CallStatic {
                symbol: "fora".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let r = super::super::chamadas::resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.inclusoes_escritas == InclusoesEscritas::Desconhecidas)
        );
    }
}
