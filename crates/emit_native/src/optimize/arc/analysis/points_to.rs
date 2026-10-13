//! Solver de inclusão por campo, sem atualização forte (§28.2).
//! Consome restrições explícitas, ainda sem extração HIR/resumos SDK.
//! Ausência de uma restrição não certifica ausência de efeitos do programa.

use super::model::{PointSet, AbstractNode};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

/// Campo nominal de um layout; índices iguais de layouts distintos não coincidem.
/// Versão/validação de layout pertence ao produtor das restrições.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::points_to::CampoArc;
/// let campo = CampoArc { layout: "No:v1".into(), posicao: 0 };
/// assert_ne!(campo.layout, "Outra:v1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArcField {
    /// Identidade nominal e versão do esquema físico/semântico.
    pub layout: String,
    /// Posição no esquema, não um índice dinâmico sem prova de limite.
    pub position: u32,
}

/// Restrição de inclusão de aliases. Variáveis são índices locais do solver,
/// não identidades persistentes de sítio. O produtor deve registrar valores
/// iniciais/externos, todas as escritas e desconhecimentos relevantes.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::points_to::Restricao;
/// let copia = Restricao::Copiar { origem: 0, destino: 1 };
/// assert!(matches!(copia, Restricao::Copiar { .. }));
/// ```
#[derive(Debug, Clone)]
pub enum Restricao {
    /// Inclui uma origem nominal possível na variável.
    Seed { dest: usize, no: AbstractNode },
    /// Valor opaco/informação invalidada, nunca ausência de aliases.
    Unknown { dest: usize },
    /// Copy, movimento, cast de referência ou uma entrada de Phi.
    Copy { origin: usize, dest: usize },
    /// Inclui destinos em todos os receivers possíveis; nunca apaga arestas.
    Write {
        object: usize,
        field: ArcField,
        value: usize,
    },
    /// Inclui todos os destinos possíveis do campo em cada receiver.
    Read {
        object: usize,
        field: ArcField,
        dest: usize,
    },
    /// Leitura por índice físico, selecionando o campo nominal de cada nó.
    /// Esquemas são premissas locais do produtor. Receiver sem esquema/índice
    /// ou vazio ao fim da solução exige topo e invalida os campos conhecidos:
    /// o caminho de erro/helper ainda não tem resumo de heap.
    ReadBySchema {
        object: usize,
        schemas: Arc<BTreeMap<AbstractNode, Vec<ArcField>>>,
        index: usize,
        dest: usize,
    },
    /// Escrita fraca por esquema; nunca remove os destinos anteriores.
    /// Fronteira não coberta invalida todos os campos fornecidos nos esquemas,
    /// inclusive objetos descobertos depois, sem supor receiver singleton.
    WriteBySchema {
        object: usize,
        schemas: Arc<BTreeMap<AbstractNode, Vec<ArcField>>>,
        index: usize,
        value: usize,
    },
    /// Escritor opaco pode tocar qualquer receiver desse campo/layout.
    /// Não torna opacos campos de outros layouts; efeitos adicionais exigem
    /// outras restrições, inclusive quando há reentrada/transitividade.
    UnknownField { field: ArcField },
}

/// Ponto fixo das restrições fornecidas, sem certificado de política.
/// Não representa o heap do programa enquanto o produtor não cobrir todas
/// as operações e instanciar corretamente placeholders dos resumos.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::points_to::{resolver, Restricao};
/// let r = resolver(1, 8, &[Restricao::Desconhecer { destino: 0 }])?;
/// assert!(r.variavel(0).unwrap().nos().is_none());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct PointsToResult {
    variables: Vec<PointSet>,
    fields: BTreeMap<(AbstractNode, ArcField), PointSet>,
    opaque_fields: BTreeSet<ArcField>,
}

impl PointsToResult {
    /// Consulta uma variável; índice inexistente devolve `None`, sem fato vazio.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analise::points_to::resolver;
    /// assert!(resolver(0, 8, &[])?.variavel(0).is_none());
    /// # Ok::<(), String>(())
    /// ```
    pub fn variable(&self, id: usize) -> Option<&PointSet> {
        self.variables.get(id)
    }

