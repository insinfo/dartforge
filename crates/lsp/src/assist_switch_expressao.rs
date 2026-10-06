//! `ConvertToSwitchExpression` (convert_to_switch_expression.dart) do
//! `analysis_server` 3.6.2: o comando `switch` cujos casos só retornam, só
//! atribuem à mesma variável local ou só passam um argumento à mesma função
//! de topo vira uma expressão `switch`.
//!
//! | Título | Espécie |
//! |---|---|
//! | `Convert to switch expression` | `refactor.convert.switchExpression` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::{Contexto, Elem};
use crate::refatoracoes_exec::{Texto, UM_RECUO};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassKind, FunctionKind};
use dartforge_types::{Type, TypeId, TypeTable};

/// `_Group`: o `default` sozinho ou os casos de padrão juntos.
enum Grupo {
    Padrao { membro: usize, comandos: Vec<usize> },
    Casos { casos: Vec<usize>, comandos: Vec<usize> },
}

impl Grupo {
    fn comandos(&self) -> &[usize] {
        match self {
            Grupo::Padrao { comandos, .. } | Grupo::Casos { comandos, .. } => comandos,
        }
    }
}

/// `_SwitchType`.
enum TipoDeSwitch {
    Retorno,
    /// O nome da variável e o operador.
    Atribuicao(String, String),
    /// O nome da função.
    Argumento(String),
}

