//! Solver de inclusão por campo, sem atualização forte (§28.2).
//! Consome restrições explícitas, ainda sem extração HIR/resumos SDK.
//! Ausência de uma restrição não certifica ausência de efeitos do programa.

use super::modelo::{ConjuntoPontos, NoAbstrato};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Campo nominal de um layout; índices iguais de layouts distintos não coincidem.
/// Versão/validação de layout pertence ao produtor das restrições.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::points_to::CampoArc;
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
/// use dartforge_emit_native::otimizar::arc::analise::points_to::Restricao;
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
/// use dartforge_emit_native::otimizar::arc::analise::points_to::{resolver, Restricao};
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
    /// use dartforge_emit_native::otimizar::arc::analise::points_to::resolver;
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
    /// use dartforge_emit_native::otimizar::arc::analise::{modelo::NoAbstrato, points_to::*};
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
/// Índice de variável fora do domínio declarado. Validação precede a solução.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::{modelo::NoAbstrato, points_to::*};
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
            Restricao::Gravar { objeto, valor, .. } => (vec![*objeto, *valor], None),
            Restricao::Ler {
                objeto,
                campo,
                destino,
            } => {
                leitores.entry(campo.clone()).or_default().push(id);
                (vec![*objeto], Some(*destino))
            }
            Restricao::DesconhecerCampo { .. } => (vec![], None),
        };
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
    while let Some(id) = fila.pop_front() {
        agendada[id] = false;
        let r = &restricoes[id];
        let mut variavel_alterada = None;
        let mut campo_alterado = None;
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
                            campo_alterado = Some(campo);
                        }
                    }
                } else if resultado.campos_opacos.insert(campo.clone()) {
                    campo_alterado = Some(campo);
                }
            }
            Restricao::DesconhecerCampo { campo } => {
                if resultado.campos_opacos.insert(campo.clone()) {
                    campo_alterado = Some(campo);
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
        }
        let eventos = variavel_alterada
            .into_iter()
            .flat_map(|v| dependentes[v].iter())
            .chain(
                campo_alterado
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
    Ok(resultado)
}

#[cfg(test)]
mod testes {
    use super::*;
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
        assert!(resolver(
            1,
            8,
            &[Restricao::Copiar {
                origem: 1,
                destino: 0
            }]
        )
        .is_err());
        assert!(resolver(
            1,
            8,
            &[Restricao::Ler {
                objeto: 0,
                campo: campo("L"),
                destino: 1
            }]
        )
        .is_err());
    }
}