    /// Consulta o campo materializado, ou `None` para campo ausente/opaco.
    /// `None` nunca certifica um campo vazio: efeitos/valores iniciais ausentes
    /// precisam ser fornecidos pelo produtor antes de usar o resultado.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analise::{modelo::NoAbstrato, points_to::*};
    /// let r = resolver(0, 8, &[])?;
    /// assert!(r.campo(&NoAbstrato::Global("x".into()), &CampoArc { layout: "L".into(), posicao: 0 }).is_none());
    /// # Ok::<(), String>(())
    /// ```
    pub fn field(&self, no: &AbstractNode, field: &ArcField) -> Option<&PointSet> {
        if self.opaque_fields.contains(field) {
            None
        } else {
            self.fields.get(&(no.clone(), field.clone()))
        }
    }
}

/// Resolve restrições monotônicas por worklist com dependências de variáveis/campos.
/// Escrita por receiver desconhecido torna o campo opaco para todos os nós,
/// inclusive nós descobertos depois. Budget é por conjunto; excedê-lo propaga
/// topo a leitores/cópias. A fila só agenda dependentes de fatos alterados.
/// Variáveis/campos começam no fundo do sistema de inclusão: o produtor deve
/// fornecer fatos de inicialização/externs; isso não infere zero inicial do heap.
///
/// # Erros
/// Índice de variável fora do domínio declarado ou posições incompatíveis
/// no esquema de um acesso selecionado. Validação precede a solução.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::{modelo::NoAbstrato, points_to::*};
/// let r = resolver(2, 4, &[
///     Restricao::Copiar { origem: 0, destino: 1 },
///     Restricao::Semear { destino: 0, no: NoAbstrato::Global("raiz".into()) },
/// ])?;
/// assert_eq!(r.variavel(0), r.variavel(1));
/// # Ok::<(), String>(())
/// ```
pub fn resolver(
    n: usize,
    limit: usize,
    constraints: &[Restricao],
) -> Result<PointsToResult, String> {
    let mut dependents = vec![Vec::new(); n];
    let mut readers: BTreeMap<ArcField, Vec<usize>> = BTreeMap::new();
    for (id, r) in constraints.iter().enumerate() {
        let (uses, dest) = match r {
            Restricao::Seed { dest, .. } | Restricao::Unknown { dest } => {
                (vec![], Some(*dest))
            }
            Restricao::Copy { origin, dest } => (vec![*origin], Some(*dest)),
            Restricao::Write { object, value, .. }
            | Restricao::WriteBySchema { object, value, .. } => (vec![*object, *value], None),
            Restricao::Read {
                object,
                field,
                dest,
            } => {
                readers.entry(field.clone()).or_default().push(id);
                (vec![*object], Some(*dest))
            }
            Restricao::UnknownField { .. } => (vec![], None),
            Restricao::ReadBySchema {
                object,
                schemas,
                index,
                dest,
            } => {
                for field in schemas.values().filter_map(|cs| cs.get(*index)) {
                    readers.entry(field.clone()).or_default().push(id);
                }
                (vec![*object], Some(*dest))
            }
        };
        if let Restricao::ReadBySchema { schemas, .. }
        | Restricao::WriteBySchema { schemas, .. } = r
            && schemas
                .values()
                .any(|cs| cs.iter().enumerate().any(|(i, c)| c.position as usize != i))
        {
            return Err(format!("points-to: esquema incompatível na restrição {id}"));
        }
        if uses.iter().chain(dest.iter()).any(|&v| v >= n) {
            return Err(format!(
                "points-to: variável fora do domínio na restrição {id}"
            ));
        }
        for v in uses {
            dependents[v].push(id);
        }
    }
    let mut result = PointsToResult {
        variables: vec![PointSet::empty(limit); n],
        fields: BTreeMap::new(),
        opaque_fields: BTreeSet::new(),
    };
    let mut queue: VecDeque<_> = (0..constraints.len()).collect();
    let mut scheduled = vec![true; constraints.len()];
    loop {
        while let Some(id) = queue.pop_front() {
            scheduled[id] = false;
            let r = &constraints[id];
            let mut changed_variable = None;
            let mut changed_fields = BTreeSet::new();
            match r {
                Restricao::Seed { dest, no } => {
                    if result.variables[*dest].include(no.clone()) {
                        changed_variable = Some(*dest);
                    }
                }
                Restricao::Unknown { dest } => {
                    if result.variables[*dest].join_set(&PointSet::unknown(limit)) {
                        changed_variable = Some(*dest);
                    }
                }
                Restricao::Copy { origin, dest } => {
                    let origin = result.variables[*origin].clone();
                    if result.variables[*dest].join_set(&origin) {
                        changed_variable = Some(*dest);
                    }
                }
                Restricao::Write {
                    object,
                    field,
                    value,
                } => {
                    if let Some(nos) = result.variables[*object].nos() {
                        let value = &result.variables[*value];
                        for no in nos {
                            let dests = result
                                .fields
                                .entry((no.clone(), field.clone()))
                                .or_insert_with(|| PointSet::empty(limit));
                            if dests.join_set(value) {
                                changed_fields.insert(field);
                            }
                        }
                    } else if result.opaque_fields.insert(field.clone()) {
                        changed_fields.insert(field);
                    }
                }
                Restricao::UnknownField { field } => {
                    if result.opaque_fields.insert(field.clone()) {
                        changed_fields.insert(field);
                    }
                }
                Restricao::Read {
                    object,
                    field,
                    dest,
                } => {
                    let mut value = PointSet::empty(limit);
                    if let Some(nos) = result.variables[*object]
                        .nos()
                        .filter(|_| !result.opaque_fields.contains(field))
                    {
                        for no in nos {
                            if let Some(c) = result.fields.get(&(no.clone(), field.clone())) {
                                value.join_set(c);
                            }
                        }
                    } else {
                        value = PointSet::unknown(limit);
                    }
                    if result.variables[*dest].join_set(&value) {
                        changed_variable = Some(*dest);
                    }
                }
                Restricao::ReadBySchema {
                    object,
                    schemas,
                    index,
                    dest,
                }
                | Restricao::WriteBySchema {
                    object,
                    schemas,
                    index,
                    value: dest,
                } => {
                    let nos = result.variables[*object].nos();
                    let boundary = nos.is_none_or(|ns| {
                        ns.iter()
                            .any(|no| schemas.get(no).and_then(|cs| cs.get(*index)).is_none())
                    });
                    if boundary {
                        for field in schemas.values().flatten() {
                            if result.opaque_fields.insert(field.clone()) {
                                changed_fields.insert(field);
                            }
                        }
                    }
                    if matches!(r, Restricao::ReadBySchema { .. }) {
                        let mut value = if boundary {
                            PointSet::unknown(limit)
                        } else {
                            PointSet::empty(limit)
                        };
                        if let Some(nos) = nos {
                            for no in nos {
                                if let Some(field) = schemas.get(no).and_then(|cs| cs.get(*index))
                                {
                                    if result.opaque_fields.contains(field) {
                                        value.join_set(&PointSet::unknown(limit));
                                    } else if let Some(c) =
                                        result.fields.get(&(no.clone(), field.clone()))
                                    {
                                        value.join_set(c);
                                    }
                                }
                            }
                        }
                        if result.variables[*dest].join_set(&value) {
                            changed_variable = Some(*dest);
                        }
                    } else if !boundary && let Some(nos) = nos {
                        let value = &result.variables[*dest];
                        for no in nos {
                            let field = &schemas[no][*index];
                            if result
                                .fields
                                .entry((no.clone(), field.clone()))
                                .or_insert_with(|| PointSet::empty(limit))
                                .join_set(value)
                            {
                                changed_fields.insert(field);
                            }
                        }
                    }
                }
            }
            let events = changed_variable
                .into_iter()
                .flat_map(|v| dependents[v].iter())
                .chain(
                    changed_fields
                        .into_iter()
                        .flat_map(|c| readers.get(c).into_iter().flatten()),
                );
            for &next_item in events {
                if !scheduled[next_item] {
                    scheduled[next_item] = true;
                    queue.push_back(next_item);
                }
            }
        }
        // Bottom intermediário pode receber aliases tardios. Só depois de
        // esvaziar a fila, receiver ainda vazio deixa de ser acesso coberto:
        // null/ausência de objeto não prova ausência de efeitos do helper.
        let mut events: BTreeSet<usize> = BTreeSet::new();
        for r in constraints {
            let (object, schemas, dest) = match r {
                Restricao::ReadBySchema {
                    object,
                    schemas,
                    dest,
                    ..
                } => (*object, schemas, Some(*dest)),
                Restricao::WriteBySchema {
                    object, schemas, ..
                } => (*object, schemas, None),
                _ => continue,
            };
            if result.variables[object]
                .nos()
                .is_some_and(|ns| ns.is_empty())
            {
                if let Some(dest) = dest
                    && result.variables[dest].join_set(&PointSet::unknown(limit))
                {
                    events.extend(dependents[dest].iter().copied());
                }
                for field in schemas.values().flatten() {
                    if result.opaque_fields.insert(field.clone()) {
                        events.extend(readers.get(field).into_iter().flatten().copied());
                    }
                }
            }
        }
        if events.is_empty() {
            break;
        }
        for id in events {
            if !scheduled[id] {
                scheduled[id] = true;
                queue.push_back(id);
            }
        }
    }
    verify(n, limit, constraints, &result)?;
    Ok(result)
}