impl Contexto<'_> {
    /// `TypeSystemImpl.isAlwaysExhaustive` do analyzer 6.11 (sobre uma cópia
    /// da tabela: a *erasure* de um tipo de extensão pode criar tipos).
    fn sempre_exaustivo_no_lsp(&self, tabela: &mut TypeTable, t: TypeId, prof: u32) -> bool {
        if prof > 32 {
            return false;
        }
        let consulta = &self.p.consulta;
        let prog = self.p.programa();
        match tabela.get(t).clone() {
            Type::Null => true,
            Type::Interface { class, .. } => {
                let c = prog.class(class);
                Some(class) == consulta.core.bool_class || c.kind == ClassKind::Enum || (c.kind == ClassKind::Class && c.modifiers.sealed)
            }
            Type::ExtensionType { .. } => {
                let apagado = {
                    let mut motor = dartforge_types::constantes::avaliador::Motor::novo(prog, &consulta.nomes, tabela, &consulta.core, &consulta.outline, &consulta.corpos, &self.p.bibliotecas);
                    motor.apagar(t)
                };
                apagado != t && self.sempre_exaustivo_no_lsp(tabela, apagado, prof + 1)
            }
            Type::FutureOr { arg, .. } => self.sempre_exaustivo_no_lsp(tabela, arg, prof + 1),
            Type::Intersection { param, bound } => {
                self.sempre_exaustivo_no_lsp(tabela, bound, prof + 1) || {
                    let (explicito, limite) = (tabela.param(param).explicito, tabela.param(param).bound);
                    explicito && self.sempre_exaustivo_no_lsp(tabela, limite, prof + 1)
                }
            }
            Type::TypeParameter { param, .. } => {
                let (explicito, limite) = (tabela.param(param).explicito, tabela.param(param).bound);
                explicito && self.sempre_exaustivo_no_lsp(tabela, limite, prof + 1)
            }
            Type::Record { positional, named, .. } => {
                let campos: Vec<TypeId> = positional.iter().copied().chain(named.iter().map(|(_, c)| *c)).collect();
                campos.into_iter().all(|c| self.sempre_exaustivo_no_lsp(tabela, c, prof + 1))
            }
            _ => false,
        }
    }

    /// O `:` de um membro de `switch`.
    fn dois_pontos_do_membro(&self, membro: usize) -> Option<Span> {
        let depois = match self.especie(membro) {
            "SwitchDefault" => self.token_seguinte(self.arvore.nos[membro].inicio)?.end,
            _ => {
                let guardado = self.filhos(membro).iter().copied().find(|&f| self.especie(f) == "GuardedPattern")?;
                self.arvore.nos[guardado].fim
            }
        };
        self.token_seguinte(depois).filter(|t| &self.fonte[t.start..t.end] == ":")
    }

    /// O último token do nó (`endToken`).
    fn ultimo_token(&self, n: usize) -> Option<Span> {
        self.token_anterior(self.arvore.nos[n].fim)
    }

    /// `statement.beginToken.precedingComments != null`.
    fn tem_comentario_antes(&self, n: usize) -> bool {
        !self.comentarios_antes(Span { start: self.arvore.nos[n].inicio, end: self.arvore.nos[n].inicio }).is_empty()
    }

    /// `_getBreakRange`: do fim do primeiro comentário antes do `break` (ou
    /// do token anterior) ao fim do `break`.
    fn faixa_do_break(&self, b: usize) -> Option<Span> {
        let s = self.arvore.nos[b].inicio;
        let antes = match self.comentarios_antes(Span { start: s, end: s }).first() {
            Some(c) => c.end,
            None => self.token_anterior(s)?.end,
        };
        Some(Span { start: antes, end: self.arvore.nos[b].fim })
    }

    /// O padrão (o texto) dos casos juntos.
    fn texto_dos_padroes(&self, casos: &[usize]) -> Option<String> {
        let mut partes = Vec::new();
        for &c in casos {
            let guardado = self.filhos(c).iter().copied().find(|&f| self.especie(f) == "GuardedPattern")?;
            partes.push(self.texto_do_no(*self.filhos(guardado).first()?));
        }
        Some(partes.join(" || "))
    }

    /// `_getSupportedSwitchType`.
    fn tipo_de_switch(&self, switch: usize) -> Option<(TipoDeSwitch, Vec<Grupo>)> {
        let grupos_de_membros = self.grupos_de_membros(switch);
        if grupos_de_membros.is_empty() {
            return None;
        }
        let prog = self.p.programa();
        let mut pode_retorno = true;
        let mut pode_atribuicao = true;
        let mut pode_argumento = true;
        let mut funcao: Option<(dartforge_elements::model::FunctionElementId, String)> = None;
        let mut escrita: Option<(usize, String, String)> = None;
        let mut grupos = Vec::new();
        let quantos = grupos_de_membros.len();
        for (membros, comandos) in grupos_de_membros {
            if membros.iter().any(|&m| self.filhos(m).iter().any(|&f| self.especie(f) == "Label")) {
                return None;
            }
            if let [m] = membros[..]
                && self.especie(m) == "SwitchDefault"
            {
                grupos.push(Grupo::Padrao { membro: m, comandos: comandos.clone() });
            } else {
                if membros.iter().any(|&m| self.especie(m) != "SwitchPatternCase") {
                    return None;
                }
                if membros.len() != 1
                    && membros.iter().any(|&m| {
                        self.filhos(m)
                            .iter()
                            .any(|&g| self.especie(g) == "GuardedPattern" && self.filhos(g).iter().any(|&w| self.especie(w) == "WhenClause"))
                    })
                {
                    return None;
                }
                grupos.push(Grupo::Casos { casos: membros.clone(), comandos: comandos.clone() });
            }
            if comandos.is_empty() || comandos.len() > 2 {
                return None;
            }
            if let [_, segundo] = comandos[..] {
                if self.especie(segundo) != "BreakStatement" {
                    return None;
                }
                pode_retorno = false;
            }
            let comando = comandos[0];
            if self.especie(comando) == "ExpressionStatement" {
                let expressao = *self.filhos(comando).first()?;
                if self.especie(expressao) == "ThrowExpression" {
                    if quantos == 1 {
                        pode_atribuicao = false;
                        pode_argumento = false;
                    }
                    continue;
                }
                pode_retorno = false;
                if pode_argumento && self.especie(expressao) == "MethodInvocation" {
                    pode_atribuicao = false;
                    let nome = self.nome_do_metodo(expressao)?;
                    let Elem::Funcao(f) = self.elemento_do_identificador(nome, true) else { return None };
                    let fe = prog.function(f);
                    if fe.class.is_some() || fe.extension.is_some() || fe.kind != FunctionKind::Function {
                        return None;
                    }
                    match &funcao {
                        None => funcao = Some((f, self.p.nome(fe.name).to_string())),
                        Some((g, _)) if *g != f => return None,
                        _ => {}
                    }
                } else if pode_atribuicao && self.especie(expressao) == "AssignmentExpression" {
                    pode_argumento = false;
                    let esquerdo = *self.filhos(expressao).first()?;
                    if self.especie(esquerdo) != "SimpleIdentifier" {
                        return None;
                    }
                    let operador = self.token_seguinte(self.arvore.nos[esquerdo].fim)?;
                    let operador = self.fonte[operador.start..operador.end].to_string();
                    let declaracao = match self.arvore.nos[esquerdo].marca {
                        crate::arvore_analyzer::Marca::Expr(x) => self.corpos.declaracao_local(x),
                        _ => None,
                    };
                    match &escrita {
                        None => {
                            let Some(d) = declaracao else { return None };
                            if !matches!(self.elemento_local(d), Elem::VariavelLocal) {
                                return None;
                            }
                            escrita = Some((d, self.texto_do_no(esquerdo).to_string(), operador));
                        }
                        Some((d, _, o)) => {
                            if declaracao != Some(*d) || *o != operador {
                                return None;
                            }
                        }
                    }
                } else {
                    return None;
                }
            } else {
                if !pode_retorno || self.especie(comando) != "ReturnStatement" || self.filhos(comando).is_empty() {
                    return None;
                }
                pode_atribuicao = false;
                pode_argumento = false;
            }
            if !pode_retorno && !pode_atribuicao && !pode_argumento {
                return None;
            }
        }
        let tipo = if pode_retorno {
            TipoDeSwitch::Retorno
        } else if pode_atribuicao {
            let (_, nome, operador) = escrita?;
            TipoDeSwitch::Atribuicao(nome, operador)
        } else if pode_argumento {
            TipoDeSwitch::Argumento(funcao?.1)
        } else {
            return None;
        };
        Some((tipo, grupos))
    }

    /// `ConvertToSwitchExpression`.
    pub(crate) fn switch_em_expressao(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let switch = self.arvore.localizar(inicio, fim)?;
        if self.especie(switch) != "SwitchStatement" {
            return None;
        }
        let escrutinio = *self.filhos(switch).first()?;
        let tipo_do_escrutinio = self.expr_do_no(escrutinio).and_then(|x| self.corpos.get_type(x));
        // `_isEffectivelyExhaustive`.
        let exaustivo = tipo_do_escrutinio.is_some_and(|t| {
            self.sempre_exaustivo_no_lsp(&mut self.p.consulta.tabela.clone(), t, 0) || {
                let membros: Vec<usize> = self.filhos(switch).iter().copied().filter(|&k| matches!(self.especie(k), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")).collect();
                match membros.last() {
                    Some(&u) if self.especie(u) == "SwitchPatternCase" => self
                        .filhos(u)
                        .iter()
                        .copied()
                        .find(|&g| self.especie(g) == "GuardedPattern")
                        .and_then(|g| self.filhos(g).first().copied())
                        .is_some_and(|p| self.especie(p) == "WildcardPattern"),
                    Some(&u) => self.especie(u) == "SwitchDefault",
                    None => false,
                }
            }
        });
        // `followingThrow`: o comando seguinte no bloco é `throw …;`.
        let mut throw_seguinte: Option<(usize, usize)> = None;
        if !exaustivo {
            let bloco = self.pai(switch).filter(|&b| self.especie(b) == "Block")?;
            let irmaos = self.filhos(bloco);
            let i = irmaos.iter().position(|&k| k == switch)?;
            let proximo = *irmaos.get(i + 1)?;
            if self.especie(proximo) != "ExpressionStatement" {
                return None;
            }
            let t = *self.filhos(proximo).first()?;
            if self.especie(t) != "ThrowExpression" {
                return None;
            }
            throw_seguinte = Some((proximo, t));
        }
        let (tipo, grupos) = self.tipo_de_switch(switch)?;
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let (ini_s, fim_s) = (self.arvore.nos[switch].inicio, self.arvore.nos[switch].fim);
        let mut edicoes: Vec<(Span, String)> = Vec::new();
        let vazio = |p: usize| Span { start: p, end: p };
        let quantos = grupos.len();
        match &tipo {
            TipoDeSwitch::Retorno => {
                edicoes.push((vazio(ini_s), "return ".to_string()));
                edicoes.push((vazio(fim_s), ";".to_string()));
                for (i, g) in grupos.iter().enumerate() {
                    let (dois_pontos, virgula) = match g {
                        Grupo::Padrao { membro, .. } => {
                            let dp = self.dois_pontos_do_membro(*membro)?;
                            edicoes.push((Span { start: self.arvore.nos[*membro].inicio, end: dp.end }, "_ =>".to_string()));
                            (dp, false)
                        }
                        Grupo::Casos { casos, .. } => {
                            let dp = self.dois_pontos_do_membro(*casos.last()?)?;
                            let texto = self.texto_dos_padroes(casos)?;
                            edicoes.push((Span { start: self.arvore.nos[casos[0]].inicio, end: dp.end }, format!("{texto} =>")));
                            (dp, i < quantos - 1 || throw_seguinte.is_some())
                        }
                    };
                    let comando = *g.comandos().first()?;
                    let comentario = self.tem_comentario_antes(comando);
                    match self.especie(comando) {
                        "ReturnStatement" => {
                            let e = *self.filhos(comando).first()?;
                            if !comentario {
                                edicoes.push((Span { start: dois_pontos.end, end: self.arvore.nos[e].inicio }, " ".to_string()));
                            } else {
                                edicoes.push((Span { start: self.arvore.nos[comando].inicio, end: self.arvore.nos[e].inicio }, String::new()));
                            }
                        }
                        "ExpressionStatement" => {
                            let e = *self.filhos(comando).first()?;
                            if self.especie(e) == "ThrowExpression" && !comentario {
                                edicoes.push((Span { start: dois_pontos.end, end: self.arvore.nos[e].inicio }, " ".to_string()));
                            }
                        }
                        _ => {}
                    }
                    edicoes.push((self.ultimo_token(comando)?, if virgula { ",".to_string() } else { String::new() }));
                }
                if let Some((comando, throw)) = throw_seguinte {
                    let fecha = self.ultimo_token(switch)?;
                    let texto = format!("{}_ => {},", tx.prefixo_da_linha(fecha.start), self.texto_do_no(throw));
                    let recuado: Vec<String> = texto.split(eol).map(|l| format!("{UM_RECUO}{l}")).collect();
                    edicoes.push((vazio(tx.inicio_do_conteudo(fecha.start)), format!("{}{eol}", recuado.join(eol))));
                    edicoes.push((tx.faixa_de_linhas(self.arvore.nos[comando].inicio, self.arvore.nos[comando].fim), String::new()));
                }
            }
            TipoDeSwitch::Atribuicao(nome, operador) => {
                edicoes.push((vazio(ini_s), format!("{nome} {operador} ")));
                for (i, g) in grupos.iter().enumerate() {
                    let (dois_pontos, virgula) = match g {
                        Grupo::Padrao { membro, .. } => {
                            let dp = self.dois_pontos_do_membro(*membro)?;
                            edicoes.push((Span { start: self.arvore.nos[*membro].inicio, end: dp.end }, "_ =>".to_string()));
                            (dp, false)
                        }
                        Grupo::Casos { casos, .. } => {
                            let dp = self.dois_pontos_do_membro(*casos.last()?)?;
                            let texto = self.texto_dos_padroes(casos)?;
                            edicoes.push((Span { start: self.arvore.nos[casos[0]].inicio, end: dp.end }, format!("{texto} =>")));
                            (dp, i < quantos - 1)
                        }
                    };
                    for &comando in g.comandos() {
                        if self.especie(comando) == "ExpressionStatement" {
                            let e = *self.filhos(comando).first()?;
                            if self.especie(e) == "AssignmentExpression" {
                                let esquerdo = *self.filhos(e).first()?;
                                let direito = *self.filhos(e).last()?;
                                let op = self.token_seguinte(self.arvore.nos[esquerdo].fim)?;
                                if !self.tem_comentario_antes(comando) {
                                    edicoes.push((Span { start: dois_pontos.end, end: op.end }, String::new()));
                                } else {
                                    edicoes.push((Span { start: self.arvore.nos[e].inicio, end: self.arvore.nos[direito].inicio }, String::new()));
                                }
                            } else if self.especie(e) == "ThrowExpression" {
                                edicoes.push((Span { start: dois_pontos.end, end: self.arvore.nos[comando].inicio - 1 }, String::new()));
                            }
                        }
                        if self.especie(comando) == "BreakStatement" {
                            edicoes.push((self.faixa_do_break(comando)?, String::new()));
                        } else {
                            edicoes.push((self.ultimo_token(comando)?, if virgula { ",".to_string() } else { String::new() }));
                        }
                    }
                }
                edicoes.push((vazio(fim_s), ";".to_string()));
            }
            TipoDeSwitch::Argumento(nome) => {
                edicoes.push((vazio(ini_s), format!("{nome}(")));
                for (i, g) in grupos.iter().enumerate() {
                    let (dois_pontos, virgula) = match g {
                        Grupo::Padrao { membro, .. } => {
                            let dp = self.dois_pontos_do_membro(*membro)?;
                            edicoes.push((Span { start: self.arvore.nos[*membro].inicio, end: dp.end }, "_ =>".to_string()));
                            (dp, false)
                        }
                        Grupo::Casos { casos, .. } => {
                            let dp = self.dois_pontos_do_membro(*casos.last()?)?;
                            let texto = self.texto_dos_padroes(casos)?;
                            edicoes.push((Span { start: self.arvore.nos[casos[0]].inicio, end: dp.end }, format!("{texto} => ")));
                            (dp, i < quantos - 1)
                        }
                    };
                    for &comando in g.comandos() {
                        let comentario = self.tem_comentario_antes(comando);
                        if self.especie(comando) == "ExpressionStatement" {
                            let e = *self.filhos(comando).first()?;
                            if self.especie(e) == "MethodInvocation" {
                                let lista = self.filhos(e).iter().copied().find(|&f| self.especie(f) == "ArgumentList")?;
                                let abre = self.arvore.nos[lista].inicio + 1;
                                let fecha = self.arvore.nos[lista].fim;
                                let de = if !comentario { dois_pontos.end } else { self.arvore.nos[e].inicio };
                                edicoes.push((Span { start: de, end: abre }, String::new()));
                                edicoes.push((Span { start: fecha - 1, end: fecha }, String::new()));
                            }
                            if !comentario && self.especie(e) == "ThrowExpression" {
                                edicoes.push((Span { start: dois_pontos.end, end: self.arvore.nos[comando].inicio - 1 }, String::new()));
                            }
                        }
                        if self.especie(comando) == "BreakStatement" {
                            edicoes.push((self.faixa_do_break(comando)?, String::new()));
                        } else {
                            edicoes.push((self.ultimo_token(comando)?, if virgula { ",".to_string() } else { String::new() }));
                        }
                    }
                }
                edicoes.push((vazio(fim_s), ");".to_string()));
            }
        }
        Some(self.acao_simples(uri, "Convert to switch expression", "refactor.convert.switchExpression", edicoes))
    }
}
