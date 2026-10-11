//! Preparação conjunta de corpos separados para emissão em módulos distintos.

use super::*;

/// Prepara atomicamente um conjunto fechado de módulos Dart com a mesma ABI ARC.
///
/// Chamadas entre módulos usam os corpos desta transação. Nenhum resumo de um
/// SDK já emitido é presumido Owned: os corpos preparados devem ser os emitidos.
/// Preserva a distribuição das funções, globais e demais metadados de cada
/// módulo. Publica somente corpos, tabelas verificadas e planos após sucesso do
/// conjunto inteiro. Símbolos de funções/globais precisam ser distintos; layouts
/// compartilhados precisam coincidir. Tracing é uma operação sem alterações.
/// Não integra o pipeline nem certifica dispatch indireto, callbacks ou recarga.
///
/// # Erros
/// Quantidades/modos incompatíveis, fatos atribuídos ao módulo errado, símbolos
/// repetidos, layouts conflitantes ou qualquer falha da preparação ARC.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let folha = Function { symbol: "folha".into(), name: "folha".into(),
///     depuracao: None, params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// let mut caller = folha.clone(); caller.symbol = "caller".into();
/// caller.blocks[0].instructions.push((ValueId(0), Instruction::CallStatic {
///     symbol: "folha".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
/// let mut a = Module::new(); a.memoria_arc = true; a.functions.push(caller);
/// let mut b = Module::new(); b.memoria_arc = true; b.functions.push(folha);
/// let mut modulos = [a, b];
/// let mut planos = [HashMap::from([("caller".into(), PlanoFuncaoDart::default())]),
///     HashMap::from([("folha".into(), PlanoFuncaoDart::default())])];
/// assert_eq!(preparar_arc_modulos_dart(&mut modulos, &mut planos)?, (0, 0));
/// # Ok::<(), String>(())
/// ```
pub fn preparar_arc_modulos_dart(
    modulos: &mut [Module],
    planos: &mut [HashMap<String, PlanoFuncaoDart>],
) -> Result<(usize, usize), String> {
    if modulos.len() != planos.len() {
        return Err("módulos ARC: quantidade de planos incompatível".into());
    }
    let Some(primeiro) = modulos.first() else {
        return Ok((0, 0));
    };
    if modulos.iter().any(|m| {
        m.memoria_arc != primeiro.memoria_arc
            || m.excecoes_por_tabelas != primeiro.excecoes_por_tabelas
    }) {
        return Err("módulos ARC: modos de memória/exceções incompatíveis".into());
    }
    if !primeiro.memoria_arc {
        return Ok((0, 0));
    }
    let mut conjunto = Module::new();
    conjunto.memoria_arc = true;
    conjunto.excecoes_por_tabelas = primeiro.excecoes_por_tabelas;
    let mut novos = HashMap::new();
    let mut simbolos = HashSet::new();
    let mut globais = HashSet::new();
    let mut classes = HashMap::<u32, usize>::new();
    for (m, p) in modulos.iter().zip(planos.iter()) {
        let locais: HashSet<_> = m.functions.iter().map(|f| &f.symbol).collect();
        if locais.len() != m.functions.len()
            || locais.len() != p.len()
            || p.keys().any(|s| !locais.contains(s))
        {
            return Err("módulos ARC: funções/planos locais incompatíveis".into());
        }
        let fatos = m
            .parametros_escalares_dart
            .keys()
            .chain(m.parametros_rti_dart.keys())
            .chain(m.parametros_vetores_dart.keys())
            .chain(m.retornos_ref_dart.iter())
            .chain(m.retornos_uniformes_arc.iter())
            .chain(m.sitios_arc.keys());
        for s in fatos {
            if !locais.contains(s) {
                return Err(format!("módulos ARC: fato fora do módulo de {s}"));
            }
        }
        for f in &m.functions {
            if !simbolos.insert(f.symbol.clone()) {
                return Err(format!("módulos ARC: função repetida {}", f.symbol));
            }
        }
        for g in &m.globais {
            if !globais.insert(g.2.clone()) || !globais.insert(format!("{}$ok", g.2)) {
                return Err(format!("módulos ARC: global sobreposto {}", g.2));
            }
        }
        for s in &m.caches_rti_arc {
            if !m.globais.iter().any(|g| &g.2 == s) {
                return Err(format!("módulos ARC: cache fora do módulo de {s}"));
            }
        }
        super::layouts::verificar(m)?;
        let mut ids_locais = HashSet::new();
        for c in &m.classes {
            if !ids_locais.insert(c.id) {
                return Err(format!("módulos ARC: classe local repetida {}", c.id));
            }
            if let Some(&indice) = classes.get(&c.id) {
                let anterior = &conjunto.classes[indice];
                // O passe consulta somente identidade e forma física. Seletores
                // e tabelas de métodos continuam nos módulos de emissão reais.
                if anterior.name != c.name || anterior.field_count != c.field_count {
                    return Err(format!("módulos ARC: classe conflitante {}", c.id));
                }
            } else {
                classes.insert(c.id, conjunto.classes.len());
                conjunto.classes.push(c.clone());
            }
        }
        for (cid, layout) in &m.layouts_campos_arc {
            if conjunto
                .layouts_campos_arc
                .get(cid)
                .is_some_and(|l| l != layout)
            {
                return Err(format!("módulos ARC: layout conflitante da classe {cid}"));
            }
            conjunto.layouts_campos_arc.insert(*cid, layout.clone());
        }
        if conjunto.excecoes_por_tabelas && m.functions.len() != m.tabelas.len() {
            return Err("módulos ARC: inventário excepcional local incompleto".into());
        }
        conjunto.functions.extend(m.functions.clone());
        conjunto.tabelas.extend(m.tabelas.clone());
        conjunto.globais.extend(m.globais.clone());
        conjunto
            .parametros_escalares_dart
            .extend(m.parametros_escalares_dart.clone());
        conjunto
            .parametros_rti_dart
            .extend(m.parametros_rti_dart.clone());
        conjunto
            .parametros_vetores_dart
            .extend(m.parametros_vetores_dart.clone());
        conjunto
            .retornos_ref_dart
            .extend(m.retornos_ref_dart.clone());
        conjunto
            .retornos_uniformes_arc
            .extend(m.retornos_uniformes_arc.clone());
        conjunto.caches_rti_arc.extend(m.caches_rti_arc.clone());
        conjunto.sitios_arc.extend(m.sitios_arc.clone());
        novos.extend(p.clone());
    }
    let resultado = if conjunto.excecoes_por_tabelas {
        preparar_arc_modulo_tabelado(&mut conjunto, &mut novos)?
    } else {
        preparar_arc_modulo_dart(&mut conjunto, &mut novos)?
    };
    // Nenhuma operação abaixo pode reprovar: todos os símbolos, quantidades e
    // tabelas foram validados. Distribuição e ordem pertencem aos módulos reais.
    let mut inicio = 0;
    for (m, p) in modulos.iter_mut().zip(planos.iter_mut()) {
        let fim = inicio + m.functions.len();
        m.functions
            .clone_from_slice(&conjunto.functions[inicio..fim]);
        if conjunto.excecoes_por_tabelas {
            m.tabelas = conjunto.tabelas[inicio..fim].to_vec();
        }
        for (s, plano) in p {
            *plano = novos[s].clone();
        }
        inicio = fim;
    }
    Ok(resultado)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn entrada_uniforme_encaminha_descritor_entre_modulos_sem_publica_lo() {
        for invalido in [false, true] {
            let mut modulos = Vec::new();
            for simbolo in ["caller", "callee"] {
                let mut m = Module::new();
                m.memoria_arc = true;
                let params = vec![
                    (ValueId(0), "ctx".into(), Type::Ref),
                    (ValueId(1), "args".into(), Type::Ptr),
                    (ValueId(2), "desc".into(), Type::Ptr),
                ];
                let (instructions, valor) = if simbolo == "caller" {
                    (
                        vec![(
                            ValueId(3),
                            Instruction::CallStatic {
                                symbol: "callee".into(),
                                args: vec![
                                    Operand::Val(ValueId(0)),
                                    Operand::Val(ValueId(1)),
                                    Operand::Val(ValueId(2)),
                                ],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        )],
                        ValueId(3),
                    )
                } else {
                    (vec![], ValueId(0))
                };
                m.functions.push(Function {
                    symbol: simbolo.into(),
                    name: simbolo.into(),
                    params,
                    return_ty: Type::Ref,
                    depuracao: None,
                    blocks: vec![BasicBlock {
                        id: BlockId(0),
                        instructions,
                        terminator: Terminator::Return(Some(Operand::Val(valor))),
                    }],
                });
                m.parametros_vetores_dart.insert(
                    simbolo.into(),
                    HashMap::from([
                        (ValueId(1), VetorEntradaDart::Argumentos),
                        (ValueId(2), VetorEntradaDart::Descritor),
                    ]),
                );
                m.retornos_uniformes_arc.insert(simbolo.into());
                modulos.push(m);
            }
            let mut ms = modulos;
            if invalido {
                ms[1].parametros_vetores_dart.clear();
            }
            let mut planos: Vec<HashMap<_, _>> = ms
                .iter()
                .map(|m| {
                    m.functions
                        .iter()
                        .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
                        .collect()
                })
                .collect();
            let antes = format!("{ms:?}/{planos:?}");
            let r = preparar_arc_modulos_dart(&mut ms, &mut planos);
            if invalido {
                assert!(r.is_err());
                assert_eq!(format!("{ms:?}/{planos:?}"), antes);
            } else {
                assert_eq!(r.unwrap(), (1, 0));
                assert_eq!(planos[0]["caller"].classes[&ValueId(3)], Ownership::Owned);
                assert_eq!(planos[0]["caller"].classes[&ValueId(2)], Ownership::Trivial);
            }
        }
    }

    fn entrada() -> (Vec<Module>, Vec<HashMap<String, PlanoFuncaoDart>>) {
        let mut programa = Module::new();
        programa.memoria_arc = true;
        programa.functions.push(Function {
            symbol: "programa".into(),
            name: "programa".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
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
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: "biblioteca".into(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        });
        let mut biblioteca = Module::new();
        biblioteca.memoria_arc = true;
        biblioteca.functions.push(Function {
            symbol: "biblioteca".into(),
            name: "biblioteca".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        biblioteca.retornos_ref_dart.insert("biblioteca".into());
        let modulos = vec![programa, biblioteca];
        let planos = modulos
            .iter()
            .map(|m| {
                m.functions
                    .iter()
                    .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
                    .collect()
            })
            .collect();
        (modulos, planos)
    }

    #[test]
    fn chamadas_entre_modulos_exigem_verificacao_conjunta_e_publicacao_atomica() {
        for caso in 0..20 {
            let (mut modulos, mut planos) = entrada();
            match caso {
                1 => modulos[1].retornos_ref_dart.clear(),
                2 => {
                    modulos.pop();
                    planos.pop();
                }
                3 => modulos[1].functions[0].params[0].2 = Type::I64,
                4 => modulos[1].functions[0].symbol = "programa".into(),
                5 => {
                    modulos[0]
                        .parametros_rti_dart
                        .insert("biblioteca".into(), HashSet::from([ValueId(0)]));
                }
                6 | 7 => {
                    for m in &mut modulos {
                        m.classes.push(ClassDef {
                            id: 42,
                            name: "C".into(),
                            field_count: 1,
                            vtable: vec![],
                            to_string_symbol: None,
                        });
                    }
                    modulos[0].layouts_campos_arc.insert(42, vec![Type::Ref]);
                    modulos[1]
                        .layouts_campos_arc
                        .insert(42, vec![if caso == 6 { Type::I64 } else { Type::Ref }]);
                }
                8 => {
                    for m in &mut modulos {
                        m.globais.push((0, Type::I64, "sobreposto".into()));
                    }
                }
                9 => modulos[1].memoria_arc = false,
                10 => {
                    for m in &mut modulos {
                        m.memoria_arc = false;
                        m.retornos_uniformes_arc.insert("obsoleto".into());
                    }
                }
                11 => {
                    modulos[1].functions[0].blocks[0].terminator =
                        Terminator::Return(Some(Operand::Constant(Constant::Int(42))))
                }
                12 => modulos[1].excecoes_por_tabelas = true,
                13 | 14 => {
                    for m in &mut modulos {
                        m.excecoes_por_tabelas = true;
                        m.tabelas.push(TabelasDaFuncao::default());
                    }
                    if caso == 14 {
                        modulos[1].tabelas.clear();
                    }
                }
                15 => {
                    modulos.reverse();
                    planos.reverse();
                }
                16 => {
                    let mut vazio = Module::new();
                    vazio.memoria_arc = true;
                    modulos.push(vazio);
                    planos.push(HashMap::new());
                }
                17 => {
                    let deslocado = planos[1].remove("biblioteca").unwrap();
                    planos[0].insert("biblioteca".into(), deslocado);
                }
                18 => {
                    planos[1].get_mut("biblioteca").unwrap().tokens.retorno =
                        RetornoTokens::Borrowed
                }
                19 => {
                    modulos[0].caches_rti_arc.insert("da_biblioteca".into());
                    modulos[1]
                        .globais
                        .push((0, Type::I64, "da_biblioteca".into()));
                }
                _ => {}
            }
            let antes = format!("{modulos:?}/{planos:?}");
            let resultado = preparar_arc_modulos_dart(&mut modulos, &mut planos);
            if matches!(caso, 0 | 7 | 10 | 13 | 15 | 16) {
                if caso == 10 {
                    assert_eq!(resultado.unwrap(), (0, 0));
                    assert_eq!(format!("{modulos:?}/{planos:?}"), antes);
                    continue;
                }
                assert_eq!(
                    resultado.unwrap_or_else(|e| panic!("caso {caso}: {e}")),
                    (1, 2)
                );
                let biblioteca = modulos
                    .iter()
                    .find(|m| m.functions.iter().any(|f| f.symbol == "biblioteca"))
                    .unwrap();
                assert!(
                    biblioteca.functions[0]
                        .blocks
                        .iter()
                        .flat_map(|b| &b.instructions)
                        .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
                );
                let programa = modulos
                    .iter()
                    .position(|m| m.functions.iter().any(|f| f.symbol == "programa"))
                    .unwrap();
                assert_eq!(
                    planos[programa]["programa"].classes[&ValueId(1)],
                    Ownership::Owned
                );
                if caso == 13 {
                    for (m, p) in modulos.iter().zip(&planos) {
                        assert_eq!(
                            m.tabelas,
                            m.functions
                                .iter()
                                .map(|f| p[&f.symbol].tabelas.clone())
                                .collect::<Vec<_>>()
                        );
                    }
                }
                let depois = format!("{modulos:?}/{planos:?}");
                assert_eq!(
                    preparar_arc_modulos_dart(&mut modulos, &mut planos).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{modulos:?}/{planos:?}"), depois, "caso {caso}");
            } else {
                assert!(resultado.is_err(), "caso {caso}: {resultado:?}");
                assert_eq!(format!("{modulos:?}/{planos:?}"), antes, "caso {caso}");
            }
        }
        let (mut modulos, mut planos) = entrada();
        planos.pop();
        let antes = format!("{modulos:?}/{planos:?}");
        assert!(preparar_arc_modulos_dart(&mut modulos, &mut planos).is_err());
        assert_eq!(format!("{modulos:?}/{planos:?}"), antes);
        assert_eq!(preparar_arc_modulos_dart(&mut [], &mut []).unwrap(), (0, 0));
    }
}
