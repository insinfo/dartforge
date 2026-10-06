//! `ConvertClassToEnum` (convert_class_to_enum.dart) do `analysis_server`
//! 3.6.2: a classe privada (ou de construtores privados) cujos campos
//! estáticos constantes são as instâncias dela vira um `enum`.
//!
//! | Título | Espécie |
//! |---|---|
//! | `Convert class to an enum` | `refactor.convert.classToEnum` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::{Ligacao, Marca};
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Texto, UM_RECUO};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_types::constantes::avaliador::Motor;
use dartforge_types::constantes::valor::{Campo, Estado, Valor};
use dartforge_types::{Resolved, Type};
use std::collections::HashSet;

/// `_Field`: o membro (a posição em `members`), o nó da
/// `VariableDeclaration` e os nós das variáveis da lista.
struct CampoDaClasse {
    membro: usize,
    variavel: usize,
    variaveis: Vec<usize>,
}

/// `_ConstantField`.
struct CampoConstante {
    campo: CampoDaClasse,
    nome: String,
    criacao: usize,
    construtor: FunctionElementId,
    indice: i64,
}

/// `_Parameter` de um construtor usado: o construtor, o membro e a posição
/// do parâmetro `this.index` na lista.
struct ParametroDoIndice {
    construtor: FunctionElementId,
    membro: usize,
    posicao: usize,
}