/// Confere independentemente se a solução cobre todas as restrições fornecidas.
/// Não executa a worklist nem exige o menor ponto fixo: sobreaproximações são
/// válidas, incluindo topo. Confere índices, budgets e inclusão de cada aresta.
/// Não prova que o produtor cobriu a HIR, inicializações, observadores ou SDK;
/// não é certificado de vida, singleton ou política ARC.
///
/// # Erros
/// Domínio/budget incompatível, índice inválido ou restrição não satisfeita.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::{modelo::*, points_to::*};
/// let rs = vec![Restricao::Semear { destino: 0, no: NoAbstrato::Global("g".into()) },
///     Restricao::Copiar { origem: 0, destino: 1 }];
/// let r = resolver(2, 8, &rs)?;
/// verificar(2, 8, &rs, &r)?;
/// assert!(verificar(3, 8, &rs, &r).is_err());
/// # Ok::<(), String>(())
/// ```
pub fn verify(
    n: usize,
    limit: usize,
    constraints: &[Restricao],
    r: &PointsToResult,
) -> Result<(), String> {
    if r.variables.len() != n {
        return Err("verificador points-to: domínio incompatível".into());
    }
    if r.variables
        .iter()
        .chain(r.fields.values())
        .any(|p| p.internal_limit() != limit || p.nos().is_some_and(|nos| nos.len() > limit))
    {
        return Err("verificador points-to: budget incompatível".into());
    }
    for (id, constraint) in constraints.iter().enumerate() {
        let variable = |v: usize| {
            r.variables
                .get(v)
                .ok_or_else(|| format!("verificador points-to: índice na restrição {id}"))
        };
        let valid = match constraint {
            Restricao::Seed { dest, no } => {
                variable(*dest)?.nos().is_none_or(|nos| nos.contains(no))
            }
            Restricao::Unknown { dest } => variable(*dest)?.nos().is_none(),
            Restricao::Copy { origin, dest } => {
                includes(variable(*dest)?, variable(*origin)?)
            }
            Restricao::UnknownField { field } => r.opaque_fields.contains(field),
            Restricao::Write {
                object,
                field,
                value,
            } => {
                let receiver = variable(*object)?;
                let value = variable(*value)?;
                match receiver.nos() {
                    None => r.opaque_fields.contains(field),
                    Some(nos) => {
                        r.opaque_fields.contains(field)
                            || nos.iter().all(|no| {
                                r.fields
                                    .get(&(no.clone(), field.clone()))
                                    .is_some_and(|p| includes(p, value))
                            })
                    }
                }
            }
            Restricao::Read {
                object,
                field,
                dest,
            } => {
                let receiver = variable(*object)?;
                let dest = variable(*dest)?;
                if r.opaque_fields.contains(field) || receiver.nos().is_none() {
                    dest.nos().is_none()
                } else {
                    receiver.nos().unwrap().iter().all(|no| {
                        // Ausente é bottom só no sistema de restrições; não
                        // prova inicialização vazia de um campo do heap real.
                        r.fields
                            .get(&(no.clone(), field.clone()))
                            .is_none_or(|p| includes(dest, p))
                    })
                }
            }
            Restricao::ReadBySchema {
                object,
                schemas,
                index,
                dest,
            }
            | Restricao::WriteBySchema {
                object,
                schemas,
                index,
                value: dest,
            } => {
                if schemas
                    .values()
                    .any(|cs| cs.iter().enumerate().any(|(i, c)| c.position as usize != i))
                {
                    return Err(format!("verificador points-to: esquema na restrição {id}"));
                }
                let receiver = variable(*object)?;
                let value = variable(*dest)?;
                let boundary = receiver.nos().is_none_or(|ns| {
                    ns.is_empty()
                        || ns
                            .iter()
                            .any(|no| schemas.get(no).and_then(|cs| cs.get(*index)).is_none())
                });
                if boundary {
                    schemas
                        .values()
                        .flatten()
                        .all(|c| r.opaque_fields.contains(c))
                        && (!matches!(constraint, Restricao::ReadBySchema { .. })
                            || value.nos().is_none())
                } else {
                    receiver.nos().unwrap().iter().all(|no| {
                        let field = &schemas[no][*index];
                        if matches!(constraint, Restricao::ReadBySchema { .. }) {
                            if r.opaque_fields.contains(field) {
                                value.nos().is_none()
                            } else {
                                r.fields
                                    .get(&(no.clone(), field.clone()))
                                    .is_none_or(|c| includes(value, c))
                            }
                        } else {
                            r.opaque_fields.contains(field)
                                || r.fields
                                    .get(&(no.clone(), field.clone()))
                                    .is_some_and(|c| includes(c, value))
                        }
                    })
                }
            }
        };
        if !valid {
            return Err(format!(
                "verificador points-to: restrição {id} não satisfeita"
            ));
        }
    }
    Ok(())
}

