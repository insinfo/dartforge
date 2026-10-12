//! Solver de inclusão por campo, sem atualização forte (§28.2).
//! Consome restrições explícitas, ainda sem extração HIR/resumos SDK.
//! Ausência de uma restrição não certifica ausência de efeitos do programa.

use super::modelo::{ConjuntoPontos, NoAbstrato};
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
pub struct CampoArc {
    /// Identidade nominal e versão do esquema físico/semântico.
    pub layout: String,
    /// Posição no esquema, não um índice dinâmico sem prova de limite.
    pub posicao: u32,
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
    Semear { destino: usize, no: NoAbstrato },
    /// Valor opaco/informação invalidada, nunca ausência de aliases.
    Desconhecer { destino: usize },
    /// Copy, movimento, cast de referência ou uma entrada de Phi.
    Copiar { origem: usize, destino: usize },
    /// Inclui destinos em todos os receivers possíveis; nunca apaga arestas.
    Gravar {
        objeto: usize,
        campo: CampoArc,
        valor: usize,
    },
    /// Inclui todos os destinos possíveis do campo em cada receiver.
    Ler {
        objeto: usize,
        campo: CampoArc,
        destino: usize,
    },
    /// Leitura por índice físico, selecionando o campo nominal de cada nó.
    /// Esquemas são premissas locais do produtor. Receiver sem esquema/índice
    /// ou vazio ao fim da solução exige topo e invalida os campos conhecidos:
    /// o caminho de erro/helper ainda não tem resumo de heap.
    LerPorEsquema {
        objeto: usize,
        esquemas: Arc<BTreeMap<NoAbstrato, Vec<CampoArc>>>,
        indice: usize,
        destino: usize,
    },
    /// Escrita fraca por esquema; nunca remove os destinos anteriores.
    /// Fronteira não coberta invalida todos os campos fornecidos nos esquemas,
    /// inclusive objetos descobertos depois, sem supor receiver singleton.
    GravarPorEsquema {
        objeto: usize,
        esquemas: Arc<BTreeMap<NoAbstrato, Vec<CampoArc>>>,
        indice: usize,
        valor: usize,
    },
    /// Escritor opaco pode tocar qualquer receiver desse campo/layout.
    /// Não torna opacos campos de outros layouts; efeitos adicionais exigem
    /// outras restrições, inclusive quando há reentrada/transitividade.
    DesconhecerCampo { campo: CampoArc },
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
pub struct ResultadoPointsTo {
    variaveis: Vec<ConjuntoPontos>,
    campos: BTreeMap<(NoAbstrato, CampoArc), ConjuntoPontos>,
    campos_opacos: BTreeSet<CampoArc>,
}

impl ResultadoPointsTo {
    /// Consulta uma variável; índice inexistente devolve `None`, sem fato vazio.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analise::points_to::resolver;
    /// assert!(resolver(0, 8, &[])?.variavel(0).is_none());
    /// # Ok::<(), String>(())
    /// ```
    pub fn variavel(&self, id: usize) -> Option<&ConjuntoPontos> {
        self.variaveis.get(id)
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
    pub fn campo(&self, no: &NoAbstrato, campo: &CampoArc) -> Option<&ConjuntoPontos> {
        if self.campos_opacos.contains(campo) {
            None
        } else {
            self.campos.get(&(no.clone(), campo.clone()))
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
    limite: usize,
    restricoes: &[Restricao],
) -> Result<ResultadoPointsTo, String> {
    let mut dependentes = vec![Vec::new(); n];
    let mut leitores: BTreeMap<CampoArc, Vec<usize>> = BTreeMap::new();
    for (id, r) in restricoes.iter().enumerate() {
        let (usos, destino) = match r {
            Restricao::Semear { destino, .. } | Restricao::Desconhecer { destino } => {
                (vec![], Some(*destino))
            }
            Restricao::Copiar { origem, destino } => (vec![*origem], Some(*destino)),
            Restricao::Gravar { objeto, valor, .. }
            | Restricao::GravarPorEsquema { objeto, valor, .. } => (vec![*objeto, *valor], None),
            Restricao::Ler {
                objeto,
                campo,
                destino,
            } => {
                leitores.entry(campo.clone()).or_default().push(id);
                (vec![*objeto], Some(*destino))
            }
            Restricao::DesconhecerCampo { .. } => (vec![], None),
            Restricao::LerPorEsquema {
                objeto,
                esquemas,
                indice,
                destino,
            } => {
                for campo in esquemas.values().filter_map(|cs| cs.get(*indice)) {
                    leitores.entry(campo.clone()).or_default().push(id);
                }
                (vec![*objeto], Some(*destino))
            }
        };
        if let Restricao::LerPorEsquema { esquemas, .. }
        | Restricao::GravarPorEsquema { esquemas, .. } = r
            && esquemas
                .values()
                .any(|cs| cs.iter().enumerate().any(|(i, c)| c.posicao as usize != i))
        {
            return Err(format!("points-to: esquema incompatível na restrição {id}"));
        }
        if usos.iter().chain(destino.iter()).any(|&v| v >= n) {
            return Err(format!(
                "points-to: variável fora do domínio na restrição {id}"
            ));
        }
        for v in usos {
            dependentes[v].push(id);
        }
    }
    let mut resultado = ResultadoPointsTo {
        variaveis: vec![ConjuntoPontos::vazio(limite); n],
        campos: BTreeMap::new(),
        campos_opacos: BTreeSet::new(),
    };
    let mut fila: VecDeque<_> = (0..restricoes.len()).collect();
    let mut agendada = vec![true; restricoes.len()];
    loop {
        while let Some(id) = fila.pop_front() {
            agendada[id] = false;
            let r = &restricoes[id];
            let mut variavel_alterada = None;
            let mut campos_alterados = BTreeSet::new();
            match r {
                Restricao::Semear { destino, no } => {
                    if resultado.variaveis[*destino].incluir(no.clone()) {
                        variavel_alterada = Some(*destino);
                    }
                }
                Restricao::Desconhecer { destino } => {
                    if resultado.variaveis[*destino].unir(&ConjuntoPontos::desconhecido(limite)) {
                        variavel_alterada = Some(*destino);
                    }
                }
                Restricao::Copiar { origem, destino } => {
                    let origem = resultado.variaveis[*origem].clone();
                    if resultado.variaveis[*destino].unir(&origem) {
                        variavel_alterada = Some(*destino);
                    }
                }
                Restricao::Gravar {
                    objeto,
                    campo,
                    valor,
                } => {
                    if let Some(nos) = resultado.variaveis[*objeto].nos() {
                        let valor = &resultado.variaveis[*valor];
                        for no in nos {
                            let destinos = resultado
                                .campos
                                .entry((no.clone(), campo.clone()))
                                .or_insert_with(|| ConjuntoPontos::vazio(limite));
                            if destinos.unir(valor) {
                                campos_alterados.insert(campo);
                            }
                        }
                    } else if resultado.campos_opacos.insert(campo.clone()) {
                        campos_alterados.insert(campo);
                    }
                }
                Restricao::DesconhecerCampo { campo } => {
                    if resultado.campos_opacos.insert(campo.clone()) {
                        campos_alterados.insert(campo);
                    }
                }
                Restricao::Ler {
                    objeto,
                    campo,
                    destino,
                } => {
                    let mut valor = ConjuntoPontos::vazio(limite);
                    if let Some(nos) = resultado.variaveis[*objeto]
                        .nos()
                        .filter(|_| !resultado.campos_opacos.contains(campo))
                    {
                        for no in nos {
                            if let Some(c) = resultado.campos.get(&(no.clone(), campo.clone())) {
                                valor.unir(c);
                            }
                        }
                    } else {
                        valor = ConjuntoPontos::desconhecido(limite);
                    }
                    if resultado.variaveis[*destino].unir(&valor) {
                        variavel_alterada = Some(*destino);
                    }
                }
                Restricao::LerPorEsquema {
                    objeto,
                    esquemas,
                    indice,
                    destino,
                }
                | Restricao::GravarPorEsquema {
                    objeto,
                    esquemas,
                    indice,
                    valor: destino,
                } => {
                    let nos = resultado.variaveis[*objeto].nos();
                    let fronteira = nos.is_none_or(|ns| {
                        ns.iter()
                            .any(|no| esquemas.get(no).and_then(|cs| cs.get(*indice)).is_none())
                    });
                    if fronteira {
                        for campo in esquemas.values().flatten() {
                            if resultado.campos_opacos.insert(campo.clone()) {
                                campos_alterados.insert(campo);
                            }
                        }
                    }
                    if matches!(r, Restricao::LerPorEsquema { .. }) {
                        let mut valor = if fronteira {
                            ConjuntoPontos::desconhecido(limite)
                        } else {
                            ConjuntoPontos::vazio(limite)
                        };
                        if let Some(nos) = nos {
                            for no in nos {
                                if let Some(campo) = esquemas.get(no).and_then(|cs| cs.get(*indice))
                                {
                                    if resultado.campos_opacos.contains(campo) {
                                        valor.unir(&ConjuntoPontos::desconhecido(limite));
                                    } else if let Some(c) =
                                        resultado.campos.get(&(no.clone(), campo.clone()))
                                    {
                                        valor.unir(c);
                                    }
                                }
                            }
                        }
                        if resultado.variaveis[*destino].unir(&valor) {
                            variavel_alterada = Some(*destino);
                        }
                    } else if !fronteira && let Some(nos) = nos {
                        let valor = &resultado.variaveis[*destino];
                        for no in nos {
                            let campo = &esquemas[no][*indice];
                            if resultado
                                .campos
                                .entry((no.clone(), campo.clone()))
                                .or_insert_with(|| ConjuntoPontos::vazio(limite))
                                .unir(valor)
                            {
                                campos_alterados.insert(campo);
                            }
                        }
                    }
                }
            }
            let eventos = variavel_alterada
                .into_iter()
                .flat_map(|v| dependentes[v].iter())
                .chain(
                    campos_alterados
                        .into_iter()
                        .flat_map(|c| leitores.get(c).into_iter().flatten()),
                );
            for &proximo in eventos {
                if !agendada[proximo] {
                    agendada[proximo] = true;
                    fila.push_back(proximo);
                }
            }
        }
        // Bottom intermediário pode receber aliases tardios. Só depois de
        // esvaziar a fila, receiver ainda vazio deixa de ser acesso coberto:
        // null/ausência de objeto não prova ausência de efeitos do helper.
        let mut eventos: BTreeSet<usize> = BTreeSet::new();
        for r in restricoes {
            let (objeto, esquemas, destino) = match r {
                Restricao::LerPorEsquema {
                    objeto,
                    esquemas,
                    destino,
                    ..
                } => (*objeto, esquemas, Some(*destino)),
                Restricao::GravarPorEsquema {
                    objeto, esquemas, ..
                } => (*objeto, esquemas, None),
                _ => continue,
            };
            if resultado.variaveis[objeto]
                .nos()
                .is_some_and(|ns| ns.is_empty())
            {
                if let Some(destino) = destino
                    && resultado.variaveis[destino].unir(&ConjuntoPontos::desconhecido(limite))
                {
                    eventos.extend(dependentes[destino].iter().copied());
                }
                for campo in esquemas.values().flatten() {
                    if resultado.campos_opacos.insert(campo.clone()) {
                        eventos.extend(leitores.get(campo).into_iter().flatten().copied());
                    }
                }
            }
        }
        if eventos.is_empty() {
            break;
        }
        for id in eventos {
            if !agendada[id] {
                agendada[id] = true;
                fila.push_back(id);
            }
        }
    }
    verificar(n, limite, restricoes, &resultado)?;
    Ok(resultado)
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
pub fn verificar(
    n: usize,
    limite: usize,
    restricoes: &[Restricao],
    r: &ResultadoPointsTo,
) -> Result<(), String> {
    if r.variaveis.len() != n {
        return Err("verificador points-to: domínio incompatível".into());
    }
    if r.variaveis
        .iter()
        .chain(r.campos.values())
        .any(|p| p.limite_interno() != limite || p.nos().is_some_and(|nos| nos.len() > limite))
    {
        return Err("verificador points-to: budget incompatível".into());
    }
    for (id, restricao) in restricoes.iter().enumerate() {
        let variavel = |v: usize| {
            r.variaveis
                .get(v)
                .ok_or_else(|| format!("verificador points-to: índice na restrição {id}"))
        };
        let valida = match restricao {
            Restricao::Semear { destino, no } => {
                variavel(*destino)?.nos().is_none_or(|nos| nos.contains(no))
            }
            Restricao::Desconhecer { destino } => variavel(*destino)?.nos().is_none(),
            Restricao::Copiar { origem, destino } => {
                inclui(variavel(*destino)?, variavel(*origem)?)
            }
            Restricao::DesconhecerCampo { campo } => r.campos_opacos.contains(campo),
            Restricao::Gravar {
                objeto,
                campo,
                valor,
            } => {
                let receiver = variavel(*objeto)?;
                let valor = variavel(*valor)?;
                match receiver.nos() {
                    None => r.campos_opacos.contains(campo),
                    Some(nos) => {
                        r.campos_opacos.contains(campo)
                            || nos.iter().all(|no| {
                                r.campos
                                    .get(&(no.clone(), campo.clone()))
                                    .is_some_and(|p| inclui(p, valor))
                            })
                    }
                }
            }
            Restricao::Ler {
                objeto,
                campo,
                destino,
            } => {
                let receiver = variavel(*objeto)?;
                let destino = variavel(*destino)?;
                if r.campos_opacos.contains(campo) || receiver.nos().is_none() {
                    destino.nos().is_none()
                } else {
                    receiver.nos().unwrap().iter().all(|no| {
                        // Ausente é bottom só no sistema de restrições; não
                        // prova inicialização vazia de um campo do heap real.
                        r.campos
                            .get(&(no.clone(), campo.clone()))
                            .is_none_or(|p| inclui(destino, p))
                    })
                }
            }
            Restricao::LerPorEsquema {
                objeto,
                esquemas,
                indice,
                destino,
            }
            | Restricao::GravarPorEsquema {
                objeto,
                esquemas,
                indice,
                valor: destino,
            } => {
                if esquemas
                    .values()
                    .any(|cs| cs.iter().enumerate().any(|(i, c)| c.posicao as usize != i))
                {
                    return Err(format!("verificador points-to: esquema na restrição {id}"));
                }
                let receiver = variavel(*objeto)?;
                let valor = variavel(*destino)?;
                let fronteira = receiver.nos().is_none_or(|ns| {
                    ns.is_empty()
                        || ns
                            .iter()
                            .any(|no| esquemas.get(no).and_then(|cs| cs.get(*indice)).is_none())
                });
                if fronteira {
                    esquemas
                        .values()
                        .flatten()
                        .all(|c| r.campos_opacos.contains(c))
                        && (!matches!(restricao, Restricao::LerPorEsquema { .. })
                            || valor.nos().is_none())
                } else {
                    receiver.nos().unwrap().iter().all(|no| {
                        let campo = &esquemas[no][*indice];
                        if matches!(restricao, Restricao::LerPorEsquema { .. }) {
                            if r.campos_opacos.contains(campo) {
                                valor.nos().is_none()
                            } else {
                                r.campos
                                    .get(&(no.clone(), campo.clone()))
                                    .is_none_or(|c| inclui(valor, c))
                            }
                        } else {
                            r.campos_opacos.contains(campo)
                                || r.campos
                                    .get(&(no.clone(), campo.clone()))
                                    .is_some_and(|c| inclui(c, valor))
                        }
                    })
                }
            }
        };
        if !valida {
            return Err(format!(
                "verificador points-to: restrição {id} não satisfeita"
            ));
        }
    }
    Ok(())
}

fn inclui(destino: &ConjuntoPontos, origem: &ConjuntoPontos) -> bool {
    match (destino.nos(), origem.nos()) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(d), Some(o)) => o.is_subset(d),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn leituras_por_esquema_conferem_com_caminhadas_em_343_grafos() {
        let esquemas = Arc::new(
            (0..3)
                .map(|i| (no(&format!("n{i}")), vec![campo(&format!("L{i}"))]))
                .collect::<BTreeMap<_, _>>(),
        );
        let mut conferidos = 0;
        for mascara in 0..512usize {
            // Cada nó tem saída: os testes de fronteira tratam separadamente
            // receivers null/vazios, sem supor que sejam reads puros.
            if (0..3).any(|v| mascara & (7 << (3 * v)) == 0) {
                continue;
            }
            conferidos += 1;
            let mut rs = Vec::new();
            for passo in 0..7 {
                rs.push(Restricao::LerPorEsquema {
                    objeto: if passo == 0 { 0 } else { 2 + passo },
                    esquemas: esquemas.clone(),
                    indice: 0,
                    destino: 3 + passo,
                });
            }
            for v in 0..3 {
                for w in 0..3 {
                    if mascara & (1 << (3 * v + w)) != 0 {
                        rs.push(Restricao::Gravar {
                            objeto: v,
                            campo: campo(&format!("L{v}")),
                            valor: w,
                        });
                    }
                }
                rs.push(Restricao::Semear {
                    destino: v,
                    no: no(&format!("n{v}")),
                });
            }
            // Oráculo de caminhadas de comprimento exato no grafo original;
            // não usa restrições, fila, esquema do solver ou seu verificador.
            let mut atuais = BTreeSet::from([0]);
            let mut esperados = Vec::new();
            for _ in 0..7 {
                atuais = (0..3)
                    .filter(|&w| atuais.iter().any(|&v| mascara & (1 << (3 * v + w)) != 0))
                    .collect();
                esperados.push(
                    atuais
                        .iter()
                        .map(|i| no(&format!("n{i}")))
                        .collect::<BTreeSet<_>>(),
                );
            }
            for _ in 0..2 {
                let r = resolver(10, 8, &rs).unwrap();
                for (passo, esperado) in esperados.iter().enumerate() {
                    assert_eq!(
                        r.variavel(3 + passo).unwrap().nos().unwrap(),
                        esperado,
                        "grafo {mascara}, passo {passo}"
                    );
                }
                assert!(r.campos_opacos.is_empty());
                rs.reverse();
            }
        }
        assert_eq!(conferidos, 343);
    }

    #[test]
    fn selecao_por_esquema_preserva_layouts_escrita_fraca_e_aliases_tardios() {
        let esquemas = Arc::new(BTreeMap::from([
            (no("a"), vec![campo("A")]),
            (no("b"), vec![campo("B")]),
        ]));
        let mut rs = vec![
            Restricao::LerPorEsquema {
                objeto: 0,
                esquemas: esquemas.clone(),
                indice: 0,
                destino: 3,
            },
            Restricao::LerPorEsquema {
                objeto: 6,
                esquemas: esquemas.clone(),
                indice: 0,
                destino: 7,
            },
            Restricao::GravarPorEsquema {
                objeto: 0,
                esquemas,
                indice: 0,
                valor: 4,
            },
            Restricao::Gravar {
                objeto: 5,
                campo: campo("A"),
                valor: 1,
            },
            Restricao::Gravar {
                objeto: 6,
                campo: campo("B"),
                valor: 2,
            },
            // Mesmo índice, outro layout: esta aresta não é lida em b.B.
            Restricao::Gravar {
                objeto: 6,
                campo: campo("A"),
                valor: 1,
            },
            Restricao::Semear {
                destino: 5,
                no: no("a"),
            },
            Restricao::Semear {
                destino: 6,
                no: no("b"),
            },
            Restricao::Copiar {
                origem: 5,
                destino: 0,
            },
            Restricao::Copiar {
                origem: 6,
                destino: 0,
            },
            Restricao::Semear {
                destino: 1,
                no: no("x"),
            },
            Restricao::Semear {
                destino: 2,
                no: no("y"),
            },
            Restricao::Semear {
                destino: 4,
                no: no("z"),
            },
        ];
        for _ in 0..2 {
            let mut r = resolver(8, 8, &rs).unwrap();
            assert_eq!(
                r.variavel(3).unwrap().nos().unwrap(),
                &BTreeSet::from([no("x"), no("y"), no("z")])
            );
            assert_eq!(
                r.variavel(7).unwrap().nos().unwrap(),
                &BTreeSet::from([no("y"), no("z")])
            );
            assert!(r.campos_opacos.is_empty());
            r.variaveis[3] = ConjuntoPontos::vazio(8);
            assert!(verificar(8, 8, &rs, &r).is_err());
            r = resolver(8, 8, &rs).unwrap();
            r.campos
                .insert((no("b"), campo("B")), ConjuntoPontos::vazio(8));
            assert!(verificar(8, 8, &rs, &r).is_err());
            rs.reverse();
        }
    }

    #[test]
    fn selecao_por_esquema_fecha_fronteiras_e_verificador_recusa_opacidade_apagada() {
        let esquemas = Arc::new(BTreeMap::from([(no("a"), vec![campo("A")])]));
        for caso in 0..5 {
            let mut rs = vec![Restricao::LerPorEsquema {
                objeto: 0,
                esquemas: esquemas.clone(),
                indice: if caso == 3 { 1 } else { 0 },
                destino: 1,
            }];
            match caso {
                0 => {} // Receiver vazio só é fechado após os aliases convergirem.
                1 => rs.push(Restricao::Desconhecer { destino: 0 }),
                2 => rs.push(Restricao::Semear {
                    destino: 0,
                    no: no("sem esquema"),
                }),
                3 | 4 => rs.push(Restricao::Semear {
                    destino: 0,
                    no: no("a"),
                }),
                _ => unreachable!(),
            }
            let limite = if caso == 4 { 0 } else { 8 };
            let mut r = resolver(2, limite, &rs).unwrap();
            assert!(r.variavel(1).unwrap().nos().is_none(), "caso {caso}");
            assert!(r.campos_opacos.contains(&campo("A")));
            r.campos_opacos.clear();
            assert!(verificar(2, limite, &rs, &r).is_err());
        }
        let rs = [Restricao::GravarPorEsquema {
            objeto: 0,
            esquemas,
            indice: 0,
            valor: 1,
        }];
        let mut r = resolver(2, 8, &rs).unwrap();
        assert!(r.campos_opacos.contains(&campo("A")));
        r.campos_opacos.clear();
        assert!(verificar(2, 8, &rs, &r).is_err());
        let invalido = Arc::new(BTreeMap::from([(
            no("a"),
            vec![CampoArc {
                layout: "A".into(),
                posicao: 9,
            }],
        )]));
        assert!(
            resolver(
                2,
                8,
                &[Restricao::LerPorEsquema {
                    objeto: 0,
                    esquemas: invalido,
                    indice: 0,
                    destino: 1
                }]
            )
            .is_err()
        );
    }
    #[test]
    fn verificador_rejeita_truncamento_copia_e_desconhecimento_apagados() {
        let rs = vec![
            Restricao::Semear {
                destino: 0,
                no: no("a"),
            },
            Restricao::Copiar {
                origem: 0,
                destino: 1,
            },
            Restricao::Desconhecer { destino: 2 },
        ];
        let mut r = resolver(3, 8, &rs).unwrap();
        r.variaveis[1] = ConjuntoPontos::vazio(8);
        assert!(verificar(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        r.variaveis[0] = ConjuntoPontos::vazio(8);
        assert!(verificar(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        r.variaveis[2] = ConjuntoPontos::vazio(8);
        assert!(verificar(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        assert!(verificar(3, 7, &rs, &r).is_err());
        assert!(verificar(2, 8, &rs, &r).is_err());
        assert!(
            verificar(
                3,
                8,
                &[Restricao::Copiar {
                    origem: 99,
                    destino: 0
                }],
                &r
            )
            .is_err()
        );
        // Aceitar sobreaproximação segura sem depender de igualdade ao solver.
        r.variaveis.fill(ConjuntoPontos::desconhecido(8));
        verificar(3, 8, &rs, &r).unwrap();
    }

    #[test]
    fn verificador_rejeita_arestas_de_store_leitura_e_opacidade_omitidas() {
        let rs = vec![
            Restricao::Semear {
                destino: 0,
                no: no("obj"),
            },
            Restricao::Semear {
                destino: 1,
                no: no("filho"),
            },
            Restricao::Gravar {
                objeto: 0,
                campo: campo("L"),
                valor: 1,
            },
            Restricao::Ler {
                objeto: 0,
                campo: campo("L"),
                destino: 2,
            },
        ];
        let mut r = resolver(3, 8, &rs).unwrap();
        r.campos.clear();
        assert!(verificar(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        r.campos
            .insert((no("obj"), campo("L")), ConjuntoPontos::vazio(8));
        assert!(verificar(3, 8, &rs, &r).is_err());
        r = resolver(3, 8, &rs).unwrap();
        r.variaveis[2] = ConjuntoPontos::vazio(8);
        assert!(verificar(3, 8, &rs, &r).is_err());
        let mut opacas = rs;
        opacas.push(Restricao::DesconhecerCampo { campo: campo("L") });
        r = resolver(3, 8, &opacas).unwrap();
        r.campos_opacos.clear();
        assert!(verificar(3, 8, &opacas, &r).is_err());
    }

    #[test]
    fn receiver_topo_exige_opacidade_global_para_receivers_futuros() {
        let rs = vec![
            Restricao::Desconhecer { destino: 0 },
            Restricao::Gravar {
                objeto: 0,
                campo: campo("L"),
                valor: 1,
            },
            Restricao::Semear {
                destino: 2,
                no: no("tardio"),
            },
            Restricao::Ler {
                objeto: 2,
                campo: campo("L"),
                destino: 3,
            },
        ];
        let mut r = resolver(4, 8, &rs).unwrap();
        r.campos_opacos.clear();
        assert!(verificar(4, 8, &rs, &r).is_err());
    }

    #[test]
    fn inclusao_confere_com_fecho_transitivo_em_todos_os_grafos_de_tres_variaveis() {
        for mascara in 0..512 {
            let mut rs = Vec::new();
            let mut alcance = [[false; 3]; 3];
            for v in 0..3 {
                alcance[v][v] = true;
                rs.push(Restricao::Semear {
                    destino: v,
                    no: no(&v.to_string()),
                });
                for w in 0..3 {
                    if mascara & (1 << (3 * v + w)) != 0 {
                        alcance[v][w] = true;
                        rs.push(Restricao::Copiar {
                            origem: v,
                            destino: w,
                        });
                    }
                }
            }
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        alcance[v][w] |= alcance[v][k] && alcance[k][w];
                    }
                }
            }
            for limite in 1..=3 {
                let r = resolver(3, limite, &rs).unwrap();
                for w in 0..3 {
                    let esperado: BTreeSet<_> = (0..3)
                        .filter(|&v| alcance[v][w])
                        .map(|v| no(&v.to_string()))
                        .collect();
                    let obtido = r.variavel(w).unwrap().nos();
                    if esperado.len() > limite {
                        assert!(obtido.is_none());
                    } else {
                        assert_eq!(obtido.unwrap(), &esperado, "grafo {mascara}");
                    }
                }
            }
        }
    }
    fn no(n: &str) -> NoAbstrato {
        NoAbstrato::Global(n.into())
    }
    fn campo(n: &str) -> CampoArc {
        CampoArc {
            layout: n.into(),
            posicao: 0,
        }
    }

    #[test]
    fn campos_e_aliases_tardios_convergem_sem_apagar_arestas() {
        let mut rs = vec![
            Restricao::Ler {
                objeto: 2,
                campo: campo("L"),
                destino: 3,
            },
            Restricao::Gravar {
                objeto: 0,
                campo: campo("L"),
                valor: 1,
            },
            Restricao::Copiar {
                origem: 0,
                destino: 2,
            },
            Restricao::Semear {
                destino: 0,
                no: no("a"),
            },
            Restricao::Semear {
                destino: 1,
                no: no("b"),
            },
            Restricao::Semear {
                destino: 1,
                no: no("c"),
            },
        ];
        let r = resolver(4, 8, &rs).unwrap();
        assert_eq!(r.variavel(3), r.variavel(1));
        assert_eq!(r.campo(&no("a"), &campo("L")), r.variavel(1));
        rs.reverse();
        let inverso = resolver(4, 8, &rs).unwrap();
        assert_eq!(r.variaveis, inverso.variaveis);
        assert_eq!(r.campos, inverso.campos);
    }

    #[test]
    fn escritor_opaco_e_budget_invalidam_leitores_sem_contaminar_outro_layout() {
        for opaco in [true, false] {
            let rs = vec![
                Restricao::Ler {
                    objeto: 0,
                    campo: campo("L"),
                    destino: 3,
                },
                Restricao::Ler {
                    objeto: 0,
                    campo: campo("Outro"),
                    destino: 4,
                },
                Restricao::Gravar {
                    objeto: 0,
                    campo: campo("Outro"),
                    valor: 2,
                },
                Restricao::Gravar {
                    objeto: 1,
                    campo: campo("L"),
                    valor: 2,
                },
                Restricao::Semear {
                    destino: 0,
                    no: no("a"),
                },
                Restricao::Semear {
                    destino: 1,
                    no: no("a"),
                },
                Restricao::Semear {
                    destino: 2,
                    no: no("b"),
                },
                if opaco {
                    Restricao::Desconhecer { destino: 1 }
                } else {
                    Restricao::Semear {
                        destino: 1,
                        no: no("c"),
                    }
                },
            ];
            let r = resolver(5, 1, &rs).unwrap();
            assert!(r.variavel(3).unwrap().nos().is_none());
            assert_eq!(r.variavel(4), r.variavel(2));
            assert!(r.campo(&no("a"), &campo("L")).is_none());
        }
    }

    #[test]
    fn cadeia_profunda_e_phi_recursivo_terminam_por_dependencias() {
        let n = 20_000;
        let mut rs: Vec<_> = (1..n)
            .rev()
            .map(|v| Restricao::Copiar {
                origem: v - 1,
                destino: v,
            })
            .collect();
        rs.push(Restricao::Copiar {
            origem: n - 1,
            destino: 0,
        });
        rs.push(Restricao::Semear {
            destino: 0,
            no: no("raiz"),
        });
        let r = resolver(n, 1, &rs).unwrap();
        assert!(r.variaveis.iter().all(|p| p == &r.variaveis[0]));
        assert_eq!(r.variaveis[0].nos().unwrap().len(), 1);
    }

    #[test]
    fn indices_invalidos_sao_erros_e_nunca_fatos_vazios() {
        assert!(
            resolver(
                1,
                8,
                &[Restricao::Copiar {
                    origem: 1,
                    destino: 0
                }]
            )
            .is_err()
        );
        assert!(
            resolver(
                1,
                8,
                &[Restricao::Ler {
                    objeto: 0,
                    campo: campo("L"),
                    destino: 1
                }]
            )
            .is_err()
        );
    }
}
