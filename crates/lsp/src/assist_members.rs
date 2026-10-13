//! Assistências sobre membros de classe, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to final field` | `refactor.convert.getterToFinalField` | `ConvertIntoFinalField` |
//! | `Convert to getter` | `refactor.convert.finalFieldToGetter` | `ConvertIntoGetter` |
//! | `Convert to normal parameter` | `refactor.convert.toConstructorNormalParameter` | `ConvertToNormalParameter` |
//! | `Convert class to a mixin` | `refactor.convert.classToMixin` | `ConvertClassToMixin` |
//! | `Encapsulate field` | `refactor.encapsulateField` | `EncapsulateField` |
//! | `Convert to field formal parameter` | `refactor.convert.toConstructorFieldParameter` | `ConvertToFieldParameter` |
//! | `Create a local variable that shadows the field` | `refactor.shadowField` | `ShadowField` |

use crate::actions::CodeAction;
use crate::arvore_analyzer::Marca;
use crate::refactorings::Context;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody, StmtKind};
use dartforge_types::TypeId;

/// As espécies de `TypeAnnotation`.
const TYPES: &[&str] = &["NamedType", "GenericFunctionType", "RecordTypeAnnotation"];

impl Context<'_> {
    fn member_action(&self, uri: &str, title: &str, kind: &str, span: Span, text: String) -> CodeAction {
        CodeAction {
            title: title.into(),
            kind: kind.into(),
            edits: vec![Edit { uri: uri.to_string(), span, text }],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        }
    }

    /// `ConvertIntoFinalField` (convert_into_final_field.dart).
    pub(crate) fn convert_to_final_field(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        // O getter envolvente: passa por identificadores e tipos.
        let mut getter = None;
        for n in self.with_parents(no) {
            let e = self.kind(n);
            if e == "MethodDeclaration" {
                getter = Some(n);
                break;
            }
            if e == "SimpleIdentifier" || TYPES.contains(&e) || e == "TypeArgumentList" {
                continue;
            }
            break;
        }
        let getter = getter?;
        let Marca::Funcao(fid) = self.tree.nos[getter].marca else { return None };
        let f = self.ast.function(fid);
        if f.kind != ast::FunctionKind::Getter {
            return None;
        }
        let name = f.name?;
        // Sem setter correspondente (`variable.setter2`).
        let element = self.p.node_function(self.unit, fid)?;
        let prog = self.p.program();
        let fe = prog.function(element);
        let key = self.p.query.names.lookup(&format!("{}=", self.p.name(fe.name)));
        let has_setter = key.is_some_and(|k| match (fe.class, fe.extension) {
            (Some(c), _) => {
                let k_ = prog.class(c);
                if fe.static_ { k_.static_members.contains_key(&k) } else { k_.instance_members.contains_key(&k) }
            }
            (None, Some(x)) => {
                let x_ = prog.extension(x);
                if fe.static_ { x_.static_members.contains_key(&k) } else { x_.instance_members.contains_key(&k) }
            }
            _ => false,
        });
        if has_setter {
            return None;
        }
        // A expressão devolvida.
        let expression = match &f.body {
            FunctionBody::Expression(e) => Some(*e),
            FunctionBody::Block(b) => match &self.ast.stmt(*b).kind {
                StmtKind::Block(cmds) if cmds.len() == 1 => match &self.ast.stmt(cmds[0]).kind {
                    StmtKind::Return(Some(e)) => Some(*e),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }?;
        let mut code = String::from("final");
        if let Some(t) = f.return_type {
            let s = self.ast.ty(t).span;
            code.push(' ');
            code.push_str(&self.source[s.start..s.end]);
        }
        code.push(' ');
        code.push_str(&self.source[name.span.start..name.span.end]);
        let es = self.ast.expr(expression).span;
        if !matches!(self.ast.expr(expression).kind, ast::ExprKind::Null) {
            code.push_str(" = ");
            code.push_str(&self.source[es.start..es.end]);
        }
        code.push(';');
        // `range.startEnd(returnType ?? propertyKeywordGet, getter)`.
        let start_swap = match f.return_type {
            Some(t) => self.ast.ty(t).span.start,
            None => self
                .tokens
                .iter()
                .filter(|t| t.span.end <= name.span.start && &self.source[t.span.start..t.span.end] == "get")
                .map(|t| t.span.start)
                .next_back()?,
        };
        let range = Span { start: start_swap, end: self.tree.nos[getter].fim };
        Some(self.member_action(uri, "Convert to final field", "refactor.convert.getterToFinalField", range, code))
    }

    /// `ConvertIntoGetter` (convert_into_getter.dart).
    pub(crate) fn convert_to_getter(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let mut declaration = None;
        for n in self.with_parents(no) {
            let e = self.kind(n);
            if e == "FieldDeclaration" {
                declaration = Some(n);
                break;
            }
            if matches!(e, "SimpleIdentifier" | "VariableDeclaration" | "VariableDeclarationList" | "TypeArgumentList") || TYPES.contains(&e) {
                continue;
            }
            break;
        }
        let declaration = declaration?;
        let list = self.children(declaration).iter().copied().find(|&f| self.kind(f) == "VariableDeclarationList")?;
        let variables = self.kind_children(list, "VariableDeclaration");
        if variables.len() != 1 {
            return None;
        }
        let field = variables[0];
        // O `keyword` da lista (`final`, `const` ou `var`).
        let ty = self.children(list).iter().copied().find(|&f| TYPES.contains(&self.kind(f)));
        let limit = ty.map_or(self.tree.nos[field].inicio, |t| self.tree.nos[t].inicio);
        let word = self
            .tokens
            .iter()
            .find(|t| t.span.start >= self.tree.nos[list].inicio && t.span.end <= limit && matches!(&self.source[t.span.start..t.span.end], "final" | "const" | "var"))?
            .span;
        let initializer = *self.children(field).first()?;
        let name = self.next_token(self.tree.nos[field].inicio)?;
        let mut code = String::new();
        if let Some(t) = ty {
            code.push_str(self.node_text(t));
            code.push(' ');
        }
        code.push_str("get ");
        code.push_str(&self.source[name.start..name.end]);
        code.push_str(" => ");
        code.push_str(self.node_text(initializer));
        code.push(';');
        let range = Span { start: word.start, end: self.tree.nos[declaration].fim };
        Some(self.member_action(uri, "Convert to getter", "refactor.convert.finalFieldToGetter", range, code))
    }

    /// `ConvertToNormalParameter` (convert_to_normal_parameter.dart): um
    /// `this.x` direto na lista de parâmetros de um construtor vira
    /// parâmetro comum com o inicializador `x = x`.
    pub(crate) fn convert_to_normal_parameter(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let parameter = self.tree.localizar(start, end)?;
        if self.kind(parameter) != "FieldFormalParameter" {
            return None;
        }
        let list = self.parent(parameter)?;
        if self.kind(list) != "FormalParameterList" {
            return None;
        }
        let constructor = self.parent(list)?;
        if self.kind(constructor) != "ConstructorDeclaration" {
            return None;
        }
        // O nome: o último token do parâmetro (`this.x`).
        let name_span = self.previous_token(self.tree.nos[parameter].fim)?;
        let name = self.source[name_span.start..name_span.end].to_string();
        // O tipo do elemento: o do parâmetro do construtor, pela posição.
        let index = self.children(list).iter().position(|&k| k == parameter)?;
        let node_end = self.tree.nos[constructor].fim;
        let mi = self.ast.members.iter().position(|m| matches!(m.kind, ast::MemberKind::Constructor(_)) && m.span.end == node_end)?;
        let element = self.p.node_constructor(self.unit, ast::MemberId(mi as u32))?;
        let ty = self.p.query.outline.functions.get(element.0 as usize)?.parameters.get(index)?.ty;
        let mut import = std::collections::BTreeSet::new();
        let replacement = if matches!(self.p.query.table.get(ty), dartforge_types::Type::Dynamic) {
            name.clone()
        } else {
            let mut writer = crate::write_type::Writer::new(self, self.tree.nos[parameter].inicio);
            let written = writer.write(ty, false).unwrap_or_default();
            import = writer.import;
            format!("{written} {name}")
        };
        let initializers: Vec<usize> = self
            .children(constructor)
            .iter()
            .copied()
            .filter(|&k| matches!(self.kind(k), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        let (em, text) = match initializers.last() {
            None => (self.tree.nos[list].fim, format!(" : {name} = {name}")),
            Some(&u) => (self.tree.nos[u].fim, format!(", {name} = {name}")),
        };
        let mut m = crate::refactorings_exec::Change::default();
        m.add(uri, self.tree.span(parameter), replacement);
        m.add(uri, Span { start: em, end: em }, text);
        crate::refactorings_move::builder_imports(self, &mut m, &import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert to normal parameter".into(),
            kind: "refactor.convert.toConstructorNormalParameter".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `_SuperclassReferenceFinder`: a classe (não mixin) que declara cada
    /// membro alcançado por `super` dentro de `span`, com repetições.
    fn classes_referenced_by_super(&self, span: Span) -> Vec<dartforge_elements::model::ClassId> {
        use dartforge_types::{MemberRef, Resolved};
        let prog = self.p.program();
        let mut output = Vec::new();
        let owner = |r: Option<&Resolved>| -> Option<dartforge_elements::model::ClassId> {
            let f = match r? {
                Resolved::Member { member: MemberRef::Function(f), .. } => *f,
                Resolved::Member { member: MemberRef::Variable(v), .. } => prog.variable(*v).getter?,
                _ => return None,
            };
            let c = prog.function(f).class?;
            (prog.class(c).kind != dartforge_elements::model::ClassKind::Mixin).then_some(c)
        };
        let is_super = |x: ast::ExprId| matches!(self.ast.expr(x).kind, ast::ExprKind::Super);
        for (i, e) in self.ast.exprs.iter().enumerate() {
            if e.span.start < span.start || e.span.end > span.end {
                continue;
            }
            let id = ast::ExprId(i as u32);
            let target_ = match &e.kind {
                // `super.x`, `super.m(…)` (o nome está no `Property`).
                ast::ExprKind::Property { target, .. } if is_super(*target) => Some(id),
                ast::ExprKind::Binary { left, .. } if is_super(*left) => Some(id),
                ast::ExprKind::Index { target, .. } if is_super(*target) => Some(id),
                _ => None,
            };
            if let Some(c) = target_.and_then(|x| owner(self.bodies.get_resolved(x))) {
                output.push(c);
            }
        }
        output
    }

    /// `ConvertClassToMixin` (convert_class_to_mixin.dart).
    pub(crate) fn convert_class_to_mixin(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let declaration = self.with_parents(no).find(|&k| self.kind(k) == "ClassDeclaration")?;
        let Marca::Decl(d) = self.tree.nos[declaration].marca else { return None };
        let ast::DeclKind::Class(k) = &self.ast.decl(d).kind else { return None };
        // `class` (a palavra) e o nome.
        let class_word = self
            .tokens
            .iter()
            .filter(|t| t.span.end <= k.name.span.start && &self.source[t.span.start..t.span.end] == "class")
            .map(|t| t.span)
            .next_back()?;
        if start > k.name.span.end || end < class_word.start {
            return None;
        }
        if k.members.iter().any(|&m| matches!(self.ast.member(m).kind, ast::MemberKind::Constructor(_))) {
            return None;
        }
        if k.modifiers.final_ || k.modifiers.interface || k.modifiers.sealed {
            return None;
        }
        let class_ = self.declaration_class(self.unit, d)?;
        let data = self.p.query.outline.classes.get(class_.0 as usize)?;
        let referred = self.classes_referenced_by_super(self.tree.span(declaration));
        let table = &self.p.query.table;
        let type_class = |t: TypeId| match table.get(t) {
            dartforge_types::Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let mut constraints: Vec<TypeId> = Vec::new();
        let mut interfaces: Vec<TypeId> = Vec::new();
        for &m in data.mixins.iter() {
            if type_class(m).is_some_and(|c| referred.contains(&c)) {
                constraints.push(m);
            } else {
                interfaces.push(m);
            }
        }
        if k.extends.is_some()
            && let Some(s) = data.supertype
        {
            if referred.len() > constraints.len() {
                constraints.insert(0, s);
            } else {
                interfaces.insert(0, s);
            }
        }
        interfaces.extend(data.interfaces.iter().copied());
        let mut writer = crate::write_type::Writer::new(self, class_word.start);
        let mut text = format!("mixin {}", &self.source[k.name.span.start..k.name.span.end]);
        // `writeTypeParameters2`.
        if !k.type_params.is_empty() {
            let mut parts = Vec::new();
            for (i, tp) in k.type_params.iter().enumerate() {
                let mut s = self.source[tp.name.span.start..tp.name.span.end].to_string();
                if tp.bound.is_some()
                    && let Some(&param) = data.type_params.get(i)
                {
                    let limit = table.param(param).bound;
                    if let Some(b) = writer.write(limit, false) {
                        s.push_str(" extends ");
                        s.push_str(&b);
                    }
                }
                parts.push(s);
            }
            text.push('<');
            text.push_str(&parts.join(", "));
            text.push('>');
        }
        let write_list = |writer: &mut crate::write_type::Writer<'_, '_>, types: &[TypeId], prefix: &str, text: &mut String| {
            if types.is_empty() {
                return;
            }
            let parts: Vec<String> = types.iter().map(|&t| writer.write(t, false).unwrap_or_default()).collect();
            text.push_str(prefix);
            text.push_str(&parts.join(", "));
        };
        write_list(&mut writer, &constraints, " on ", &mut text);
        write_list(&mut writer, &interfaces, " implements ", &mut text);
        text.push(' ');
        let start_swap = if k.modifiers.abstract_ {
            self.tokens
                .iter()
                .filter(|t| t.span.end <= class_word.start && &self.source[t.span.start..t.span.end] == "abstract")
                .map(|t| t.span.start)
                .next_back()
                .unwrap_or(class_word.start)
        } else {
            class_word.start
        };
        let opens = self.tokens.iter().find(|t| t.span.start >= k.name.span.end && &self.source[t.span.start..t.span.end] == "{")?.span.start;
        let mut m = crate::refactorings_exec::Change::default();
        m.add(uri, Span { start: start_swap, end: opens }, text);
        crate::refactorings_move::builder_imports(self, &mut m, &writer.import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert class to a mixin".into(),
            kind: "refactor.convert.classToMixin".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `inheritanceManager.getOverridden4(class, Name(name))`: algum
    /// supertipo (superclasse, mixins, interfaces, restrições `on`, de forma
    /// transitiva) declara um membro de instância com esse nome — o getter ou
    /// o método (`setter` falso), ou o setter (`name=`).
    fn overridden_member(&self, class: dartforge_elements::model::ClassId, name: &str, setter: bool) -> bool {
        let prog = self.p.program();
        let key = if setter { format!("{name}_=") } else { name.to_string() };
        let mut seen = std::collections::HashSet::new();
        let ce = prog.class(class);
        let mut stack: Vec<dartforge_elements::model::ClassId> =
            ce.supertype_class.iter().chain(&ce.mixin_classes).chain(&ce.interface_classes).chain(&ce.on_classes).copied().collect();
        while let Some(c) = stack.pop() {
            if !seen.insert(c) {
                continue;
            }
            let e = prog.class(c);
            if e.instance_members.keys().any(|k| self.p.name(*k) == key) {
                return true;
            }
            stack.extend(e.supertype_class.iter().chain(&e.mixin_classes).chain(&e.interface_classes).chain(&e.on_classes).copied());
        }
        false
    }

    /// `EncapsulateField` (encapsulate_field.dart): no nome de um campo de
    /// instância público, não final, único na declaração, de classe ou mixin,
    /// o campo vira privado e ganha getter e setter com o nome dele.
    pub(crate) fn encapsulate_field(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let declaration = self.with_parents(no).find(|&k| self.kind(k) == "FieldDeclaration")?;
        let list = self.children(declaration).iter().copied().find(|&k| self.kind(k) == "VariableDeclarationList")?;
        // As palavras antes da lista (`static`, `covariant`…) e as da lista.
        let words = |de: usize, until: usize| -> Vec<&str> {
            self.tokens.iter().filter(|t| t.span.start >= de && t.span.end <= until).map(|t| &self.source[t.span.start..t.span.end]).collect()
        };
        let annotations: Vec<usize> = self.children(declaration).iter().copied().filter(|&k| self.kind(k) == "Annotation").collect();
        let after_metadata = annotations.last().map_or(self.tree.nos[declaration].inicio, |&a| self.tree.nos[a].fim);
        let before = words(after_metadata, self.tree.nos[list].inicio);
        if before.contains(&"static") {
            return None;
        }
        let ty = self.children(list).iter().copied().find(|&k| TYPES.contains(&self.kind(k)));
        let variables: Vec<usize> = self.children(list).iter().copied().filter(|&k| self.kind(k) == "VariableDeclaration").collect();
        let up_to_name = variables.first().map_or(self.tree.nos[list].fim, |&v| self.tree.nos[v].inicio);
        let of_list = words(self.tree.nos[list].inicio, up_to_name);
        let word = of_list.iter().find(|w| matches!(**w, "var" | "final" | "const")).copied();
        if word.is_none() && ty.is_none() {
            return None;
        }
        if word == Some("final") || variables.len() != 1 {
            return None;
        }
        let name_tk = self.next_token(self.tree.nos[variables[0]].inicio)?;
        let name = &self.source[name_tk.start..name_tk.end];
        if name.starts_with('_') {
            return None;
        }
        // `nameToken != token`: o token da seleção é o nome.
        if !(name_tk.start <= start && start <= name_tk.end) {
            return None;
        }
        let class_body = self.parent(declaration)?;
        if !matches!(self.kind(class_body), "ClassDeclaration" | "MixinDeclaration") {
            return None;
        }
        let Marca::Decl(d) = self.tree.nos[class_body].marca else { return None };
        let class = self.declaration_class(self.unit, d)?;
        let tx = crate::refactorings_exec::Text::new(self.source);
        let eol = tx.eol();
        let mut m = crate::refactorings_exec::Change::default();
        // Sem as anotações no campo.
        if let (Some(&first), Some(&last)) = (annotations.first(), annotations.last()) {
            m.add(uri, tx.line_range(self.tree.nos[first].inicio, self.tree.nos[last].fim), String::new());
        }
        m.add(uri, name_tk, format!("_{name}"));
        let type_code = ty.map_or(String::new(), |t| self.node_text(t).to_string());
        // Os construtores: os `this.x` e os inicializadores do campo.
        for &member in self.children(class_body) {
            if self.kind(member) != "ConstructorDeclaration" {
                continue;
            }
            let Some(&parameters) = self.children(member).iter().find(|&&k| self.kind(k) == "FormalParameterList") else { continue };
            let initializers: Vec<usize> = self
                .children(member)
                .iter()
                .copied()
                .filter(|&k| matches!(self.kind(k), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
                .collect();
            for &parameter in self.children(parameters) {
                let (internal, named) = if self.kind(parameter) == "DefaultFormalParameter" {
                    let Some(&i) = self.children(parameter).first() else { continue };
                    // `isNamed`: entre `{ }`.
                    let before_parameter = &self.source[self.tree.nos[parameters].inicio..self.tree.nos[parameter].inicio];
                    (i, before_parameter.contains('{'))
                } else {
                    (parameter, false)
                };
                if self.kind(internal) != "FieldFormalParameter" {
                    continue;
                }
                let Some(identifier) = self.previous_token(self.tree.nos[internal].fim) else { continue };
                if &self.source[identifier.start..identifier.end] != name {
                    continue;
                }
                if named {
                    // `this.` sai (com o tipo do campo no lugar) e o campo é
                    // inicializado na lista.
                    let this_ = self.tokens.iter().find(|t| t.span.start >= self.tree.nos[internal].inicio && &self.source[t.span.start..t.span.end] == "this")?.span;
                    let dot = self.next_token(this_.end)?;
                    let text = if type_code.is_empty() { String::new() } else { format!("{type_code} ") };
                    m.add(uri, Span { start: this_.start, end: dot.end }, text);
                    if initializers.is_empty() {
                        let parameter_end = self.tree.nos[parameters].fim;
                        m.add(uri, Span { start: parameter_end, end: parameter_end }, format!(" : _{name} = {name}"));
                    } else {
                        let separator = self.previous_token(self.tree.nos[initializers[0]].inicio)?;
                        m.add(uri, Span { start: separator.end, end: separator.end }, format!(" _{name} = {name},"));
                    }
                    break;
                }
                m.add(uri, identifier, format!("_{name}"));
            }
            for &i in &initializers {
                if self.kind(i) != "ConstructorFieldInitializer" {
                    continue;
                }
                let Some(&field) = self.children(i).first() else { continue };
                if self.node_text(field) == name {
                    m.add(uri, self.tree.span(field), format!("_{name}"));
                }
            }
        }
        // O getter e o setter, depois da declaração.
        let documentation = self.children(declaration).iter().copied().find(|&k| self.kind(k) == "Comment").map(|c| self.node_text(c).to_string());
        let code = if type_code.is_empty() { String::new() } else { format!("{type_code} ") };
        let header = |preserve_override: bool| -> String {
            let mut s = format!("{eol}{eol}");
            if let Some(doc) = &documentation {
                s.push_str(&format!("  {doc}{eol}"));
            }
            for &a in &annotations {
                let text = self.node_text(a);
                if text != "@override" || preserve_override {
                    s.push_str(&format!("  {text}{eol}"));
                }
            }
            s
        };
        let mut s = header(self.overridden_member(class, name, false));
        s.push_str(&format!("  {code}get {name} => _{name};"));
        s.push_str(&header(self.overridden_member(class, name, true)));
        s.push_str(&format!("  set {name}({code}value) {{{eol}    _{name} = value;{eol}  }}"));
        let declaration_end = self.tree.nos[declaration].fim;
        m.add(uri, Span { start: declaration_end, end: declaration_end }, s);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Encapsulate field".into(),
            kind: "refactor.encapsulateField".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `ConvertToFieldParameter._findParameter`: o parâmetro simples (direto
    /// na lista de um construtor) da seleção, ou o citado pela expressão de
    /// um inicializador de campo.
    fn parameter_to_field(&self, no: usize) -> Option<(usize, Span, usize)> {
        let parent = self.parent(no)?;
        if self.kind(no) == "SimpleFormalParameter" {
            let name = self.previous_token(self.tree.nos[no].fim)?;
            if self.kind(parent) != "FormalParameterList" {
                return None;
            }
            let constructor = self.parent(parent)?;
            return (self.kind(constructor) == "ConstructorDeclaration").then_some((no, name, constructor));
        }
        if self.kind(no) == "SimpleIdentifier" && self.kind(parent) == "ConstructorFieldInitializer" {
            let constructor = self.parent(parent)?;
            if self.kind(constructor) != "ConstructorDeclaration" || self.children(parent).get(1) != Some(&no) {
                return None;
            }
            let list = self.children(constructor).iter().copied().find(|&k| self.kind(k) == "FormalParameterList")?;
            let text = self.node_text(no);
            for &f in self.children(list) {
                if self.kind(f) == "SimpleFormalParameter"
                    && let Some(name) = self.previous_token(self.tree.nos[f].fim)
                    && &self.source[name.start..name.end] == text
                {
                    return Some((f, name, constructor));
                }
            }
        }
        None
    }

    /// `ConvertToFieldParameter` (convert_to_field_parameter.dart): o
    /// parâmetro citado uma só vez nos inicializadores, num `field = p`, vira
    /// `this.field` e o inicializador sai.
    pub(crate) fn convert_to_field_parameter(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let (parameter, name_, constructor) = self.parameter_to_field(no)?;
        let parameter_name = &self.source[name_.start..name_.end];
        let initializers: Vec<usize> = self
            .children(constructor)
            .iter()
            .copied()
            .filter(|&k| matches!(self.kind(k), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        // `_ReferenceCounter`: os identificadores dos inicializadores cujo
        // elemento é o parâmetro.
        let mut count = 0;
        let mut stack = initializers.clone();
        while let Some(k) = stack.pop() {
            if self.kind(k) == "SimpleIdentifier"
                && let Marca::Expr(x) = self.tree.nos[k].marca
                && match self.bodies.get_resolved(x) {
                    Some(dartforge_types::Resolved::Parameter { name, .. }) => self.p.name(*name) == parameter_name,
                    // Nos inicializadores o parâmetro é um local declarado no nome dele.
                    Some(dartforge_types::Resolved::Local(_)) => self.bodies.declaracao_local(x) == Some(name_.start),
                    _ => false,
                }
            {
                count += 1;
            }
            stack.extend(self.children(k).iter().copied());
        }
        if count != 1 {
            return None;
        }
        let initializer = initializers.iter().copied().filter(|&i| {
            self.kind(i) == "ConstructorFieldInitializer"
                && self.children(i).get(1).is_some_and(|&e| self.kind(e) == "SimpleIdentifier" && self.node_text(e) == parameter_name)
        }).last()?;
        let field = self.node_text(*self.children(initializer).first()?).to_string();
        let mut edits = vec![(self.tree.span(parameter), format!("this.{field}"))];
        let index = initializers.iter().position(|&i| i == initializer)?;
        let list = self.children(constructor).iter().copied().find(|&k| self.kind(k) == "FormalParameterList")?;
        if initializers.len() == 1 {
            edits.push((Span { start: self.tree.nos[list].fim, end: self.tree.nos[initializer].fim }, String::new()));
        } else if index == 0 {
            edits.push((Span { start: self.tree.nos[initializer].inicio, end: self.tree.nos[initializers[1]].inicio }, String::new()));
        } else {
            edits.push((Span { start: self.tree.nos[initializers[index - 1]].fim, end: self.tree.nos[initializer].fim }, String::new()));
        }
        Some(CodeAction {
            title: "Convert to field formal parameter".into(),
            kind: "refactor.convert.toConstructorFieldParameter".into(),
            edits: edits.into_iter().rev().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `ShadowField` (shadow_field.dart): o campo testado com `is` ou com
    /// `==`/`!=` na condição de um `if` (através de `&&`) dentro de um bloco
    /// que não escreve nele ganha um local de mesmo nome antes do `if`.
    pub(crate) fn shadow_field(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        use dartforge_types::{MemberRef, Resolved};
        let no = self.tree.localizar(start, end)?;
        if self.kind(no) != "SimpleIdentifier" {
            return None;
        }
        let Marca::Expr(x) = self.tree.nos[no].marca else { return None };
        let prog = self.p.program();
        // O getter de uma classe (o acessor do campo ou o declarado).
        let (variable, getter, class) = match self.bodies.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Variable(v), .. } => (Some(*v), None, prog.variable(*v).class?),
            Resolved::Member { member: MemberRef::Function(f), .. } => {
                let fe = prog.function(*f);
                match fe.kind {
                    dartforge_elements::model::FunctionKind::Getter => (None, Some(*f), fe.class?),
                    // O getter sintético de um campo.
                    dartforge_elements::model::FunctionKind::ImplicitAccessor => {
                        let v = fe.variable?;
                        if prog.variable(v).getter != Some(*f) {
                            return None;
                        }
                        (Some(v), None, fe.class?)
                    }
                    _ => return None,
                }
            }
            _ => return None,
        };
        if self.writes.contains(&x) {
            return None;
        }
        // `_getStatement`.
        let parent = self.parent(no)?;
        let condition = match self.kind(parent) {
            "IsExpression" if self.children(parent).first() == Some(&no) => parent,
            "BinaryExpression" if matches!(self.binary_operator(parent), Some("==" | "!=")) => parent,
            _ => return None,
        };
        let mut above = self.parent(condition)?;
        while self.kind(above) == "BinaryExpression" && self.binary_operator(above) == Some("&&") {
            above = self.parent(above)?;
        }
        if self.kind(above) != "IfStatement" {
            return None;
        }
        let command = above;
        let block = self.parent(command)?;
        if self.kind(block) != "Block" {
            return None;
        }
        // `correspondingSetter2`.
        let name = self.node_text(no).to_string();
        let setter: Option<dartforge_elements::model::FunctionElementId> = match (variable, getter) {
            (Some(v), _) => prog.variable(v).setter,
            (None, Some(_)) => prog.class(class).instance_members.iter().find(|(k, _)| self.p.name(**k) == format!("{name}_=")).map(|(_, f)| *f),
            _ => None,
        };
        let setter = setter?;
        // `_ReferenceFinder`: uma escrita no setter dentro do bloco.
        let block_span = self.tree.span(block);
        let writes = self.writes.iter().any(|&e| {
            let s = self.ast.expr(e).span;
            s.start >= block_span.start
                && s.end <= block_span.end
                && match self.bodies.get_resolved(e) {
                    Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => prog.variable(*v).setter == Some(setter),
                    Some(Resolved::Member { member: MemberRef::Function(f), .. }) => *f == setter,
                    _ => false,
                }
        });
        if writes {
            return None;
        }
        let tx = crate::refactorings_exec::Text::new(self.source);
        let offset = self.tree.nos[command].inicio;
        let prefix = tx.line_prefix(offset);
        let eol = tx.eol();
        Some(CodeAction {
            title: "Create a local variable that shadows the field".into(),
            kind: "refactor.shadowField".into(),
            edits: vec![Edit { uri: uri.to_string(), span: Span { start: offset, end: offset }, text: format!("var {name} = this.{name};{eol}{prefix}") }],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }
}