impl Contexto<'_> {
    /// O valor de `IntegerLiteral.value` (decimal ou `0x`, até 64 bits no
    /// hexadecimal).
    fn valor_do_literal_inteiro(&self, n: usize) -> Option<i64> {
        let texto: String = self.texto_do_no(n).chars().filter(|c| *c != '_').collect();
        match texto.strip_prefix("0x").or_else(|| texto.strip_prefix("0X")) {
            Some(hex) => u64::from_str_radix(hex, 16).ok().map(|v| v as i64),
            None => texto.parse::<i64>().ok(),
        }
    }

    /// `ConvertClassToEnum`.
    pub(crate) fn converter_classe_em_enum(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let prog = self.p.programa();
        let consulta = &self.p.consulta;
        let lib = prog.unit(self.unidade).library;
        if prog.library(lib).features.versao() < dartforge_frontend::features::LanguageVersion::new(2, 17) {
            return None;
        }
        // Com partes não dá para ver as criações e as subclasses nelas.
        if prog.library(lib).units.len() > 1 {
            return None;
        }
        let declaracao = self.arvore.localizar(inicio, fim)?;
        if self.especie(declaracao) != "ClassDeclaration" {
            return None;
        }
        let Marca::Decl(d) = self.arvore.nos[declaracao].marca else { return None };
        let DeclKind::Class(k) = &self.ast.decl(d).kind else { return None };
        // `declaration.name == token`.
        if k.mixin_application || inicio < k.name.span.start || inicio > k.name.span.end {
            return None;
        }
        let classe = self.classe_da_declaracao(self.unidade, d)?;
        if k.modifiers.abstract_ || k.modifiers.sealed {
            return None;
        }
        let dados = consulta.outline.classes.get(classe.0 as usize)?;
        if k.extends.is_some()
            && let Some(s) = dados.supertype
            && !matches!(consulta.tabela.get(s), Type::Interface { class, .. } if Some(*class) == consulta.core.object_class)
        {
            return None;
        }
        let filhos_da_classe = self.filhos(declaracao);
        let nos_dos_membros: Vec<usize> = k
            .members
            .iter()
            .map(|&m| filhos_da_classe.iter().copied().find(|&f| self.arvore.nos[f].ligacao == Ligacao::Membro(m)))
            .collect::<Option<_>>()?;
        let nome_de = |s: Span| &self.fonte[s.start..s.end];

        // `_validateConstructors`.
        let privada = nome_de(k.name.span).starts_with('_');
        let mut construtores: Vec<(FunctionElementId, usize)> = Vec::new();
        for (i, &m) in k.members.iter().enumerate() {
            let MemberKind::Constructor(c) = &self.ast.member(m).kind else { continue };
            let f = self.p.construtor_do_no(self.unidade, m)?;
            let construtor_privado = c.name.is_some_and(|n| nome_de(n.span).starts_with('_'));
            if !privada && !construtor_privado {
                return None;
            }
            if !c.factory && !c.const_ {
                return None;
            }
            construtores.push((f, i));
        }

        // `_validateMethods`.
        for &m in k.members.iter() {
            if let MemberKind::Method(fid) = self.ast.member(m).kind
                && let Some(n) = self.ast.function(fid).name
                && matches!(nome_de(n.span), "==" | "hashCode")
            {
                return None;
            }
        }

        // `_validateFields`.
        let variavel_do_campo = |m: ast::MemberId, j: usize| -> Option<VariableId> {
            prog.class(classe).fields.iter().copied().find(|&v| prog.variable(v).node == VariableRef::Field { unit: self.unidade, member: m, index: j })
        };
        let mut tabela = consulta.tabela.clone();
        let mut motor = Motor::novo(prog, &consulta.nomes, &mut tabela, &consulta.core, &consulta.outline, &consulta.corpos, &self.p.bibliotecas);
        let mut grupos: Vec<(Valor, Vec<CampoConstante>)> = Vec::new();
        let mut campo_do_indice: Option<CampoDaClasse> = None;
        for (i, &m) in k.members.iter().enumerate() {
            let MemberKind::Field(l) = &self.ast.member(m).kind else { continue };
            let lista = self.filhos(nos_dos_membros[i]).iter().copied().find(|&f| self.especie(f) == "VariableDeclarationList")?;
            let variaveis: Vec<usize> = self.filhos(lista).iter().copied().filter(|&f| self.especie(f) == "VariableDeclaration").collect();
            if variaveis.len() != l.variables.len() {
                return None;
            }
            if l.static_ {
                for (j, v) in l.variables.iter().enumerate() {
                    let Some(var) = variavel_do_campo(m, j) else { continue };
                    if !prog.variable(var).const_ {
                        continue;
                    }
                    let Some(t) = consulta.tipo_da_variavel(var) else { continue };
                    if !matches!(consulta.tabela.get(t), Type::Interface { class, .. } if *class == classe) {
                        continue;
                    }
                    if v.initializer.is_none() {
                        continue;
                    }
                    let Some(&criacao) = self.filhos(variaveis[j]).first() else { continue };
                    if self.especie(criacao) != "InstanceCreationExpression" {
                        continue;
                    }
                    let Some(Resolved::Constructor(f)) = self.expr_do_no(criacao).and_then(|x| self.corpos.get_resolved(x)) else { continue };
                    let f = *f;
                    if prog.function(f).factory || prog.function(f).class != Some(classe) {
                        continue;
                    }
                    let Some(valor) = motor.valor_de_variavel(var).and_then(|c| c.valor().cloned()) else { continue };
                    if l.variables.len() != 1 {
                        return None;
                    }
                    // `fieldValue.getField('index')?.toIntValue() ?? -1`.
                    let indice = match &valor.estado {
                        Estado::Generico { campos, .. } => campos.iter().find_map(|(c, x)| match (c, &x.estado) {
                            (Campo::Nome(s), Estado::Int(Some(n))) if self.p.nome(*s) == "index" => Some(*n),
                            _ => None,
                        }),
                        _ => None,
                    }
                    .unwrap_or(-1);
                    let campo = CampoConstante {
                        campo: CampoDaClasse { membro: i, variavel: variaveis[j], variaveis: variaveis.clone() },
                        nome: nome_de(v.name.span).to_string(),
                        criacao,
                        construtor: f,
                        indice,
                    };
                    let mut achado = None;
                    for (g, (chave, _)) in grupos.iter().enumerate() {
                        if motor.iguais(chave, &valor) {
                            achado = Some(g);
                            break;
                        }
                    }
                    match achado {
                        Some(g) => grupos[g].1.push(campo),
                        None => grupos.push((valor, vec![campo])),
                    }
                }
            } else {
                for (j, v) in l.variables.iter().enumerate() {
                    if !l.final_ {
                        return None;
                    }
                    let Some(var) = variavel_do_campo(m, j) else { continue };
                    let e_int = consulta
                        .tipo_da_variavel(var)
                        .is_some_and(|t| matches!(consulta.tabela.get(t), Type::Interface { class, .. } if Some(*class) == consulta.core.int_class));
                    if nome_de(v.name.span) == "index" && e_int {
                        campo_do_indice = Some(CampoDaClasse { membro: i, variavel: variaveis[j], variaveis: variaveis.clone() });
                    }
                }
            }
        }
        drop(motor);
        let mut campos: Vec<CampoConstante> = Vec::new();
        for (_, lista) in grupos {
            if lista.len() != 1 {
                return None;
            }
            campos.extend(lista);
        }
        if campos.is_empty() {
            return None;
        }

        // `_EnumVisitor` e `_NonEnumVisitor`: nenhuma criação por construtor
        // gerador da classe fora do inicializador de um campo a converter.
        let (ci, cf) = (self.arvore.nos[declaracao].inicio, self.arvore.nos[declaracao].fim);
        let permitidas: HashSet<usize> = campos.iter().map(|c| c.criacao).collect();
        for n in 0..self.arvore.nos.len() {
            if self.especie(n) != "InstanceCreationExpression" {
                continue;
            }
            let gerador = matches!(
                self.expr_do_no(n).and_then(|x| self.corpos.get_resolved(x)),
                Some(Resolved::Constructor(f)) if !prog.function(*f).factory && prog.function(*f).class == Some(classe)
            );
            if !gerador {
                continue;
            }
            let dentro = self.arvore.nos[n].inicio >= ci && self.arvore.nos[n].fim <= cf;
            if !dentro || !permitidas.contains(&n) {
                return None;
            }
        }
        for &outra in self.p.programa().unit(self.unidade).unit.declarations.iter() {
            let DeclKind::Class(ko) = &self.ast.decl(outra).kind else { continue };
            if ko.mixin_application || outra == d {
                continue;
            }
            let Some(c) = self.classe_da_declaracao(self.unidade, outra) else { return None };
            let ce = prog.class(c);
            if ce.supertype_class == Some(classe) || ce.interface_classes.contains(&classe) || ce.mixin_classes.contains(&classe) {
                return None;
            }
        }

        // `_computeUsedConstructors`.
        let mut usados: Vec<(FunctionElementId, usize)> = Vec::new();
        for c in campos.iter() {
            if usados.iter().any(|&(f, _)| f == c.construtor) {
                continue;
            }
            if let Some(&par) = construtores.iter().find(|&&(f, _)| f == c.construtor) {
                usados.push(par);
            }
        }
        // `_indexFieldData`.
        let mapa: Option<Vec<ParametroDoIndice>> = match &campo_do_indice {
            None => None,
            Some(_) => {
                let mut mapa = Vec::new();
                for &(f, i) in usados.iter() {
                    let MemberKind::Constructor(kc) = &self.ast.member(k.members[i]).kind else { return None };
                    let posicao = kc.parameters.iter().position(|p| p.this_ && p.name.is_some_and(|n| nome_de(n.span) == "index"))?;
                    mapa.push(ParametroDoIndice { construtor: f, membro: i, posicao });
                }
                let mut valores: Vec<i64> = Vec::new();
                for c in campos.iter() {
                    let par = mapa.iter().find(|p| p.construtor == c.construtor)?;
                    let MemberKind::Constructor(kc) = &self.ast.member(k.members[par.membro]).kind else { return None };
                    // `getArgument`: o argumento do parâmetro; o de um
                    // nomeado é a `NamedExpression`, que não é literal.
                    if kc.parameters[par.posicao].kind == ParameterKind::Named {
                        return None;
                    }
                    let argumentos = self.argumentos_da_lista(c.criacao);
                    let argumento = *argumentos.iter().filter(|&&a| self.especie(a) != "NamedExpression").nth(par.posicao)?;
                    if self.especie(argumento) != "IntegerLiteral" {
                        return None;
                    }
                    let v = self.valor_do_literal_inteiro(argumento)?;
                    if valores.contains(&v) {
                        return None;
                    }
                    valores.push(v);
                }
                valores.sort_unstable();
                if !(valores.len() == campos.len() && valores[0] == 0 && *valores.last()? == campos.len() as i64 - 1) {
                    return None;
                }
                Some(mapa)
            }
        };

        // `applyChanges`.
        let mut edicoes: Vec<(Span, String)> = Vec::new();
        let mut membros_a_excluir: Vec<usize> = Vec::new();
        let palavra_class = self
            .tokens
            .iter()
            .filter(|t| t.span.end <= k.name.span.start && &self.fonte[t.span.start..t.span.end] == "class")
            .map(|t| t.span)
            .next_back()?;
        edicoes.push((palavra_class, "enum".to_string()));
        if let Some(&extends) = filhos_da_classe.iter().find(|&&f| self.especie(f) == "ExtendsClause") {
            let seguinte = self.token_seguinte(self.arvore.nos[extends].fim)?;
            edicoes.push((Span { start: self.arvore.nos[extends].inicio, end: seguinte.start }, String::new()));
        }
        let eol = Texto::novo(self.fonte).eol();
        let recuo = UM_RECUO;
        let mut constantes = String::new();
        campos.sort_by_key(|c| c.indice);
        let excluir_campo = |campo: &CampoDaClasse, edicoes: &mut Vec<(Span, String)>, membros: &mut Vec<usize>| {
            if campo.variaveis.len() == 1 {
                membros.push(campo.membro);
            } else {
                edicoes.push((self.no_em_lista(&campo.variaveis, campo.variavel), String::new()));
            }
        };
        for campo in campos.iter() {
            let documentacao = self.filhos(nos_dos_membros[campo.campo.membro]).iter().copied().find(|&f| self.especie(f) == "Comment");
            if !constantes.is_empty() {
                constantes.push(',');
                constantes.push_str(eol);
                if documentacao.is_some() {
                    constantes.push_str(eol);
                }
                constantes.push_str(recuo);
            }
            if let Some(doc) = documentacao {
                constantes.push_str(self.texto_do_no(doc));
                constantes.push_str(eol);
                constantes.push_str(recuo);
            }
            constantes.push_str(&campo.nome);
            let parametro = mapa.as_ref().and_then(|m| m.iter().find(|p| p.construtor == campo.construtor));
            let nome_do_construtor = self.filhos(campo.criacao).iter().copied().find(|&f| self.especie(f) == "ConstructorName")?;
            let filhos_do_nome = self.filhos(nome_do_construtor);
            let tipo = *filhos_do_nome.first()?;
            let argumentos_de_tipo = self.filhos(tipo).iter().copied().find(|&f| self.especie(f) == "TypeArgumentList");
            if let Some(a) = argumentos_de_tipo {
                constantes.push_str(self.texto_do_no(a));
            }
            let nome = filhos_do_nome.get(1).copied().filter(|&f| self.especie(f) == "SimpleIdentifier");
            if let Some(n) = nome {
                constantes.push('.');
                constantes.push_str(self.texto_do_no(n));
            }
            let lista = self.filhos(campo.criacao).iter().copied().find(|&f| self.especie(f) == "ArgumentList")?;
            let argumentos = self.argumentos_da_lista(campo.criacao);
            let quantos = argumentos.len() - usize::from(parametro.is_some());
            if quantos == 0 {
                if argumentos_de_tipo.is_some() || nome.is_some() {
                    constantes.push_str("()");
                }
            } else if let Some(par) = parametro {
                constantes.push('(');
                let indice = par.posicao;
                let ultimo = argumentos.len() - 1;
                let ini = |a: usize| self.arvore.nos[a].inicio;
                let fim_de = |a: usize| self.arvore.nos[a].fim;
                if indice == 0 {
                    constantes.push_str(&self.fonte[ini(argumentos[1])..fim_de(argumentos[ultimo])]);
                } else if indice == ultimo {
                    let virgula = self.token_seguinte(fim_de(argumentos[ultimo])).is_some_and(|t| &self.fonte[t.start..t.end] == ",");
                    let ate = if virgula { ini(argumentos[ultimo]) } else { fim_de(argumentos[ultimo - 1]) };
                    constantes.push_str(&self.fonte[ini(argumentos[0])..ate]);
                } else {
                    constantes.push_str(&self.fonte[ini(argumentos[0])..ini(argumentos[indice])]);
                    let fecha = self.arvore.nos[lista].fim - 1;
                    constantes.push_str(&self.fonte[ini(argumentos[indice + 1])..fecha]);
                }
                constantes.push(')');
            } else {
                constantes.push_str(self.texto_do_no(lista));
            }
            excluir_campo(&campo.campo, &mut edicoes, &mut membros_a_excluir);
        }
        if let Some(indice) = &campo_do_indice {
            excluir_campo(indice, &mut edicoes, &mut membros_a_excluir);
        }

        // `_removeUnnamedConstructor`.
        let mut removido: Option<usize> = None;
        let declarados: Vec<usize> = (0..k.members.len()).filter(|&i| matches!(self.ast.member(k.members[i]).kind, MemberKind::Constructor(_))).collect();
        if let [i] = declarados[..]
            && let MemberKind::Constructor(kc) = &self.ast.member(k.members[i]).kind
            && kc.name.is_none_or(|n| nome_de(n.span) == "new")
        {
            let restantes = kc.parameters.len() as i64 - i64::from(mapa.as_ref().is_some_and(|m| !m.is_empty()));
            if restantes == 0 {
                membros_a_excluir.push(i);
                removido = Some(i);
            }
        }
        // `_transformConstructors`.
        if let Some(mapa) = &mapa {
            for par in mapa.iter() {
                if removido == Some(par.membro) {
                    continue;
                }
                let lista = self.filhos(nos_dos_membros[par.membro]).iter().copied().find(|&f| self.especie(f) == "FormalParameterList")?;
                let parametros = self.filhos(lista).to_vec();
                edicoes.push((self.no_em_lista(&parametros, *parametros.get(par.posicao)?), String::new()));
            }
        }

        let titulo = "Convert class to an enum";
        let especie = "refactor.convert.classToEnum";
        let (primeiro, ultimo) = (*nos_dos_membros.first()?, *nos_dos_membros.last()?);
        if membros_a_excluir.len() == nos_dos_membros.len() {
            edicoes.push((Span { start: self.arvore.nos[primeiro].inicio, end: self.arvore.nos[ultimo].fim }, constantes));
            return Some(self.acao_simples(uri, titulo, especie, edicoes));
        }
        let chave = self.token_anterior(self.arvore.nos[primeiro].inicio)?;
        if &self.fonte[chave.start..chave.end] != "{" {
            return None;
        }
        edicoes.push((Span { start: chave.end, end: chave.end }, format!("{eol}{recuo}{constantes};{eol}")));
        // `range.nodesInList(members, membersToDelete)`.
        membros_a_excluir.sort_unstable();
        let mut grupos_de_indices: Vec<(usize, usize)> = Vec::new();
        for &i in membros_a_excluir.iter() {
            match grupos_de_indices.last_mut() {
                Some((_, sup)) if *sup + 1 == i => *sup = i,
                _ => grupos_de_indices.push((i, i)),
            }
        }
        for (inf, sup) in grupos_de_indices {
            let span = if inf == 0 {
                Span { start: self.arvore.nos[nos_dos_membros[inf]].inicio, end: self.arvore.nos[*nos_dos_membros.get(sup + 1)?].inicio }
            } else {
                Span { start: self.arvore.nos[nos_dos_membros[inf - 1]].fim, end: self.arvore.nos[nos_dos_membros[sup]].fim }
            };
            edicoes.push((span, String::new()));
        }
        Some(self.acao_simples(uri, titulo, especie, edicoes))
    }
}