fn includes(dest: &PointSet, origin: &PointSet) -> bool {
    match (dest.nos(), origin.nos()) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(d), Some(o)) => o.is_subset(d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_reads_match_walks_in_343_graphs() {
        let schemas = Arc::new(
            (0..3)
                .map(|i| (no(&format!("n{i}")), vec![field(&format!("L{i}"))]))
                .collect::<BTreeMap<_, _>>(),
        );
        let mut checked_set = 0;
        for mask in 0..512usize {
            // Cada nó tem saída: os testes de fronteira tratam separadamente
            // receivers null/vazios, sem supor que sejam reads puros.
            if (0..3).any(|v| mask & (7 << (3 * v)) == 0) {
                continue;
            }
            checked_set += 1;
            let mut rs = Vec::new();
            for step in 0..7 {
                rs.push(Restricao::ReadBySchema {
                    object: if step == 0 { 0 } else { 2 + step },
                    schemas: schemas.clone(),
                    index: 0,
                    dest: 3 + step,
                });
            }
            for v in 0..3 {
                for w in 0..3 {
                    if mask & (1 << (3 * v + w)) != 0 {
                        rs.push(Restricao::Write {
                            object: v,
                            field: field(&format!("L{v}")),
                            value: w,
                        });
                    }
                }
                rs.push(Restricao::Seed {
                    dest: v,
                    no: no(&format!("n{v}")),
                });
            }
            // Oráculo de caminhadas de comprimento exato no grafo original;
            // não usa restrições, fila, esquema do solver ou seu verificador.
            let mut current_ones = BTreeSet::from([0]);
            let mut expected_set = Vec::new();
            for _ in 0..7 {
                current_ones = (0..3)
                    .filter(|&w| current_ones.iter().any(|&v| mask & (1 << (3 * v + w)) != 0))
                    .collect();
                expected_set.push(
                    current_ones
                        .iter()
                        .map(|i| no(&format!("n{i}")))
                        .collect::<BTreeSet<_>>(),
                );
            }
            for _ in 0..2 {
                let r = resolver(10, 8, &rs).unwrap();
                for (step, expected) in expected_set.iter().enumerate() {
                    assert_eq!(
                        r.variable(3 + step).unwrap().nos().unwrap(),
                        expected,
                        "grafo {mask}, passo {step}"
                    );
                }
                assert!(r.opaque_fields.is_empty());
                rs.reverse();
            }
        }
        assert_eq!(checked_set, 343);
    }

    #[test]
    fn schema_selection_preserves_layouts_weak_write_and_late_aliases() {
        let schemas = Arc::new(BTreeMap::from([
            (no("a"), vec![field("A")]),
            (no("b"), vec![field("B")]),
        ]));
        let mut rs = vec![
            Restricao::ReadBySchema {
                object: 0,
                schemas: schemas.clone(),
                index: 0,
                dest: 3,
            },
            Restricao::ReadBySchema {
                object: 6,
                schemas: schemas.clone(),
                index: 0,
                dest: 7,
            },
            Restricao::WriteBySchema {
                object: 0,
                schemas,
                index: 0,
                value: 4,
            },
            Restricao::Write {
                object: 5,
                field: field("A"),
                value: 1,
            },
            Restricao::Write {
                object: 6,
                field: field("B"),
                value: 2,
            },
            // Mesmo índice, outro layout: esta aresta não é lida em b.B.
            Restricao::Write {
                object: 6,
                field: field("A"),
                value: 1,
            },
            Restricao::Seed {
                dest: 5,
                no: no("a"),
            },
            Restricao::Seed {
                dest: 6,
                no: no("b"),
            },
            Restricao::Copy {
                origin: 5,
                dest: 0,
            },
            Restricao::Copy {
                origin: 6,
                dest: 0,
            },
            Restricao::Seed {
                dest: 1,
                no: no("x"),
            },
            Restricao::Seed {
                dest: 2,
                no: no("y"),
            },
            Restricao::Seed {
                dest: 4,
                no: no("z"),
            },
        ];
        for _ in 0..2 {
            let mut r = resolver(8, 8, &rs).unwrap();
            assert_eq!(
                r.variable(3).unwrap().nos().unwrap(),
                &BTreeSet::from([no("x"), no("y"), no("z")])
            );
            assert_eq!(
                r.variable(7).unwrap().nos().unwrap(),
                &BTreeSet::from([no("y"), no("z")])
            );
            assert!(r.opaque_fields.is_empty());
            r.variables[3] = PointSet::empty(8);
            assert!(verify(8, 8, &rs, &r).is_err());
            r = resolver(8, 8, &rs).unwrap();
            r.fields
                .insert((no("b"), field("B")), PointSet::empty(8));
            assert!(verify(8, 8, &rs, &r).is_err());
            rs.reverse();
        }
    }

    #[test]
    fn schema_selection_closes_boundaries_and_verifier_rejects_erased_opacity() {
        let schemas = Arc::new(BTreeMap::from([(no("a"), vec![field("A")])]));
        for case in 0..5 {
            let mut rs = vec![Restricao::ReadBySchema {
                object: 0,
                schemas: schemas.clone(),
                index: if case == 3 { 1 } else { 0 },
                dest: 1,
            }];
            match case {
                0 => {} // Receiver vazio só é fechado após os aliases convergirem.
                1 => rs.push(Restricao::Unknown { dest: 0 }),
                2 => rs.push(Restricao::Seed {
                    dest: 0,
                    no: no("sem esquema"),
                }),
                3 | 4 => rs.push(Restricao::Seed {
                    dest: 0,
                    no: no("a"),
                }),
                _ => unreachable!(),
            }
            let limit = if case == 4 { 0 } else { 8 };
            let mut r = resolver(2, limit, &rs).unwrap();
            assert!(r.variable(1).unwrap().nos().is_none(), "caso {case}");
            assert!(r.opaque_fields.contains(&field("A")));
            r.opaque_fields.clear();
            assert!(verify(2, limit, &rs, &r).is_err());
        }
        let rs = [Restricao::WriteBySchema {
            object: 0,
            schemas,
            index: 0,
            value: 1,
        }];
        let mut r = resolver(2, 8, &rs).unwrap();
        assert!(r.opaque_fields.contains(&field("A")));
        r.opaque_fields.clear();
        assert!(verify(2, 8, &rs, &r).is_err());
        let invalid = Arc::new(BTreeMap::from([(
            no("a"),
            vec![ArcField {
                layout: "A".into(),
                position: 9,
            }],
        )]));
        assert!(
            resolver(
                2,
                8,
                &[Restricao::ReadBySchema {
                    object: 0,
                    schemas: invalid,
                    index: 0,
                    dest: 1
                }]
            )
            .is_err()
        );
    }
    #[test]
    fn verifier_rejects_erased_truncation_copy_and_unknown() {
        let rs = vec![
            Restricao::Seed {
                dest: 0,
                no: no("a"),
            },
            Restricao::Copy {
                origin: 0,
                dest: 1,
            },
            Restricao::Unknown { dest: 2 },
        ];
        let mut r = resolver(3, 8, &rs).unwrap();
        r.variables[1] = PointSet::empty(8);
        assert!(verify(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        r.variables[0] = PointSet::empty(8);
        assert!(verify(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        r.variables[2] = PointSet::empty(8);
        assert!(verify(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        assert!(verify(3, 7, &rs, &r).is_err());
        assert!(verify(2, 8, &rs, &r).is_err());
        assert!(
            verify(
                3,
                8,
                &[Restricao::Copy {
                    origin: 99,
                    dest: 0
                }],
                &r
            )
            .is_err()
        );
        // Aceitar sobreaproximação segura sem depender de igualdade ao solver.
        r.variables.fill(PointSet::unknown(8));
        verify(3, 8, &rs, &r).unwrap();
    }

    #[test]
    fn verifier_rejects_omitted_store_read_and_opacity_edges() {
        let rs = vec![
            Restricao::Seed {
                dest: 0,
                no: no("obj"),
            },
            Restricao::Seed {
                dest: 1,
                no: no("filho"),
            },
            Restricao::Write {
                object: 0,
                field: field("L"),
                value: 1,
            },
            Restricao::Read {
                object: 0,
                field: field("L"),
                dest: 2,
            },
        ];
        let mut r = resolver(3, 8, &rs).unwrap();
        r.fields.clear();
        assert!(verify(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        r.fields
            .insert((no("obj"), field("L")), PointSet::empty(8));
        assert!(verify(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        r.variables[2] = PointSet::empty(8);
        assert!(verify(3, 8, &rs, &r).is_err());
        let mut opaque_set = rs;
        opaque_set.push(Restricao::UnknownField { field: field("L") });
        r = resolver(3, 8, &opaque_set).unwrap();
        r.opaque_fields.clear();
        assert!(verify(3, 8, &opaque_set, &r).is_err());
    }

    #[test]
    fn top_receiver_requires_global_opacity_for_future_receivers() {
        let rs = vec![
            Restricao::Unknown { dest: 0 },
            Restricao::Write {
                object: 0,
                field: field("L"),
                value: 1,
            },
            Restricao::Seed {
                dest: 2,
                no: no("tardio"),
            },
            Restricao::Read {
                object: 2,
                field: field("L"),
                dest: 3,
            },
        ];
        let mut r = resolver(4, 8, &rs).unwrap();
        r.opaque_fields.clear();
        assert!(verify(4, 8, &rs, &r).is_err());
    }

    #[test]
    fn inclusion_matches_transitive_closure_in_all_three_variable_graphs() {
        for mask in 0..512 {
            let mut rs = Vec::new();
            let mut reachability = [[false; 3]; 3];
            for v in 0..3 {
                reachability[v][v] = true;
                rs.push(Restricao::Seed {
                    dest: v,
                    no: no(&v.to_string()),
                });
                for w in 0..3 {
                    if mask & (1 << (3 * v + w)) != 0 {
                        reachability[v][w] = true;
                        rs.push(Restricao::Copy {
                            origin: v,
                            dest: w,
                        });
                    }
                }
            }
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        reachability[v][w] |= reachability[v][k] && reachability[k][w];
                    }
                }
            }
            for limit in 1..=3 {
                let r = resolver(3, limit, &rs).unwrap();
                for w in 0..3 {
                    let expected: BTreeSet<_> = (0..3)
                        .filter(|&v| reachability[v][w])
                        .map(|v| no(&v.to_string()))
                        .collect();
                    let obtained = r.variable(w).unwrap().nos();
                    if expected.len() > limit {
                        assert!(obtained.is_none());
                    } else {
                        assert_eq!(obtained.unwrap(), &expected, "grafo {mask}");
                    }
                }
            }
        }
    }
    fn no(n: &str) -> AbstractNode {
        AbstractNode::Global(n.into())
    }
    fn field(n: &str) -> ArcField {
        ArcField {
            layout: n.into(),
            position: 0,
        }
    }

    #[test]
    fn fields_and_late_aliases_converge_without_erasing_edges() {
        let mut rs = vec![
            Restricao::Read {
                object: 2,
                field: field("L"),
                dest: 3,
            },
            Restricao::Write {
                object: 0,
                field: field("L"),
                value: 1,
            },
            Restricao::Copy {
                origin: 0,
                dest: 2,
            },
            Restricao::Seed {
                dest: 0,
                no: no("a"),
            },
            Restricao::Seed {
                dest: 1,
                no: no("b"),
            },
            Restricao::Seed {
                dest: 1,
                no: no("c"),
            },
        ];
        let r = resolver(4, 8, &rs).unwrap();
        assert_eq!(r.variable(3), r.variable(1));
        assert_eq!(r.field(&no("a"), &field("L")), r.variable(1));
        rs.reverse();
        let inverse = resolver(4, 8, &rs).unwrap();
        assert_eq!(r.variables, inverse.variables);
        assert_eq!(r.fields, inverse.fields);
    }

    #[test]
    fn opaque_writer_and_budget_invalidate_readers_without_contaminating_other_layout() {
        for opaque in [true, false] {
            let rs = vec![
                Restricao::Read {
                    object: 0,
                    field: field("L"),
                    dest: 3,
                },
                Restricao::Read {
                    object: 0,
                    field: field("Outro"),
                    dest: 4,
                },
                Restricao::Write {
                    object: 0,
                    field: field("Outro"),
                    value: 2,
                },
                Restricao::Write {
                    object: 1,
                    field: field("L"),
                    value: 2,
                },
                Restricao::Seed {
                    dest: 0,
                    no: no("a"),
                },
                Restricao::Seed {
                    dest: 1,
                    no: no("a"),
                },
                Restricao::Seed {
                    dest: 2,
                    no: no("b"),
                },
                if opaque {
                    Restricao::Unknown { dest: 1 }
                } else {
                    Restricao::Seed {
                        dest: 1,
                        no: no("c"),
                    }
                },
            ];
            let r = resolver(5, 1, &rs).unwrap();
            assert!(r.variable(3).unwrap().nos().is_none());
            assert_eq!(r.variable(4), r.variable(2));
            assert!(r.field(&no("a"), &field("L")).is_none());
        }
    }

    #[test]
    fn deep_chain_and_recursive_phi_terminate_by_dependencies() {
        let n = 20_000;
        let mut rs: Vec<_> = (1..n)
            .rev()
            .map(|v| Restricao::Copy {
                origin: v - 1,
                dest: v,
            })
            .collect();
        rs.push(Restricao::Copy {
            origin: n - 1,
            dest: 0,
        });
        rs.push(Restricao::Seed {
            dest: 0,
            no: no("raiz"),
        });
        let r = resolver(n, 1, &rs).unwrap();
        assert!(r.variables.iter().all(|p| p == &r.variables[0]));
        assert_eq!(r.variables[0].nos().unwrap().len(), 1);
    }

    #[test]
    fn invalid_indices_are_errors_and_never_empty_facts() {
        assert!(
            resolver(
                1,
                8,
                &[Restricao::Copy {
                    origin: 1,
                    dest: 0
                }]
            )
            .is_err()
        );
        assert!(
            resolver(
                1,
                8,
                &[Restricao::Read {
                    object: 0,
                    field: field("L"),
                    dest: 1
                }]
            )
            .is_err()
        );
    }
}
