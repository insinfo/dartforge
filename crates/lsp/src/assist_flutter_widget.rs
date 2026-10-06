//! `FlutterConvertToStatefulWidget` e `FlutterConvertToStatelessWidget` do
//! `analysis_server` 3.6.2 (flutter_convert_to_stateful_widget.dart,
//! flutter_convert_to_stateless_widget.dart).
//!
//! | Título | Espécie |
//! |---|---|
//! | `Convert to StatefulWidget` | `refactor.flutter.convert.toStatefulWidget` |
//! | `Convert to StatelessWidget` | `refactor.flutter.convert.toStatelessWidget` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::{Ligacao, Marca};
use crate::assist_flutter::URI_FRAMEWORK;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Mudanca, Texto};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, VariableId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use dartforge_types::{MemberRef, Resolved, Type};
use std::collections::HashSet;

/// Um elemento executável da classe do widget (`ExecutableElement`): um
/// método ou acessor declarado, ou o acessor implícito de um campo.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Executavel {
    Funcao(dartforge_elements::model::FunctionElementId),
    Campo(VariableId),
}

impl Contexto<'_> {
    /// O `{` do corpo de uma declaração de classe (o par do `}` final).
    fn abre_corpo(&self, classe_no: usize) -> Option<Span> {
        let fim = self.arvore.nos[classe_no].fim;
        let i_fim = self.tokens.iter().position(|t| t.span.end == fim)?;
        let mut nivel = 0i32;
        for k in (self.tokens.iter().position(|t| t.span.start >= self.arvore.nos[classe_no].inicio)?..=i_fim).rev() {
            match &self.fonte[self.tokens[k].span.start..self.tokens[k].span.end] {
                "}" => nivel += 1,
                "{" => {
                    nivel -= 1;
                    if nivel == 0 {
                        return Some(self.tokens[k].span);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// O `class` da declaração.
    fn palavra_class(&self, nome: Span) -> Option<Span> {
        self.tokens.iter().filter(|t| t.span.end <= nome.start && &self.fonte[t.span.start..t.span.end] == "class").map(|t| t.span).next_back()
    }

    /// Os nós de membro de uma `ClassDeclaration`, na ordem dos membros.
    fn nos_dos_membros(&self, classe_no: usize, membros: &[ast::MemberId]) -> Option<Vec<usize>> {
        membros
            .iter()
            .map(|&m| self.filhos(classe_no).iter().copied().find(|&f| self.arvore.nos[f].ligacao == Ligacao::Membro(m)))
            .collect()
    }

    /// O elemento estático (`staticElement`) executável de um
    /// identificador: o membro ou o acessor implícito do campo. `None` numa
    /// escrita (o `staticElement` do alvo de uma atribuição é nulo).
    fn executavel_do_identificador(&self, n: usize) -> Option<(Executavel, Option<ClassId>, bool)> {
        let Marca::Expr(x) = self.arvore.nos[n].marca else { return None };
        if self.escritas.contains(&x) {
            return None;
        }
        let p = self.p.programa();
        match self.corpos.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => {
                let fe = p.function(*f);
                match (fe.kind, fe.variable) {
                    (FunctionKind::ImplicitAccessor, Some(v)) => Some((Executavel::Campo(v), fe.class, fe.static_)),
                    (FunctionKind::Constructor | FunctionKind::SyntheticConstructor, _) => None,
                    _ => Some((Executavel::Funcao(*f), fe.class, fe.static_)),
                }
            }
            Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => {
                let ve = p.variable(*v);
                Some((Executavel::Campo(*v), ve.class, ve.static_))
            }
            _ => None,
        }
    }

    /// `_FieldFinder`: os campos atribuídos nos construtores (`this.x`, o
    /// inicializador `x = …` e as escritas).
    fn campos_atribuidos_nos_construtores(&self, classe: ClassId, membros: &[ast::MemberId], nos: &[usize]) -> HashSet<VariableId> {
        let p = self.p.programa();
        let campo_de_nome = |nome: &str| p.class(classe).fields.iter().copied().find(|&v| self.p.nome(p.variable(v).name) == nome);
        let mut saida = HashSet::new();
        for (i, &m) in membros.iter().enumerate() {
            let MemberKind::Constructor(k) = &self.ast.member(m).kind else { continue };
            for q in k.parameters.iter() {
                if q.this_
                    && let Some(n) = q.name
                    && let Some(v) = campo_de_nome(self.p.nome(n.sym))
                {
                    saida.insert(v);
                }
            }
            for ini in k.initializers.iter() {
                if let ast::Initializer::Field { name, .. } = ini
                    && let Some(v) = campo_de_nome(self.p.nome(name.sym))
                {
                    saida.insert(v);
                }
            }
            // As escritas no corpo (`inSetterContext`).
            let (a, b) = (self.arvore.nos[nos[i]].inicio, self.arvore.nos[nos[i]].fim);
            for no in self.arvore.nos.iter() {
                if no.especie == "SimpleIdentifier"
                    && no.inicio >= a
                    && no.fim <= b
                    && let Marca::Expr(x) = no.marca
                    && self.escritas.contains(&x)
                {
                    match self.corpos.get_resolved(x) {
                        Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                            saida.insert(*v);
                        }
                        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                            if let Some(v) = p.function(*f).variable
                                && p.variable(v).class.is_some()
                            {
                                saida.insert(v);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        saida
    }

    /// `FlutterConvertToStatefulWidget`.
    pub(crate) fn flutter_para_stateful(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let classe_no = self.com_pais(no).find(|&k| self.especie(k) == "ClassDeclaration")?;
        let Marca::Decl(d) = self.arvore.nos[classe_no].marca else { return None };
        let DeclKind::Class(k) = &self.ast.decl(d).kind else { return None };
        let extends = k.extends?;
        let palavra = self.palavra_class(k.name.span)?;
        let abre = self.abre_corpo(classe_no)?;
        if inicio < palavra.start || inicio > abre.end {
            return None;
        }
        let classe = self.classe_da_declaracao(self.unidade, d)?;
        let p = self.p.programa();
        let consulta = &self.p.consulta;
        let superclasse = consulta.outline.classes.get(classe.0 as usize)?.supertype?;
        let Type::Interface { class: sc, .. } = consulta.tabela.get(superclasse) else { return None };
        if !self.flutter_exatamente(*sc, "StatelessWidget", URI_FRAMEWORK) {
            return None;
        }
        let nos = self.nos_dos_membros(classe_no, &k.members)?;
        // `_findBuildMethod`.
        let build = k.members.iter().position(|&m| match self.ast.member(m).kind {
            MemberKind::Method(fid) => {
                let f = self.ast.function(fid);
                f.name.is_some_and(|n| self.p.nome(n.sym) == "build") && f.parameters.as_ref().is_some_and(|ps| ps.len() == 1)
            }
            _ => false,
        })?;
        let nome_widget = self.p.nome(k.name.sym).to_string();
        let nome_state = if nome_widget.starts_with('_') { format!("{nome_widget}State") } else { format!("_{nome_widget}State") };
        let atribuidos = self.campos_atribuidos_nos_construtores(classe, &k.members, &nos);
        // Os membros a mover e os elementos deles.
        let mut a_mover: Vec<usize> = Vec::new();
        let mut movidos: HashSet<Executavel> = HashSet::new();
        for (i, &m) in k.members.iter().enumerate() {
            match &self.ast.member(m).kind {
                MemberKind::Field(l) if !l.static_ => {
                    for (j, _) in l.variables.iter().enumerate() {
                        let Some(v) = p.class(classe).fields.iter().copied().find(|&v| p.variable(v).node == dartforge_elements::model::VariableRef::Field { unit: self.unidade, member: m, index: j }) else { continue };
                        if !atribuidos.contains(&v) {
                            if !a_mover.contains(&i) {
                                a_mover.push(i);
                            }
                            movidos.insert(Executavel::Campo(v));
                        }
                    }
                }
                MemberKind::Method(fid) if !self.ast.function(*fid).static_ => {
                    a_mover.push(i);
                    if let Some(f) = self.p.funcao_do_no(self.unidade, *fid) {
                        movidos.insert(Executavel::Funcao(f));
                    }
                }
                _ => {}
            }
        }
        let classe_state = self.classe_flutter("package:flutter/widgets.dart", "State")?;
        let classe_stateful = self.classe_flutter("package:flutter/widgets.dart", "StatefulWidget")?;
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, palavra.start);
        let mut m = Mudanca::default();
        m.adicionar(uri, p.unit(self.unidade).ast.ty(extends).span, escritor.referencia_de(Element::Class(classe_stateful), "StatefulWidget"));
        let parametros_de_tipo = self.filhos(classe_no).iter().copied().find(|&f| self.especie(f) == "TypeParameterList").map(|f| self.texto_do_no(f).to_string()).unwrap_or_default();
        let ref_state = escritor.referencia_de(Element::Class(classe_state), "State");
        let nome_cru = self.p.nome(k.name.sym).to_string();
        // `replaceInterval`.
        let criar_state = |antes: bool, depois: bool| -> String {
            let mut s = String::new();
            if antes {
                s.push_str(eol);
            }
            s.push_str(&format!("  @override{eol}"));
            s.push_str(&format!("  {ref_state}<{nome_cru}{parametros_de_tipo}> createState() => {nome_state}{parametros_de_tipo}();{eol}"));
            if depois {
                s.push_str(eol);
            }
            s
        };
        let mut inicio_troca = 0usize;
        let mut tem_build = false;
        let mut ultimo_removido_campo = false;
        let mut fim_do_ultimo_mantido = 0usize;
        for (i, &n) in nos.iter().enumerate() {
            if a_mover.contains(&i) {
                if inicio_troca == 0 {
                    let comeco = self.comentarios_antes(Span { start: self.arvore.nos[n].inicio, end: self.arvore.nos[n].inicio }).first().map_or(self.arvore.nos[n].inicio, |c| c.start);
                    inicio_troca = tx.inicio_do_conteudo(comeco);
                }
                if i == build {
                    tem_build = true;
                }
                ultimo_removido_campo = matches!(self.ast.member(k.members[i]).kind, MemberKind::Field(_));
            } else {
                let linhas = tx.faixa_de_linhas(self.arvore.nos[n].inicio, self.arvore.nos[n].fim);
                fim_do_ultimo_mantido = linhas.end;
                if inicio_troca != 0 {
                    let texto = if tem_build {
                        tem_build = false;
                        criar_state(false, true)
                    } else if ultimo_removido_campo && !matches!(self.ast.member(k.members[i]).kind, MemberKind::Field(_)) {
                        eol.to_string()
                    } else {
                        String::new()
                    };
                    m.adicionar(uri, Span { start: inicio_troca, end: linhas.start }, texto);
                    inicio_troca = 0;
                }
            }
        }
        if inicio_troca != 0 {
            if fim_do_ultimo_mantido != 0 {
                inicio_troca = fim_do_ultimo_mantido;
            }
            let fecha = self.arvore.nos[classe_no].fim - 1;
            let texto = if tem_build { criar_state(fim_do_ultimo_mantido != 0, false) } else { String::new() };
            m.adicionar(uri, Span { start: inicio_troca, end: fecha }, texto);
        }
        // A classe `State`.
        let mut s = format!("{eol}{eol}class {nome_state}{parametros_de_tipo} extends {ref_state}<{nome_cru}");
        if !k.type_params.is_empty() {
            // Só os nomes, sem separador (o `first` do Dart nunca vira
            // falso depois do primeiro).
            s.push('<');
            for t in k.type_params.iter() {
                s.push_str(self.p.nome(t.name.sym));
            }
            s.push('>');
        }
        s.push_str(&format!("> {{{eol}"));
        let mut primeira = true;
        for &i in &a_mover {
            if !primeira {
                s.push_str(eol);
            }
            let n = nos[i];
            if let Some(c) = self.comentarios_antes(Span { start: self.arvore.nos[n].inicio, end: self.arvore.nos[n].inicio }).first() {
                let de = tx.inicio_do_conteudo(c.start);
                s.push_str(&self.fonte[de..c.end]);
                s.push_str(eol);
            }
            s.push_str(&self.reescrever_para_state(n, classe, &movidos, &nome_cru));
            primeira = false;
        }
        s.push('}');
        let fim_classe = self.arvore.nos[classe_no].fim;
        m.adicionar(uri, Span { start: fim_classe, end: fim_classe }, s);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &escritor.importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to StatefulWidget".into(),
            especie: "refactor.flutter.convert.toStatefulWidget".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `rewriteWidgetMemberReferences` do StatefulWidget: `widget.` (ou o
    /// nome da classe, num estático) antes de cada referência a um membro do
    /// widget que fica.
    fn reescrever_para_state(&self, n: usize, classe: ClassId, movidos: &HashSet<Executavel>, nome_classe: &str) -> String {
        let tx = Texto::novo(self.fonte);
        let linhas = tx.faixa_de_linhas(self.arvore.nos[n].inicio, self.arvore.nos[n].fim);
        let mut edicoes: Vec<(usize, usize, String)> = Vec::new();
        let (a, b) = (self.arvore.nos[n].inicio, self.arvore.nos[n].fim);
        for (k, no) in self.arvore.nos.iter().enumerate() {
            if no.especie != "SimpleIdentifier" || no.inicio < a || no.fim > b {
                continue;
            }
            let Some((e, dono, estatico)) = self.executavel_do_identificador(k) else { continue };
            if dono != Some(classe) || movidos.contains(&e) {
                continue;
            }
            let o = no.inicio - linhas.start;
            let qualificador = if estatico { nome_classe } else { "widget" };
            let interpolado = self.pai(k).is_some_and(|p| self.especie(p) == "InterpolationExpression") && no.inicio > 0 && self.fonte.as_bytes()[no.inicio - 1] == b'$';
            if interpolado {
                edicoes.push((o, 0, format!("{{{qualificador}.")));
                edicoes.push((o + (no.fim - no.inicio), 0, "}".to_string()));
            } else {
                edicoes.push((o, 0, format!("{qualificador}.")));
            }
        }
        aplicar_em_sequencia(&self.fonte[linhas.start..linhas.end], edicoes)
    }
}

/// `SourceEdit.applySequence` das edições na ordem inversa da visita.
fn aplicar_em_sequencia(texto: &str, mut edicoes: Vec<(usize, usize, String)>) -> String {
    // A visita é em pré-ordem (posições crescentes).
    edicoes.sort_by_key(|e| e.0);
    let mut s = texto.to_string();
    for (o, n, t) in edicoes.into_iter().rev() {
        s.replace_range(o..o + n, &t);
    }
    s
}
