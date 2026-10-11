//! Preparação conjunta de corpos separados para emissão em módulos distintos.

use super::*;

/// Prepara atomicamente um conjunto fechado de módulos Dart com a mesma ABI ARC.
///
/// Chamadas entre módulos usam os corpos desta transação. Nenhum resumo de um
/// SDK já emitido é presumido Owned: os corpos preparados devem ser os emitidos.
/// Preserva a distribuição das funções, globais e demais metadados de cada
/// módulo. Publica somente corpos, tabelas verificadas e planos após sucesso do
/// conjunto inteiro. Cada variante compartilhada é verificada separadamente,
/// com a mesma interface e áreas globais distintas. Falhas de qualquer variante
/// propagam aos chamadores. Não prova equivalência funcional ODR. Layouts
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
    let mut definicoes: HashMap<String, (usize, usize)> = HashMap::new();
    let mut variantes: HashMap<String, Vec<String>> = HashMap::new();
    let mut distribuicao = Vec::new();
    let mut origens_variantes = Vec::new();
    let mut classes = HashMap::<u32, usize>::new();
    let mut usados: HashSet<_> = modulos
        .iter()
        .flat_map(|m| {
            m.functions.iter().map(|f| f.symbol.clone()).chain(
                m.globais
                    .iter()
                    .flat_map(|g| [g.2.clone(), format!("{}$ok", g.2)]),
            )
        })
        .collect();
    let mut proximo = 0u64;
    for (indice_modulo, (m, p)) in modulos.iter().zip(planos.iter()).enumerate() {
        // Entradas nativas continuam abertas ao inferir argumentos do grupo.
        conjunto.ajudantes.extend(m.ajudantes.iter().cloned());
        conjunto.tabelas_de_metodos.extend(m.tabelas_de_metodos.iter().cloned());
        let locais: HashSet<_> = m.functions.iter().map(|f| &f.symbol).collect();
        if locais.len() != m.functions.len()
            || locais.len() != p.len()
            || p.keys().any(|s| !locais.contains(s))
        {
            return Err("módulos ARC: funções/planos locais incompatíveis".into());
        }
        for s in m
            .parametros_escalares_dart
            .keys()
            .chain(m.parametros_rti_dart.keys())
            .chain(m.parametros_vetores_dart.keys())
            .chain(m.retornos_ref_dart.iter())
            .chain(m.retornos_uniformes_arc.iter())
            .chain(m.funcoes_compartilhadas_arc.iter())
            .chain(m.sitios_arc.keys())
        {
            if !locais.contains(s) {
                return Err(format!("módulos ARC: fato fora do módulo de {s}"));
            }
        }
        if conjunto.excecoes_por_tabelas && m.functions.len() != m.tabelas.len() {
            return Err("módulos ARC: inventário excepcional local incompleto".into());
        }
        let mut area = HashMap::new();
        for (raiz, ty, nome) in &m.globais {
            let privado = nome_privado(&mut usados, &mut proximo, "global")?;
            if area.insert(nome.clone(), privado.clone()).is_some()
                || area
                    .insert(format!("{nome}$ok"), format!("{privado}$ok"))
                    .is_some()
            {
                return Err(format!("módulos ARC: global local sobreposto {nome}"));
            }
            conjunto.globais.push((*raiz, *ty, privado));
        }
        for s in &m.caches_rti_arc {
            let nome = area
                .get(s)
                .ok_or_else(|| format!("módulos ARC: cache fora do módulo de {s}"))?;
            conjunto.caches_rti_arc.insert(nome.clone());
        }
        super::layouts::verificar(m)?;
        let mut ids_locais = HashSet::new();
        for c in &m.classes {
            if !ids_locais.insert(c.id) {
                return Err(format!("módulos ARC: classe local repetida {}", c.id));
            }
            if let Some(&indice) = classes.get(&c.id) {
                let anterior = &conjunto.classes[indice];
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
        let mut indices = Vec::new();
        for (indice_funcao, original) in m.functions.iter().enumerate() {
            let simbolo = &original.symbol;
            let privado = if let Some(&(am, af)) = definicoes.get(simbolo) {
                let anterior = &modulos[am];
                if !m.modo_sdk
                    || !anterior.modo_sdk
                    || !m.funcoes_compartilhadas_arc.contains(simbolo)
                    || !anterior.funcoes_compartilhadas_arc.contains(simbolo)
                {
                    return Err(format!("módulos ARC: função repetida {simbolo}"));
                }
                if protocolo(anterior, &anterior.functions[af], &planos[am][simbolo])
                    != protocolo(m, original, &p[simbolo])
                {
                    return Err(format!(
                        "módulos ARC: ABI compartilhada divergente {simbolo}"
                    ));
                }
                let nome = nome_privado(&mut usados, &mut proximo, "funcao")?;
                variantes
                    .entry(simbolo.clone())
                    .or_default()
                    .push(nome.clone());
                nome
            } else {
                definicoes.insert(simbolo.clone(), (indice_modulo, indice_funcao));
                simbolo.clone()
            };
            if privado != *simbolo {
                origens_variantes.push((privado.clone(), simbolo.clone(), indice_modulo));
            }
            let mut f = original.clone();
            f.symbol = privado.clone();
            remapear_area(&mut f, &area)?;
            let mut plano = p[simbolo].clone();
            if conjunto.excecoes_por_tabelas {
                let t = &m.tabelas[indice_funcao];
                if plano.tabelas != TabelasDaFuncao::default() && plano.tabelas != *t {
                    return Err(format!(
                        "módulos ARC: tabelas locais conflitantes {simbolo}"
                    ));
                }
                plano.tabelas = t.clone();
            }

            if let Some(c) = &plano.tabelas.cleanup_estrangeiro {
                c.conferir(original, &plano.tabelas)?;
            }
            plano.tabelas.cleanup_estrangeiro = CleanupEstrangeiro::novo(&f, &plano)?;
            if let Some(fatos) = m.parametros_escalares_dart.get(simbolo) {
                conjunto
                    .parametros_escalares_dart
                    .insert(privado.clone(), fatos.clone());
            }
            if let Some(fatos) = m.parametros_rti_dart.get(simbolo) {
                conjunto
                    .parametros_rti_dart
                    .insert(privado.clone(), fatos.clone());
            }
            if let Some(fatos) = m.parametros_vetores_dart.get(simbolo) {
                conjunto
                    .parametros_vetores_dart
                    .insert(privado.clone(), fatos.clone());
            }
            if let Some(fatos) = m.sitios_arc.get(simbolo) {
                conjunto.sitios_arc.insert(privado.clone(), fatos.clone());
            }
            if m.retornos_ref_dart.contains(simbolo) {
                conjunto.retornos_ref_dart.insert(privado.clone());
            }
            if m.retornos_uniformes_arc.contains(simbolo) {
                conjunto.retornos_uniformes_arc.insert(privado.clone());
            }
            novos.insert(privado, plano);
            indices.push(conjunto.functions.len());
            conjunto.functions.push(f);
            if conjunto.excecoes_por_tabelas {
                conjunto.tabelas.push(
                    novos[&conjunto.functions.last().unwrap().symbol]
                        .tabelas
                        .clone(),
                );
            }
        }
        distribuicao.push((indices, area));
    }
    if conjunto.excecoes_por_tabelas {
        produzir_tabelas_arc_do_modulo(&conjunto, &mut novos)?;
    }
    let nomes_antes: Vec<_> = conjunto.functions.iter().map(|f| f.symbol.clone()).collect();
    let resultado = super::funcoes::preparar_com_variantes(&mut conjunto, &mut novos, &variantes)
        .map_err(|mut erro| {
        origens_variantes.sort_by_key(|(s, _, _)| std::cmp::Reverse(s.len()));
        for (privado, original, modulo) in &origens_variantes {
            if erro.contains(privado) {
                erro = erro.replace(privado, original);
                erro.push_str(&format!(" (variante do módulo {modulo})"));
            }
        }
        erro
    })?;
    // Restaura os nomes físicos e vincula novamente os certificados ainda em
    // cópias privadas. Qualquer falha preserva todos os módulos de entrada.
    let mut nomes_fisicos = HashMap::new();
    let mut fatos_fisicos = Vec::new();
    for (m, (indices, _)) in modulos.iter().zip(&distribuicao) {
        let mut mapa = HashMap::new();
        for (original, &indice) in m.functions.iter().zip(indices) {
            let antes = &nomes_antes[indice];
            let atual = &conjunto.functions[indice].symbol;
            let fisico = if atual == antes { original.symbol.clone() }
                else if atual == &format!("{antes}$arcslots_v1") { format!("{}$arcslots_v1", original.symbol) }
                else { return Err("módulos ARC: mudança de assinatura/símbolo não reconhecida".into()); };
            nomes_fisicos.insert(atual.clone(), fisico.clone());
            mapa.insert(original.symbol.clone(), fisico);
        }
        fatos_fisicos.push(mapa);
    }
    let mut publicacoes = Vec::new();
    for (indice_modulo, ((m, (indices, area)), fatos)) in modulos.iter().zip(distribuicao).zip(fatos_fisicos).enumerate() {
        let inversa: HashMap<_, _> = area.into_iter().map(|(a, b)| (b, a)).collect();
        let mut funcoes = Vec::new();
        let mut ps = planos[indice_modulo].clone();
        for (original, indice) in m.functions.iter().zip(indices) {
            let mut f = conjunto.functions[indice].clone();
            let mut plano = novos[&f.symbol].clone();
            super::slots_planos::remapear_funcao(&mut f, &nomes_fisicos);
            remapear_area(&mut f, &inversa)?;
            super::slots_planos::transportar_contrato(&mut plano, &nomes_fisicos);
            plano.tabelas.cleanup_estrangeiro = CleanupEstrangeiro::novo(&f, &plano)?;
            ps.remove(&original.symbol);
            ps.insert(f.symbol.clone(), plano);
            funcoes.push(f);
        }
        publicacoes.push((funcoes, ps, fatos));
    }
    for ((m, p), (fs, ps, fatos)) in modulos.iter_mut().zip(planos.iter_mut()).zip(publicacoes) {
        m.functions = fs;
        super::slots_planos::remapear_fatos(m, &fatos);
        if m.excecoes_por_tabelas {
            m.tabelas = m
                .functions
                .iter()
                .map(|f| ps[&f.symbol].tabelas.clone())
                .collect();
        }
        *p = ps;
    }
    Ok(resultado)
}

fn nome_privado(
    usados: &mut HashSet<String>,
    proximo: &mut u64,
    papel: &str,
) -> Result<String, String> {
    loop {
        let nome = format!("df.arc.privado.{papel}.{proximo}");
        *proximo = proximo
            .checked_add(1)
            .ok_or("nomes privados ARC esgotados")?;
        if usados.insert(nome.clone()) {
            return Ok(nome);
        }
    }
}

// Interface verificada, não igualdade do corpo nem promessa de ODR funcional.
// IDs SSA são locais: os papéis semânticos pertencem às posições da assinatura.
fn protocolo(
    m: &Module,
    f: &Function,
    p: &PlanoFuncaoDart,
) -> (
    Type,
    RetornoTokens,
    Vec<(Type, bool, bool, Option<VetorEntradaDart>)>,
) {
    let retorno = if m.retornos_ref_dart.contains(&f.symbol)
        || m.retornos_uniformes_arc.contains(&f.symbol)
    {
        RetornoTokens::Owned
    } else {
        p.tokens.retorno
    };
    let parametros = f
        .params
        .iter()
        .map(|(v, _, t)| {
            (
                *t,
                m.parametros_escalares_dart
                    .get(&f.symbol)
                    .is_some_and(|s| s.contains(v)),
                m.parametros_rti_dart
                    .get(&f.symbol)
                    .is_some_and(|s| s.contains(v)),
                m.parametros_vetores_dart
                    .get(&f.symbol)
                    .and_then(|s| s.get(v))
                    .copied(),
            )
        })
        .collect();
    (f.return_ty, retorno, parametros)
}

fn remapear_area(f: &mut Function, area: &HashMap<String, String>) -> Result<(), String> {
    for b in &f.blocks {
        let mut endereco = false;
        for (_, i, _) in &b.instructions {
            endereco |=
                matches!(i, Instruction::Const(Constant::Funcao(s)) if area.contains_key(s));
            super::super::operandos::operandos(i, &mut |o| {
                endereco |=
                    matches!(o, Operand::Constant(Constant::Funcao(s)) if area.contains_key(s));
            });
        }
        super::super::operandos::operandos_do_terminador(&b.terminator, &mut |o| {
            endereco |= matches!(o, Operand::Constant(Constant::Funcao(s)) if area.contains_key(s));
        });
        if endereco {
            return Err(format!("endereço da área global publicado em {}", f.symbol));
        }
    }
    for (_, i, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::LoadGlobal { simbolo, .. } | Instruction::StoreGlobal { simbolo, .. }
            | Instruction::ArcLoadStrong { slot: SlotForte::Global { simbolo } }
            | Instruction::ArcStoreStrong { slot: SlotForte::Global { simbolo }, .. } =
            i
        {
            *simbolo = area
                .get(simbolo)
                .ok_or_else(|| format!("global fora da área local: {simbolo}"))?
                .clone();
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn slots_compartilhados_entre_modulos_publicam_abi_e_contratos_juntos() {
        let mut origem = super::super::slots_vidas::testes::origem();
        origem.functions.pop();
        origem.functions[1].blocks[0].instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        let folha = origem.functions.pop().unwrap();
        let mut biblioteca = Module::new();
        biblioteca.memoria_arc = true;
        biblioteca.functions.push(folha);
        let mut ms = [origem, biblioteca];
        let mut ps: Vec<HashMap<String, PlanoFuncaoDart>> = ms.iter().map(|m| {
            m.functions.iter().map(|f| (f.symbol.clone(), PlanoFuncaoDart::default())).collect()
        }).collect();
        preparar_arc_modulos_dart(&mut ms, &mut ps).unwrap();
        assert_eq!(ms[1].functions[0].symbol, "a$arcslots_v1");
        assert!(!ps[1].contains_key("a"));
        assert_eq!(ms[1].functions[0].params.len(), 4);
        assert!(ms[0].functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| {
            matches!(i, Instruction::CallStatic { symbol, args, .. }
                if symbol == "a$arcslots_v1" && args.len() == 4)
        }));
        for (m, planos) in ms.iter().zip(&ps) {
            for f in &m.functions {
                let p = &planos[&f.symbol];
                assert!(p.tokens.quadros_compartilhados.is_some());
                verificar_contrato_funcao_dart(f, &p.classes, &p.tokens, &p.tabelas, &p.escopos).unwrap();
            }
        }
    }

    #[test]
    fn variante_falivel_propaga_cleanup_ao_simbolo_publico_e_chamador() {
        for retoma in [false, true] {
            let (mut ms, mut ps) = entrada();
            let mut copia = Module::new();
            copia.memoria_arc = true;
            copia.functions.push(ms[1].functions[0].clone());
            copia.retornos_ref_dart.insert("biblioteca".into());
            ms.push(copia);
            ps.push(ps[1].clone());
            for m in &mut ms {
                m.modo_sdk = true;
            }
            for m in &mut ms[1..] {
                m.funcoes_compartilhadas_arc.insert("biblioteca".into());
            }
            // Somente a segunda definição confere a pilha no prólogo. O ligador
            // pode escolhê-la; o chamador não pode presumir a primeira, pura.
            ps[2].get_mut("biblioteca").unwrap().tabelas.confere_pilha = true;
            // Exercita as duas formas de saída, independentemente do host da CI.
            // A chamada continua dependendo da união das falhas das variantes.
            let f = &mut ms[0].functions[0];
            f.blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            };
            for id in [1, 2] {
                f.blocks.push(BasicBlock {
                    id: BlockId(id),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                });
            }
            let t = &mut ps[0].get_mut("programa").unwrap().tabelas;
            t.invocacoes.insert(ValueId(1), BlockId(1));
            t.pousos.insert(BlockId(1));
            t.saidas.insert(
                BlockId(1),
                if retoma {
                    SaidaPorExcecao::Retoma
                } else {
                    SaidaPorExcecao::Lanca
                },
            );
            preparar_arc_modulos_dart(&mut ms, &mut ps).unwrap();
            let p = &ps[0]["programa"];
            assert!(p.tokens.instrucoes[&ValueId(1)].pode_falhar);
            let pouso = p.tabelas.invocacoes[&ValueId(1)];
            assert_eq!(p.owners_no_pouso[&pouso], vec![ValueId(0)]);
            if p.tabelas.saidas.get(&pouso) == Some(&SaidaPorExcecao::Retoma) {
                // Itanium executa a limpeza explícita antes do resume. O certificado
                // adicional só atende pousos que o emissor ainda precisa limpar.
                assert!(p.tabelas.cleanup_estrangeiro.is_none());
                let bloco = ms[0].functions[0]
                    .blocks
                    .iter()
                    .find(|b| b.id == pouso)
                    .unwrap();
                assert!(bloco.instructions.iter().any(|(_, i, _)| matches!(
                    i,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(0))
                    }
                )));
            } else {
                p.tabelas
                    .cleanup_estrangeiro
                    .as_ref()
                    .unwrap()
                    .conferir(&ms[0].functions[0], &p.tabelas)
                    .unwrap();
            }
            assert!(
                ms[1].functions[0]
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
            );
            assert!(
                ms[2].functions[0]
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
            );
        }
    }

    #[test]
    fn caches_iguais_em_areas_distintas_preservam_corpos_e_nomes_fisicos() {
        let mut ms = Vec::new();
        for receita in ["C201", "C350"] {
            let mut m = Module::new();
            m.memoria_arc = true;
            m.modo_sdk = true;
            m.globais.push((0, Type::I64, "cache".into()));
            m.caches_rti_arc.insert("cache".into());
            m.funcoes_compartilhadas_arc.insert("getter".into());
            m.functions.push(Function {
                symbol: "getter".into(),
                name: "getter".into(),
                params: vec![],
                return_ty: Type::I64,
                depuracao: None,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::Const(Constant::String(receita.into())),
                            Type::Ref,
                        ),
                        (
                            ValueId(1),
                            Instruction::CallRuntime {
                                name: "dartforge_rti_receita".into(),
                                args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(2),
                            Instruction::StoreGlobal {
                                simbolo: "cache".into(),
                                val: Operand::Val(ValueId(1)),
                                ty: Type::I64,
                                raiz: None,
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(3),
                            Instruction::LoadGlobal {
                                simbolo: "cache".into(),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
                }],
            });
            ms.push(m);
        }
        let mut ps = vec![HashMap::from([("getter".into(), PlanoFuncaoDart::default())]); 2];
        let originais: Vec<_> = ms.iter().map(|m| format!("{m:?}")).collect();
        preparar_arc_modulos_dart(&mut ms, &mut ps).unwrap();
        // Esta prova é de ownership/ABI, não de igualdade dos valores ODR.
        for (m, original) in ms.iter().zip(originais) {
            assert_eq!(format!("{m:?}"), original);
        }
        for p in &ps {
            assert_eq!(p["getter"].classes[&ValueId(3)], Ownership::Trivial);
        }
        ms[1].functions[0].blocks[0].instructions[2].1 = Instruction::StoreGlobal {
            simbolo: "cache".into(),
            val: Operand::Val(ValueId(0)),
            ty: Type::I64,
            raiz: None,
        };
        let antes = format!("{ms:?}/{ps:?}");
        assert!(preparar_arc_modulos_dart(&mut ms, &mut ps).is_err());
        assert_eq!(format!("{ms:?}/{ps:?}"), antes);
    }

    #[test]
    fn compartilhamento_exige_declaracao_corpo_contratos_e_dependencias_compativeis() {
        for caso in 0..11 {
            let (mut ms, mut ps) = entrada();
            let mut copia = Module::new();
            copia.memoria_arc = true;
            copia.functions.push(ms[1].functions[0].clone());
            copia.retornos_ref_dart.insert("biblioteca".into());
            ms.push(copia);
            ps.push(ps[1].clone());
            for m in &mut ms {
                m.modo_sdk = true;
                if m.functions[0].symbol == "biblioteca" {
                    m.funcoes_compartilhadas_arc.insert("biblioteca".into());
                }
            }
            match caso {
                1 => ms[2].funcoes_compartilhadas_arc.clear(),
                2 => ms[1].modo_sdk = false,
                3 => {
                    ms[2].functions[0].blocks[0].terminator =
                        Terminator::Return(Some(Operand::Constant(Constant::Int(42))))
                }
                4 => ps[2].get_mut("biblioteca").unwrap().tokens.retorno = RetornoTokens::Borrowed,
                5 => {
                    for m in &mut ms[1..] {
                        m.functions[0].blocks[0].instructions.push((
                            ValueId(1),
                            Instruction::LoadGlobal {
                                simbolo: "cache".into(),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ));
                    }
                }
                6 => {
                    for m in &mut ms {
                        m.memoria_arc = false;
                        m.funcoes_compartilhadas_arc.insert("obsoleto".into());
                    }
                }
                7 => {
                    ms[2].funcoes_compartilhadas_arc.insert("obsoleto".into());
                }
                8 => ms[2].retornos_ref_dart.clear(),
                9 => {
                    ps[2]
                        .get_mut("biblioteca")
                        .unwrap()
                        .classes
                        .insert(ValueId(0), Ownership::Trivial);
                }
                10 => {
                    for m in &mut ms {
                        m.excecoes_por_tabelas = true;
                        m.tabelas.push(TabelasDaFuncao::default());
                    }
                }
                _ => {}
            }
            let antes = format!("{ms:?}/{ps:?}");
            let r = preparar_arc_modulos_dart(&mut ms, &mut ps);
            if matches!(caso, 0 | 6 | 10) {
                assert_eq!(r.unwrap(), if caso == 6 { (0, 0) } else { (2, 2) });
                if caso == 6 {
                    assert_eq!(format!("{ms:?}/{ps:?}"), antes);
                    continue;
                }
                assert_eq!(
                    format!("{:?}", ms[1].functions),
                    format!("{:?}", ms[2].functions)
                );
                assert_eq!(ps[1], ps[2]);
                let depois = format!("{ms:?}/{ps:?}");
                assert_eq!(preparar_arc_modulos_dart(&mut ms, &mut ps).unwrap(), (0, 0));
                assert_eq!(format!("{ms:?}/{ps:?}"), depois);
            } else {
                assert!(r.is_err(), "caso {caso}: {r:?}");
                assert_eq!(format!("{ms:?}/{ps:?}"), antes, "caso {caso}");
            }
        }
    }

    #[test]
    fn ligacao_arc_usa_declaracao_em_vez_do_nome_do_simbolo() {
        for arc in [false, true] {
            let mut m = Module::new();
            m.modo_sdk = true;
            m.memoria_arc = arc;
            for s in ["dfc.nome_nao_certifica", "nome_sem_prefixo"] {
                m.functions.push(Function {
                    symbol: s.into(),
                    name: s.into(),
                    params: vec![],
                    return_ty: Type::Void,
                    depuracao: None,
                    blocks: vec![BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::Return(None),
                    }],
                });
            }
            m.funcoes_compartilhadas_arc
                .insert("nome_sem_prefixo".into());
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert_eq!(
                ir.contains("define linkonce_odr void @nome_sem_prefixo("),
                arc,
                "{ir}"
            );
            assert_eq!(
                ir.contains("define linkonce_odr void @dfc.nome_nao_certifica("),
                !arc,
                "{ir}"
            );
        }
    }

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
            if matches!(caso, 0 | 7 | 8 | 10 | 13 | 15 | 16) {
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
