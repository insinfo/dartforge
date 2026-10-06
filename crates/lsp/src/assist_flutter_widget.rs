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

impl Contexto<'_> {
    /// `_isDefaultOverride`: o corpo vazio, ou só `super.m(…)` do mesmo nome.
    fn sobrescrita_padrao(&self, metodo: usize) -> bool {
        let Some(&corpo) = self.filhos(metodo).last() else { return false };
        let Marca::Funcao(fid) = self.arvore.nos[metodo].marca else { return false };
        let Some(nome) = self.ast.function(fid).name else { return false };
        let expressao = match self.especie(corpo) {
            "BlockFunctionBody" => {
                let Some(&bloco) = self.filhos(corpo).first() else { return false };
                let comandos = self.filhos(bloco);
                if comandos.is_empty() {
                    return true;
                }
                let [c] = comandos[..] else { return false };
                if self.especie(c) != "ExpressionStatement" {
                    return false;
                }
                let Some(&e) = self.filhos(c).first() else { return false };
                e
            }
            "ExpressionFunctionBody" => {
                let Some(&e) = self.filhos(corpo).first() else { return false };
                e
            }
            _ => return false,
        };
        self.especie(expressao) == "MethodInvocation"
            && self.filhos(expressao).first().is_some_and(|&a| self.especie(a) == "SuperExpression")
            && self.nome_do_metodo(expressao).is_some_and(|m| self.texto_do_no(m) == self.p.nome(nome.sym))
    }

    /// `_isState`: `State<W>` (a classe `State` exata) com o widget como único
    /// argumento.
    fn e_state_de(&self, t: dartforge_types::TypeId, widget: ClassId) -> bool {
        let Type::Interface { class, args, .. } = self.p.consulta.tabela.get(t) else { return false };
        let [a] = args[..] else { return false };
        matches!(self.p.consulta.tabela.get(a), Type::Interface { class: w, .. } if *w == widget) && self.flutter_exatamente(*class, "State", URI_FRAMEWORK)
    }

    /// `FlutterConvertToStatelessWidget`.
    pub(crate) fn flutter_para_stateless(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
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
        let widget = self.classe_da_declaracao(self.unidade, d)?;
        let p = self.p.programa();
        let consulta = &self.p.consulta;
        let superclasse = consulta.outline.classes.get(widget.0 as usize)?.supertype?;
        let Type::Interface { class: sc, .. } = consulta.tabela.get(superclasse) else { return None };
        if !self.flutter_exatamente(*sc, "StatefulWidget", URI_FRAMEWORK) {
            return None;
        }
        let nos_widget = self.nos_dos_membros(classe_no, &k.members)?;
        // `_findCreateStateMethod`: o primeiro `createState`, sem parâmetros.
        let mut criar_state = None;
        for (i, &m) in k.members.iter().enumerate() {
            if let MemberKind::Method(fid) = self.ast.member(m).kind
                && self.ast.function(fid).name.is_some_and(|n| self.p.nome(n.sym) == "createState")
            {
                if self.ast.function(fid).parameters.as_ref().is_some_and(|ps| ps.is_empty()) {
                    criar_state = Some(nos_widget[i]);
                }
                break;
            }
        }
        let criar_state = criar_state?;
        // `_findStateClass`.
        let unidade = &p.unit(self.unidade).unit;
        let (decl_state, state) = unidade.declarations.iter().copied().find_map(|x| {
            let DeclKind::Class(c) = &self.ast.decl(x).kind else { return None };
            c.extends?;
            let cid = self.classe_da_declaracao(self.unidade, x)?;
            let st = consulta.outline.classes.get(cid.0 as usize)?.supertype?;
            self.e_state_de(st, widget).then_some((x, cid))
        })?;
        let DeclKind::Class(ks) = &self.ast.decl(decl_state).kind else { return None };
        if !self.p.nome(ks.name.sym).starts_with('_') || !self.mesmos_parametros_de_tipo(k, ks) {
            return None;
        }
        let state_no = (0..self.arvore.nos.len()).find(|&n| self.arvore.nos[n].marca == Marca::Decl(decl_state) && self.especie(n) == "ClassDeclaration")?;
        let nos_state = self.nos_dos_membros(state_no, &ks.members)?;
        // `_StatelessVerifier` e `_FieldFinder`.
        let mut atribuidos: HashSet<VariableId> = HashSet::new();
        let campo_de_nome = |nome: &str| p.class(state).fields.iter().copied().find(|&v| self.p.nome(p.variable(v).name) == nome);
        for (i, &m) in ks.members.iter().enumerate() {
            match &self.ast.member(m).kind {
                MemberKind::Constructor(kc) => {
                    for ini in kc.initializers.iter() {
                        if let ast::Initializer::Field { name, .. } = ini
                            && let Some(v) = campo_de_nome(self.p.nome(name.sym))
                        {
                            atribuidos.insert(v);
                        }
                    }
                    atribuidos.extend(self.escritas_de_campo(nos_state[i]));
                }
                MemberKind::Method(_) => {
                    if !self.pode_ser_stateless(nos_state[i]) {
                        return None;
                    }
                }
                _ => {}
            }
        }
        // `_StateUsageVisitor`.
        if self.state_usado(widget, state) {
            return None;
        }
        // Os membros a mover.
        let mut a_mover: Vec<usize> = Vec::new();
        for (i, &m) in ks.members.iter().enumerate() {
            match &self.ast.member(m).kind {
                MemberKind::Field(l) => {
                    if l.static_ {
                        return None;
                    }
                    for (j, _) in l.variables.iter().enumerate() {
                        let v = p.class(state).fields.iter().copied().find(|&v| p.variable(v).node == dartforge_elements::model::VariableRef::Field { unit: self.unidade, member: m, index: j });
                        if v.is_none_or(|v| !atribuidos.contains(&v)) {
                            a_mover.push(nos_state[i]);
                        }
                    }
                }
                MemberKind::Method(fid) => {
                    if self.ast.function(*fid).static_ {
                        return None;
                    }
                    if !self.sobrescrita_padrao(nos_state[i]) {
                        a_mover.push(nos_state[i]);
                    }
                }
                _ => {}
            }
        }
        let classe_stateless = self.classe_flutter("package:flutter/widgets.dart", "StatelessWidget")?;
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, palavra.start);
        let mut m = Mudanca::default();
        m.adicionar(uri, p.unit(self.unidade).ast.ty(extends).span, escritor.referencia_de(Element::Class(classe_stateless), "StatelessWidget"));
        m.adicionar(uri, self.faixa_de_exclusao(state_no, None)?, "");
        let seguinte = self.token_seguinte(self.arvore.nos[criar_state].fim)?;
        let depois = self.comentarios_antes(seguinte).first().map_or(seguinte.start, |c| c.start);
        let faixa = Span { start: tx.inicio_do_conteudo(self.arvore.nos[criar_state].inicio), end: tx.inicio_do_conteudo(depois) };
        let nova_linha = &self.fonte[seguinte.start..seguinte.end] != "}";
        let mut s = String::new();
        for (i, &n) in a_mover.iter().enumerate() {
            if let Some(c) = self.comentarios_antes(Span { start: self.arvore.nos[n].inicio, end: self.arvore.nos[n].inicio }).first() {
                let de = tx.inicio_do_conteudo(c.start);
                s.push_str(&self.fonte[de..c.end]);
                s.push_str(eol);
            }
            s.push_str(&self.reescrever_para_widget(n, widget));
            if nova_linha || i + 1 < a_mover.len() {
                s.push_str(eol);
            }
        }
        m.adicionar(uri, faixa, s);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &escritor.importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to StatelessWidget".into(),
            especie: "refactor.flutter.convert.toStatelessWidget".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `_isSameTypeParameters`.
    fn mesmos_parametros_de_tipo(&self, widget: &ast::ClassDecl, state: &ast::ClassDecl) -> bool {
        if widget.type_params.is_empty() && state.type_params.is_empty() {
            return true;
        }
        if widget.type_params.is_empty() || state.type_params.is_empty() || widget.type_params.len() < state.type_params.len() {
            return false;
        }
        let limite = |t: &ast::TypeParameter| t.bound.map(|b| self.p.programa().unit(self.unidade).source[self.ast.ty(b).span.start..self.ast.ty(b).span.end].to_string());
        state.type_params.iter().all(|s| widget.type_params.iter().any(|w| self.p.nome(w.name.sym) == self.p.nome(s.name.sym) && limite(w) == limite(s)))
    }

    /// As escritas de campo (`inSetterContext`) no nó.
    fn escritas_de_campo(&self, n: usize) -> Vec<VariableId> {
        let p = self.p.programa();
        let (a, b) = (self.arvore.nos[n].inicio, self.arvore.nos[n].fim);
        let mut v = Vec::new();
        for no in self.arvore.nos.iter() {
            if no.especie == "SimpleIdentifier" && no.inicio >= a && no.fim <= b
                && let Marca::Expr(x) = no.marca
                && self.escritas.contains(&x)
            {
                match self.corpos.get_resolved(x) {
                    Some(Resolved::Member { member: MemberRef::Variable(var), .. }) => v.push(*var),
                    Some(Resolved::Member { member: MemberRef::Function(f), .. }) => v.extend(p.function(*f).variable),
                    _ => {}
                }
            }
        }
        v
    }

    /// `_StatelessVerifier`: nenhuma chamada a membro da classe `State`
    /// fora de uma sobrescrita padrão.
    fn pode_ser_stateless(&self, metodo: usize) -> bool {
        let p = self.p.programa();
        let (a, b) = (self.arvore.nos[metodo].inicio, self.arvore.nos[metodo].fim);
        for (n, no) in self.arvore.nos.iter().enumerate() {
            if no.especie != "MethodInvocation" || no.inicio < a || no.fim > b {
                continue;
            }
            let Some(nome) = self.nome_do_metodo(n) else { continue };
            let Some((Executavel::Funcao(f), Some(c), _)) = self.executavel_do_identificador(nome) else { continue };
            let _ = f;
            if matches!(p.class(c).kind, dartforge_elements::model::ClassKind::Class) && self.flutter_exatamente(c, "State", URI_FRAMEWORK) {
                let dono = self.com_pais(n).find(|&k| self.especie(k) == "MethodDeclaration");
                if !dono.is_some_and(|d| self.sobrescrita_padrao(d)) {
                    return false;
                }
            }
        }
        true
    }

    /// `_StateUsageVisitor`: a classe `State` criada fora do `createState`
    /// do widget, ou um `createState()` chamado.
    fn state_usado(&self, widget: ClassId, state: ClassId) -> bool {
        for (n, no) in self.arvore.nos.iter().enumerate() {
            match no.especie {
                "InstanceCreationExpression" => {
                    let Some(t) = self.tipo_do_no_flutter(n) else { continue };
                    if self.classe_do_tipo_flutter(t) != Some(state) {
                        continue;
                    }
                    let metodo = self.com_pais(n).find(|&k| self.especie(k) == "MethodDeclaration");
                    let classe = metodo.and_then(|m| self.com_pais(m).find(|&k| self.especie(k) == "ClassDeclaration"));
                    let no_create_state = metodo.and_then(|m| match self.arvore.nos[m].marca {
                        Marca::Funcao(f) => self.ast.function(f).name.map(|x| self.p.nome(x.sym) == "createState"),
                        _ => None,
                    }) == Some(true);
                    let do_widget = classe.and_then(|c| match self.arvore.nos[c].marca {
                        Marca::Decl(d) => self.classe_da_declaracao(self.unidade, d),
                        _ => None,
                    }) == Some(widget);
                    if !no_create_state || !do_widget {
                        return true;
                    }
                }
                "MethodInvocation" => {
                    if self.nome_do_metodo(n).is_some_and(|m| self.texto_do_no(m) == "createState")
                        && let Some(t) = self.tipo_do_no_flutter(n)
                        && (self.e_state_de(t, widget) || self.classe_do_tipo_flutter(t) == Some(state))
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// `rewriteWidgetMemberReferences` do StatelessWidget: sem o `widget.`
    /// (ou o `W.`) antes das referências aos membros do widget.
    fn reescrever_para_widget(&self, n: usize, widget: ClassId) -> String {
        let tx = Texto::novo(self.fonte);
        let linhas = tx.faixa_de_linhas(self.arvore.nos[n].inicio, self.arvore.nos[n].fim);
        let base = linhas.start;
        let mut edicoes: Vec<(usize, usize, String)> = Vec::new();
        let (a, b) = (self.arvore.nos[n].inicio, self.arvore.nos[n].fim);
        for (k, no) in self.arvore.nos.iter().enumerate() {
            if no.especie != "SimpleIdentifier" || no.inicio < a || no.fim > b {
                continue;
            }
            let Some((_, dono, _)) = self.executavel_do_identificador(k) else { continue };
            if dono != Some(widget) {
                continue;
            }
            let Some(pai) = self.pai(k) else { continue };
            match self.especie(pai) {
                "PrefixedIdentifier" if self.filhos(pai).last() == Some(&k) => {
                    let prefixo = *self.filhos(pai).first().unwrap();
                    let avo = self.pai(pai).filter(|&g| self.especie(g) == "InterpolationExpression");
                    let mut fecha = None;
                    if !self.texto_do_no(k).contains('$')
                        && let Some(g) = avo
                        && self.fonte[self.arvore.nos[g].inicio..].starts_with("${")
                    {
                        let abre = self.arvore.nos[g].inicio + 2;
                        edicoes.push((abre - 1 - base, 1, String::new()));
                        let fim_g = self.arvore.nos[g].fim;
                        if self.fonte[..fim_g].ends_with('}') {
                            fecha = Some((fim_g - 1 - base, 1, String::new()));
                        }
                    }
                    let ini = self.arvore.nos[prefixo].inicio;
                    let ponto = self.token_seguinte(self.arvore.nos[prefixo].fim).map_or(self.arvore.nos[prefixo].fim, |t| t.end);
                    edicoes.push((ini - base, ponto - ini, String::new()));
                    if let Some(f) = fecha {
                        edicoes.push(f);
                    }
                }
                "MethodInvocation" | "PropertyAccess" if self.filhos(pai).len() > 1 && self.filhos(pai).first() != Some(&k) => {
                    let alvo = *self.filhos(pai).first().unwrap();
                    let ini = self.arvore.nos[alvo].inicio;
                    let operador = self.token_anterior(no.inicio).map_or(no.inicio, |t| t.end);
                    edicoes.push((ini - base, operador - ini, String::new()));
                }
                _ => {}
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
