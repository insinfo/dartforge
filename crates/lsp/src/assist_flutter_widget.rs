//! `FlutterConvertToStatefulWidget` e `FlutterConvertToStatelessWidget` do
//! `analysis_server` 3.6.2 (flutter_convert_to_stateful_widget.dart,
//! flutter_convert_to_stateless_widget.dart).
//!
//! | Título | Espécie |
//! |---|---|
//! | `Convert to StatefulWidget` | `refactor.flutter.convert.toStatefulWidget` |
//! | `Convert to StatelessWidget` | `refactor.flutter.convert.toStatelessWidget` |

use crate::actions::CodeAction;
use crate::arvore_analyzer::{Ligacao, Marca};
use crate::assist_flutter::URI_FRAMEWORK;
use crate::refactorings::Context;
use crate::refactorings_exec::{Change, Text};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, VariableId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use dartforge_types::{MemberRef, Resolved, Type};
use std::collections::HashSet;

/// Um elemento executável da classe do widget (`ExecutableElement`): um
/// método ou acessor declarado, ou o acessor implícito de um campo.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Executable {
    Function(dartforge_elements::model::FunctionElementId),
    Field(VariableId),
}

impl Context<'_> {
    /// O `{` do corpo de uma declaração de classe (o par do `}` final).
    fn opens_body(&self, class_node: usize) -> Option<Span> {
        let end = self.tree.nos[class_node].fim;
        let i_end = self.tokens.iter().position(|t| t.span.end == end)?;
        let mut level = 0i32;
        for k in (self.tokens.iter().position(|t| t.span.start >= self.tree.nos[class_node].inicio)?..=i_end).rev() {
            match &self.source[self.tokens[k].span.start..self.tokens[k].span.end] {
                "}" => level += 1,
                "{" => {
                    level -= 1;
                    if level == 0 {
                        return Some(self.tokens[k].span);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// O `class` da declaração.
    fn class_word(&self, name: Span) -> Option<Span> {
        self.tokens.iter().filter(|t| t.span.end <= name.start && &self.source[t.span.start..t.span.end] == "class").map(|t| t.span).next_back()
    }

    /// Os nós de membro de uma `ClassDeclaration`, na ordem dos membros.
    fn member_nodes(&self, class_node: usize, members: &[ast::MemberId]) -> Option<Vec<usize>> {
        members
            .iter()
            .map(|&m| self.children(class_node).iter().copied().find(|&f| self.tree.nos[f].ligacao == Ligacao::Membro(m)))
            .collect()
    }

    /// O elemento estático (`staticElement`) executável de um
    /// identificador: o membro ou o acessor implícito do campo. `None` numa
    /// escrita (o `staticElement` do alvo de uma atribuição é nulo).
    fn identifier_executable(&self, n: usize) -> Option<(Executable, Option<ClassId>, bool)> {
        let Marca::Expr(x) = self.tree.nos[n].marca else { return None };
        if self.writes.contains(&x) {
            return None;
        }
        let p = self.p.program();
        match self.bodies.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => {
                let fe = p.function(*f);
                match (fe.kind, fe.variable) {
                    (FunctionKind::ImplicitAccessor, Some(v)) => Some((Executable::Field(v), fe.class, fe.static_)),
                    (FunctionKind::Constructor | FunctionKind::SyntheticConstructor, _) => None,
                    _ => Some((Executable::Function(*f), fe.class, fe.static_)),
                }
            }
            Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => {
                let ve = p.variable(*v);
                Some((Executable::Field(*v), ve.class, ve.static_))
            }
            _ => None,
        }
    }

    /// `_FieldFinder`: os campos atribuídos nos construtores (`this.x`, o
    /// inicializador `x = …` e as escritas).
    fn fields_assigned_in_constructors(&self, class: ClassId, members: &[ast::MemberId], nos: &[usize]) -> HashSet<VariableId> {
        let p = self.p.program();
        let name_field = |name_: &str| p.class(class).fields.iter().copied().find(|&v| self.p.name(p.variable(v).name) == name_);
        let mut output = HashSet::new();
        for (i, &m) in members.iter().enumerate() {
            let MemberKind::Constructor(k) = &self.ast.member(m).kind else { continue };
            for q in k.parameters.iter() {
                if q.this_
                    && let Some(n) = q.name
                    && let Some(v) = name_field(self.p.name(n.sym))
                {
                    output.insert(v);
                }
            }
            for start in k.initializers.iter() {
                if let ast::Initializer::Field { name, .. } = start
                    && let Some(v) = name_field(self.p.name(name.sym))
                {
                    output.insert(v);
                }
            }
            // As escritas no corpo (`inSetterContext`).
            let (a, b) = (self.tree.nos[nos[i]].inicio, self.tree.nos[nos[i]].fim);
            for no in self.tree.nos.iter() {
                if no.especie == "SimpleIdentifier"
                    && no.inicio >= a
                    && no.fim <= b
                    && let Marca::Expr(x) = no.marca
                    && self.writes.contains(&x)
                {
                    match self.bodies.get_resolved(x) {
                        Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                            output.insert(*v);
                        }
                        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                            if let Some(v) = p.function(*f).variable
                                && p.variable(v).class.is_some()
                            {
                                output.insert(v);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        output
    }

    /// `FlutterConvertToStatefulWidget`.
    pub(crate) fn flutter_to_stateful(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let class_node = self.with_parents(no).find(|&k| self.kind(k) == "ClassDeclaration")?;
        let Marca::Decl(d) = self.tree.nos[class_node].marca else { return None };
        let DeclKind::Class(k) = &self.ast.decl(d).kind else { return None };
        let extends = k.extends?;
        let word = self.class_word(k.name.span)?;
        let opens = self.opens_body(class_node)?;
        if start < word.start || start > opens.end {
            return None;
        }
        let class = self.declaration_class(self.unit, d)?;
        let p = self.p.program();
        let query = &self.p.query;
        let superclass = query.outline.classes.get(class.0 as usize)?.supertype?;
        let Type::Interface { class: sc, .. } = query.table.get(superclass) else { return None };
        if !self.flutter_exactly(*sc, "StatelessWidget", URI_FRAMEWORK) {
            return None;
        }
        let nos = self.member_nodes(class_node, &k.members)?;
        // `_findBuildMethod`.
        let build = k.members.iter().position(|&m| match self.ast.member(m).kind {
            MemberKind::Method(fid) => {
                let f = self.ast.function(fid);
                f.name.is_some_and(|n| self.p.name(n.sym) == "build") && f.parameters.as_ref().is_some_and(|ps| ps.len() == 1)
            }
            _ => false,
        })?;
        let widget_name = self.p.name(k.name.sym).to_string();
        let name_state = if widget_name.starts_with('_') { format!("{widget_name}State") } else { format!("_{widget_name}State") };
        let assigned = self.fields_assigned_in_constructors(class, &k.members, &nos);
        // Os membros a mover e os elementos deles.
        let mut to_move: Vec<usize> = Vec::new();
        let mut moved: HashSet<Executable> = HashSet::new();
        for (i, &m) in k.members.iter().enumerate() {
            match &self.ast.member(m).kind {
                MemberKind::Field(l) if !l.static_ => {
                    for (j, _) in l.variables.iter().enumerate() {
                        let Some(v) = p.class(class).fields.iter().copied().find(|&v| p.variable(v).node == dartforge_elements::model::VariableRef::Field { unit: self.unit, member: m, index: j }) else { continue };
                        if !assigned.contains(&v) {
                            if !to_move.contains(&i) {
                                to_move.push(i);
                            }
                            moved.insert(Executable::Field(v));
                        }
                    }
                }
                MemberKind::Method(fid) if !self.ast.function(*fid).static_ => {
                    to_move.push(i);
                    if let Some(f) = self.p.node_function(self.unit, *fid) {
                        moved.insert(Executable::Function(f));
                    }
                }
                _ => {}
            }
        }
        let state_class = self.flutter_class("package:flutter/widgets.dart", "State")?;
        let stateful_class = self.flutter_class("package:flutter/widgets.dart", "StatefulWidget")?;
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let mut writer = crate::write_type::Writer::new(self, word.start);
        let mut m = Change::default();
        m.add(uri, p.unit(self.unit).ast.ty(extends).span, writer.reference_of(Element::Class(stateful_class), "StatefulWidget"));
        let type_parameters = self.children(class_node).iter().copied().find(|&f| self.kind(f) == "TypeParameterList").map(|f| self.node_text(f).to_string()).unwrap_or_default();
        let ref_state = writer.reference_of(Element::Class(state_class), "State");
        let raw_name = self.p.name(k.name.sym).to_string();
        // `replaceInterval`.
        let create_state = |before: bool, after: bool| -> String {
            let mut s = String::new();
            if before {
                s.push_str(eol);
            }
            s.push_str(&format!("  @override{eol}"));
            s.push_str(&format!("  {ref_state}<{raw_name}{type_parameters}> createState() => {name_state}{type_parameters}();{eol}"));
            if after {
                s.push_str(eol);
            }
            s
        };
        let mut start_swap = 0usize;
        let mut has_build = false;
        let mut last_removed_field = false;
        let mut last_kept_end = 0usize;
        for (i, &n) in nos.iter().enumerate() {
            if to_move.contains(&i) {
                if start_swap == 0 {
                    let start_ = self.comments_before(Span { start: self.tree.nos[n].inicio, end: self.tree.nos[n].inicio }).first().map_or(self.tree.nos[n].inicio, |c| c.start);
                    start_swap = tx.content_start(start_);
                }
                if i == build {
                    has_build = true;
                }
                last_removed_field = matches!(self.ast.member(k.members[i]).kind, MemberKind::Field(_));
            } else {
                let lines = tx.line_range(self.tree.nos[n].inicio, self.tree.nos[n].fim);
                last_kept_end = lines.end;
                if start_swap != 0 {
                    let text = if has_build {
                        has_build = false;
                        create_state(false, true)
                    } else if last_removed_field && !matches!(self.ast.member(k.members[i]).kind, MemberKind::Field(_)) {
                        eol.to_string()
                    } else {
                        String::new()
                    };
                    m.add(uri, Span { start: start_swap, end: lines.start }, text);
                    start_swap = 0;
                }
            }
        }
        if start_swap != 0 {
            if last_kept_end != 0 {
                start_swap = last_kept_end;
            }
            let closes = self.tree.nos[class_node].fim - 1;
            let text = if has_build { create_state(last_kept_end != 0, false) } else { String::new() };
            m.add(uri, Span { start: start_swap, end: closes }, text);
        }
        // A classe `State`.
        let mut s = format!("{eol}{eol}class {name_state}{type_parameters} extends {ref_state}<{raw_name}");
        if !k.type_params.is_empty() {
            // Só os nomes, sem separador (o `first` do Dart nunca vira
            // falso depois do primeiro).
            s.push('<');
            for t in k.type_params.iter() {
                s.push_str(self.p.name(t.name.sym));
            }
            s.push('>');
        }
        s.push_str(&format!("> {{{eol}"));
        let mut first = true;
        for &i in &to_move {
            if !first {
                s.push_str(eol);
            }
            let n = nos[i];
            if let Some(c) = self.comments_before(Span { start: self.tree.nos[n].inicio, end: self.tree.nos[n].inicio }).first() {
                let de = tx.content_start(c.start);
                s.push_str(&self.source[de..c.end]);
                s.push_str(eol);
            }
            s.push_str(&self.rewrite_to_state(n, class, &moved, &raw_name));
            first = false;
        }
        s.push('}');
        let class_end = self.tree.nos[class_node].fim;
        m.add(uri, Span { start: class_end, end: class_end }, s);
        crate::refactorings_move::builder_imports(self, &mut m, &writer.import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert to StatefulWidget".into(),
            kind: "refactor.flutter.convert.toStatefulWidget".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `rewriteWidgetMemberReferences` do StatefulWidget: `widget.` (ou o
    /// nome da classe, num estático) antes de cada referência a um membro do
    /// widget que fica.
    fn rewrite_to_state(&self, n: usize, class: ClassId, moved: &HashSet<Executable>, class_name_: &str) -> String {
        let tx = Text::new(self.source);
        let lines = tx.line_range(self.tree.nos[n].inicio, self.tree.nos[n].fim);
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let (a, b) = (self.tree.nos[n].inicio, self.tree.nos[n].fim);
        for (k, no) in self.tree.nos.iter().enumerate() {
            if no.especie != "SimpleIdentifier" || no.inicio < a || no.fim > b {
                continue;
            }
            let Some((e, owner, is_static)) = self.identifier_executable(k) else { continue };
            if owner != Some(class) || moved.contains(&e) {
                continue;
            }
            let o = no.inicio - lines.start;
            let qualifier = if is_static { class_name_ } else { "widget" };
            let interpolated = self.parent(k).is_some_and(|p| self.kind(p) == "InterpolationExpression") && no.inicio > 0 && self.source.as_bytes()[no.inicio - 1] == b'$';
            if interpolated {
                edits.push((o, 0, format!("{{{qualifier}.")));
                edits.push((o + (no.fim - no.inicio), 0, "}".to_string()));
            } else {
                edits.push((o, 0, format!("{qualifier}.")));
            }
        }
        apply_in_sequence(&self.source[lines.start..lines.end], edits)
    }
}

impl Context<'_> {
    /// `_isDefaultOverride`: o corpo vazio, ou só `super.m(…)` do mesmo nome.
    fn override_pattern(&self, method: usize) -> bool {
        let Some(&body) = self.children(method).last() else { return false };
        let Marca::Funcao(fid) = self.tree.nos[method].marca else { return false };
        let Some(name) = self.ast.function(fid).name else { return false };
        let expression = match self.kind(body) {
            "BlockFunctionBody" => {
                let Some(&block) = self.children(body).first() else { return false };
                let commands = self.children(block);
                if commands.is_empty() {
                    return true;
                }
                let [c] = commands[..] else { return false };
                if self.kind(c) != "ExpressionStatement" {
                    return false;
                }
                let Some(&e) = self.children(c).first() else { return false };
                e
            }
            "ExpressionFunctionBody" => {
                let Some(&e) = self.children(body).first() else { return false };
                e
            }
            _ => return false,
        };
        self.kind(expression) == "MethodInvocation"
            && self.children(expression).first().is_some_and(|&a| self.kind(a) == "SuperExpression")
            && self.method_name(expression).is_some_and(|m| self.node_text(m) == self.p.name(name.sym))
    }

    /// `_isState`: `State<W>` (a classe `State` exata) com o widget como único
    /// argumento.
    fn is_state_of(&self, t: dartforge_types::TypeId, widget: ClassId) -> bool {
        let Type::Interface { class, args, .. } = self.p.query.table.get(t) else { return false };
        let [a] = args[..] else { return false };
        matches!(self.p.query.table.get(a), Type::Interface { class: w, .. } if *w == widget) && self.flutter_exactly(*class, "State", URI_FRAMEWORK)
    }

    /// `FlutterConvertToStatelessWidget`.
    pub(crate) fn flutter_to_stateless(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let class_node = self.with_parents(no).find(|&k| self.kind(k) == "ClassDeclaration")?;
        let Marca::Decl(d) = self.tree.nos[class_node].marca else { return None };
        let DeclKind::Class(k) = &self.ast.decl(d).kind else { return None };
        let extends = k.extends?;
        let word = self.class_word(k.name.span)?;
        let opens = self.opens_body(class_node)?;
        if start < word.start || start > opens.end {
            return None;
        }
        let widget = self.declaration_class(self.unit, d)?;
        let p = self.p.program();
        let query = &self.p.query;
        let superclass = query.outline.classes.get(widget.0 as usize)?.supertype?;
        let Type::Interface { class: sc, .. } = query.table.get(superclass) else { return None };
        if !self.flutter_exactly(*sc, "StatefulWidget", URI_FRAMEWORK) {
            return None;
        }
        let widget_nodes = self.member_nodes(class_node, &k.members)?;
        // `_findCreateStateMethod`: o primeiro `createState`, sem parâmetros.
        let mut create_state = None;
        for (i, &m) in k.members.iter().enumerate() {
            if let MemberKind::Method(fid) = self.ast.member(m).kind
                && self.ast.function(fid).name.is_some_and(|n| self.p.name(n.sym) == "createState")
            {
                if self.ast.function(fid).parameters.as_ref().is_some_and(|ps| ps.is_empty()) {
                    create_state = Some(widget_nodes[i]);
                }
                break;
            }
        }
        let create_state = create_state?;
        // `_findStateClass`.
        let unit = &p.unit(self.unit).unit;
        let (decl_state, state) = unit.declarations.iter().copied().find_map(|x| {
            let DeclKind::Class(c) = &self.ast.decl(x).kind else { return None };
            c.extends?;
            let cid = self.declaration_class(self.unit, x)?;
            let st = query.outline.classes.get(cid.0 as usize)?.supertype?;
            self.is_state_of(st, widget).then_some((x, cid))
        })?;
        let DeclKind::Class(ks) = &self.ast.decl(decl_state).kind else { return None };
        if !self.p.name(ks.name.sym).starts_with('_') || !self.same_type_parameters(k, ks) {
            return None;
        }
        let state_node = (0..self.tree.nos.len()).find(|&n| self.tree.nos[n].marca == Marca::Decl(decl_state) && self.kind(n) == "ClassDeclaration")?;
        let state_nodes = self.member_nodes(state_node, &ks.members)?;
        // `_StatelessVerifier` e `_FieldFinder`.
        let mut assigned: HashSet<VariableId> = HashSet::new();
        let name_field = |name_: &str| p.class(state).fields.iter().copied().find(|&v| self.p.name(p.variable(v).name) == name_);
        for (i, &m) in ks.members.iter().enumerate() {
            match &self.ast.member(m).kind {
                MemberKind::Constructor(kc) => {
                    for start_ in kc.initializers.iter() {
                        if let ast::Initializer::Field { name, .. } = start_
                            && let Some(v) = name_field(self.p.name(name.sym))
                        {
                            assigned.insert(v);
                        }
                    }
                    assigned.extend(self.field_writes(state_nodes[i]));
                }
                MemberKind::Method(_) => {
                    if !self.can_be_stateless(state_nodes[i]) {
                        return None;
                    }
                }
                _ => {}
            }
        }
        // `_StateUsageVisitor`.
        if self.used_state(widget, state) {
            return None;
        }
        // Os membros a mover.
        let mut to_move: Vec<usize> = Vec::new();
        for (i, &m) in ks.members.iter().enumerate() {
            match &self.ast.member(m).kind {
                MemberKind::Field(l) => {
                    if l.static_ {
                        return None;
                    }
                    for (j, _) in l.variables.iter().enumerate() {
                        let v = p.class(state).fields.iter().copied().find(|&v| p.variable(v).node == dartforge_elements::model::VariableRef::Field { unit: self.unit, member: m, index: j });
                        if v.is_none_or(|v| !assigned.contains(&v)) {
                            to_move.push(state_nodes[i]);
                        }
                    }
                }
                MemberKind::Method(fid) => {
                    if self.ast.function(*fid).static_ {
                        return None;
                    }
                    if !self.override_pattern(state_nodes[i]) {
                        to_move.push(state_nodes[i]);
                    }
                }
                _ => {}
            }
        }
        let stateless_class = self.flutter_class("package:flutter/widgets.dart", "StatelessWidget")?;
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let mut writer = crate::write_type::Writer::new(self, word.start);
        let mut m = Change::default();
        m.add(uri, p.unit(self.unit).ast.ty(extends).span, writer.reference_of(Element::Class(stateless_class), "StatelessWidget"));
        m.add(uri, self.exclusion_range(state_node, None)?, "");
        let next = self.next_token(self.tree.nos[create_state].fim)?;
        let after = self.comments_before(next).first().map_or(next.start, |c| c.start);
        let range = Span { start: tx.content_start(self.tree.nos[create_state].inicio), end: tx.content_start(after) };
        let new_line = &self.source[next.start..next.end] != "}";
        let mut s = String::new();
        for (i, &n) in to_move.iter().enumerate() {
            if let Some(c) = self.comments_before(Span { start: self.tree.nos[n].inicio, end: self.tree.nos[n].inicio }).first() {
                let de = tx.content_start(c.start);
                s.push_str(&self.source[de..c.end]);
                s.push_str(eol);
            }
            s.push_str(&self.rewrite_to_widget(n, widget));
            if new_line || i + 1 < to_move.len() {
                s.push_str(eol);
            }
        }
        m.add(uri, range, s);
        crate::refactorings_move::builder_imports(self, &mut m, &writer.import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert to StatelessWidget".into(),
            kind: "refactor.flutter.convert.toStatelessWidget".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `_isSameTypeParameters`.
    fn same_type_parameters(&self, widget: &ast::ClassDecl, state: &ast::ClassDecl) -> bool {
        if widget.type_params.is_empty() && state.type_params.is_empty() {
            return true;
        }
        if widget.type_params.is_empty() || state.type_params.is_empty() || widget.type_params.len() < state.type_params.len() {
            return false;
        }
        let limit = |t: &ast::TypeParameter| t.bound.map(|b| self.p.program().unit(self.unit).source[self.ast.ty(b).span.start..self.ast.ty(b).span.end].to_string());
        state.type_params.iter().all(|s| widget.type_params.iter().any(|w| self.p.name(w.name.sym) == self.p.name(s.name.sym) && limit(w) == limit(s)))
    }

    /// As escritas de campo (`inSetterContext`) no nó.
    fn field_writes(&self, n: usize) -> Vec<VariableId> {
        let p = self.p.program();
        let (a, b) = (self.tree.nos[n].inicio, self.tree.nos[n].fim);
        let mut v = Vec::new();
        for no in self.tree.nos.iter() {
            if no.especie == "SimpleIdentifier" && no.inicio >= a && no.fim <= b
                && let Marca::Expr(x) = no.marca
                && self.writes.contains(&x)
            {
                match self.bodies.get_resolved(x) {
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
    fn can_be_stateless(&self, method: usize) -> bool {
        let p = self.p.program();
        let (a, b) = (self.tree.nos[method].inicio, self.tree.nos[method].fim);
        for (n, no) in self.tree.nos.iter().enumerate() {
            if no.especie != "MethodInvocation" || no.inicio < a || no.fim > b {
                continue;
            }
            let Some(name) = self.method_name(n) else { continue };
            let Some((Executable::Function(f), Some(c), _)) = self.identifier_executable(name) else { continue };
            let _ = f;
            if matches!(p.class(c).kind, dartforge_elements::model::ClassKind::Class) && self.flutter_exactly(c, "State", URI_FRAMEWORK) {
                let owner = self.with_parents(n).find(|&k| self.kind(k) == "MethodDeclaration");
                if !owner.is_some_and(|d| self.override_pattern(d)) {
                    return false;
                }
            }
        }
        true
    }

    /// `_StateUsageVisitor`: a classe `State` criada fora do `createState`
    /// do widget, ou um `createState()` chamado.
    fn used_state(&self, widget: ClassId, state: ClassId) -> bool {
        for (n, no) in self.tree.nos.iter().enumerate() {
            match no.especie {
                "InstanceCreationExpression" => {
                    let Some(t) = self.flutter_node_type(n) else { continue };
                    if self.flutter_type_class(t) != Some(state) {
                        continue;
                    }
                    let method = self.with_parents(n).find(|&k| self.kind(k) == "MethodDeclaration");
                    let class = method.and_then(|m| self.with_parents(m).find(|&k| self.kind(k) == "ClassDeclaration"));
                    let create_state_node = method.and_then(|m| match self.tree.nos[m].marca {
                        Marca::Funcao(f) => self.ast.function(f).name.map(|x| self.p.name(x.sym) == "createState"),
                        _ => None,
                    }) == Some(true);
                    let of_widget = class.and_then(|c| match self.tree.nos[c].marca {
                        Marca::Decl(d) => self.declaration_class(self.unit, d),
                        _ => None,
                    }) == Some(widget);
                    if !create_state_node || !of_widget {
                        return true;
                    }
                }
                "MethodInvocation" => {
                    if self.method_name(n).is_some_and(|m| self.node_text(m) == "createState")
                        && let Some(t) = self.flutter_node_type(n)
                        && (self.is_state_of(t, widget) || self.flutter_type_class(t) == Some(state))
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
    fn rewrite_to_widget(&self, n: usize, widget: ClassId) -> String {
        let tx = Text::new(self.source);
        let lines = tx.line_range(self.tree.nos[n].inicio, self.tree.nos[n].fim);
        let base = lines.start;
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let (a, b) = (self.tree.nos[n].inicio, self.tree.nos[n].fim);
        for (k, no) in self.tree.nos.iter().enumerate() {
            if no.especie != "SimpleIdentifier" || no.inicio < a || no.fim > b {
                continue;
            }
            let Some((_, owner, _)) = self.identifier_executable(k) else { continue };
            if owner != Some(widget) {
                continue;
            }
            let Some(parent) = self.parent(k) else { continue };
            match self.kind(parent) {
                "PrefixedIdentifier" if self.children(parent).last() == Some(&k) => {
                    let prefix = *self.children(parent).first().unwrap();
                    let grandparent = self.parent(parent).filter(|&g| self.kind(g) == "InterpolationExpression");
                    let mut closes = None;
                    if !self.node_text(k).contains('$')
                        && let Some(g) = grandparent
                        && self.source[self.tree.nos[g].inicio..].starts_with("${")
                    {
                        let opens = self.tree.nos[g].inicio + 2;
                        edits.push((opens - 1 - base, 1, String::new()));
                        let end_g = self.tree.nos[g].fim;
                        if self.source[..end_g].ends_with('}') {
                            closes = Some((end_g - 1 - base, 1, String::new()));
                        }
                    }
                    let start = self.tree.nos[prefix].inicio;
                    let dot = self.next_token(self.tree.nos[prefix].fim).map_or(self.tree.nos[prefix].fim, |t| t.end);
                    edits.push((start - base, dot - start, String::new()));
                    if let Some(f) = closes {
                        edits.push(f);
                    }
                }
                "MethodInvocation" | "PropertyAccess" if self.children(parent).len() > 1 && self.children(parent).first() != Some(&k) => {
                    let target = *self.children(parent).first().unwrap();
                    let start = self.tree.nos[target].inicio;
                    let operator = self.previous_token(no.inicio).map_or(no.inicio, |t| t.end);
                    edits.push((start - base, operator - start, String::new()));
                }
                _ => {}
            }
        }
        apply_in_sequence(&self.source[lines.start..lines.end], edits)
    }
}

/// `SourceEdit.applySequence` das edições na ordem inversa da visita.
fn apply_in_sequence(text: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    // A visita é em pré-ordem (posições crescentes).
    edits.sort_by_key(|e| e.0);
    let mut s = text.to_string();
    for (o, n, t) in edits.into_iter().rev() {
        s.replace_range(o..o + n, &t);
    }
    s
}
