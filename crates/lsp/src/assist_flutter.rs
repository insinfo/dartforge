//! Assistências do Flutter do `analysis_server` 3.6.2 sobre a árvore no
//! formato do analyzer, com as utilidades de
//! `utilities/extensions/flutter.dart` (o widget é reconhecido pela classe
//! `Widget` de `package:flutter/src/widgets/framework.dart` nos supertipos).
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to children:` | `refactor.flutter.convert.childToChildren` | `FlutterConvertToChildren` |
//! | `Move widget down` / `Move widget up` | `refactor.flutter.move.down` / `.up` | `FlutterMoveDown`, `FlutterMoveUp` |
//! | `Remove this widget` | `refactor.flutter.removeWidget` | `FlutterRemoveWidget` |
//! | `Swap with child` / `Swap with parent` | `refactor.flutter.swap.withChild` / `.withParent` | `FlutterSwapWithChild`, `FlutterSwapWithParent` |
//! | `Wrap with Builder` | `refactor.flutter.wrap.builder` | `FlutterWrapBuilder` |
//! | `Wrap with StreamBuilder` | `refactor.flutter.wrap.streamBuilder` | `FlutterWrapStreamBuilder` |
//! | `Wrap with widget...` | `refactor.flutter.wrap.generic` | `FlutterWrapGeneric`, `FlutterWrap` |
//! | `Wrap with Center`/`Container`/`Padding`/`SizedBox`/`Column`/`Row` | `refactor.flutter.wrap.*` | `FlutterWrap` |
//! | `Move child property to end of arguments` | `refactor.sort.child.properties.last` | `SortChildPropertyLast` |

use crate::actions::CodeAction;
use crate::refactorings::Context;
use crate::refactorings_exec::{Change, Text, ONE_INDENT};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element};
use dartforge_types::{Resolved, Type, TypeId};

pub(crate) const URI_FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";
const URI_BASIC: &str = "package:flutter/src/widgets/basic.dart";
const URI_CONTAINER: &str = "package:flutter/src/widgets/container.dart";
const URI_ASYNC: &str = "package:flutter/src/widgets/async.dart";
const URI_WIDGETS: &str = "package:flutter/widgets.dart";

impl Context<'_> {
    /// `_isExactly`: a `ClassElement` de nome `name` declarada no arquivo
    /// `uri`.
    pub(crate) fn flutter_exactly(&self, c: ClassId, name: &str, uri: &str) -> bool {
        let p = self.p.program();
        let k = p.class(c);
        matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) && self.p.name(k.name) == name && k.decl.is_some_and(|d| p.unit(d.unit).uri == uri)
    }

    /// `isWidget`: a classe `Widget` ou uma subclasse dela.
    pub(crate) fn class_and_widget(&self, c: ClassId) -> bool {
        let p = self.p.program();
        if !matches!(p.class(c).kind, ClassKind::Class | ClassKind::MixinApplication) {
            return false;
        }
        self.flutter_exactly(c, "Widget", URI_FRAMEWORK) || self.p.supertypes(c).into_iter().any(|s| self.flutter_exactly(s, "Widget", URI_FRAMEWORK))
    }

    /// A classe de um tipo de interface.
    pub(crate) fn flutter_type_class(&self, t: TypeId) -> Option<ClassId> {
        match self.p.query.table.get(t) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        }
    }

    /// `isWidgetType`.
    fn type_widget(&self, t: TypeId) -> bool {
        self.flutter_type_class(t).is_some_and(|c| self.class_and_widget(c))
    }

    /// O `staticType` do nó de expressão.
    pub(crate) fn flutter_node_type(&self, n: usize) -> Option<TypeId> {
        self.node_expr(n).and_then(|x| self.bodies.get_type(x))
    }

    /// `isWidgetExpression`.
    pub(crate) fn is_expression_widget(&self, n: usize) -> bool {
        if let Some(parent) = self.parent(n) {
            if self.kind(parent) == "NamedType" || self.parent(parent).is_some_and(|a| self.kind(a) == "NamedType") {
                return false;
            }
            if self.kind(parent) == "ConstructorName" {
                return false;
            }
        }
        if self.kind(n) == "NamedExpression" || !self.is_expression(n) {
            return false;
        }
        self.flutter_node_type(n).is_some_and(|t| self.type_widget(t))
    }

    /// `findWidgetExpression`.
    pub(crate) fn find_expression_widget(&self, n: usize) -> Option<usize> {
        let mut current = Some(n);
        while let Some(k) = current {
            if !self.is_expression_widget(k) {
                let e = self.kind(k);
                if e == "ArgumentList" || self.is_command(k) || e.ends_with("FunctionBody") {
                    return None;
                }
                current = self.parent(k);
                continue;
            }
            if self.kind(k) == "AssignmentExpression" {
                return None;
            }
            let Some(parent) = self.parent(k) else { return None };
            let children = self.children(parent);
            let last = children.last() == Some(&k);
            match self.kind(parent) {
                "AssignmentExpression" => return if last { Some(k) } else { None },
                "ArgumentList" | "ListLiteral" | "VariableDeclaration" => return Some(k),
                "ConditionalExpression" if children.first() != Some(&k) => return Some(k),
                "ExpressionFunctionBody" | "NamedExpression" | "SwitchExpressionCase" if last => return Some(k),
                "ForElement" if last => return Some(k),
                "IfElement" => {
                    // `thenElement`/`elseElement`: não a condição (nem o
                    // padrão do `case`).
                    let condition = children.first() == Some(&k) || self.kind(k) == "CaseClause";
                    if !condition {
                        return Some(k);
                    }
                }
                _ if self.is_command(parent) => return Some(k),
                _ => {}
            }
            current = Some(parent);
        }
        None
    }

    /// `findInstanceCreationExpression`.
    pub(crate) fn find_creation(&self, n: usize) -> Option<usize> {
        let mut no = n;
        for kind in ["ImportPrefixReference", "SimpleIdentifier", "PrefixedIdentifier", "NamedType", "ConstructorName"] {
            if self.kind(no) == kind {
                no = self.parent(no)?;
            }
        }
        (self.kind(no) == "InstanceCreationExpression").then_some(no)
    }

    /// `isWidgetCreation`.
    pub(crate) fn widget_creation(&self, n: usize) -> bool {
        if self.kind(n) != "InstanceCreationExpression" {
            return false;
        }
        match self.node_expr(n).and_then(|x| self.bodies.get_resolved(x)) {
            Some(Resolved::Constructor(f)) => self.p.program().function(*f).class.is_some_and(|c| self.class_and_widget(c)),
            _ => false,
        }
    }

    /// A `ArgumentList` de uma invocação.
    fn flutter_argument_list(&self, invocation: usize) -> Option<usize> {
        self.children(invocation).iter().copied().find(|&k| self.kind(k) == "ArgumentList")
    }

    /// O nome do rótulo de uma `NamedExpression`.
    pub(crate) fn named_label(&self, named: usize) -> Option<&str> {
        let label = *self.children(named).first()?;
        if self.kind(label) != "Label" {
            return None;
        }
        Some(self.node_text(*self.children(label).first()?))
    }

    /// `childArgument`, `childrenArgument`, `builderArgument`…: o argumento
    /// nomeado `name` da criação.
    pub(crate) fn flutter_named_argument(&self, creation: usize, name: &str) -> Option<usize> {
        let list = self.flutter_argument_list(creation)?;
        self.children(list).iter().copied().find(|&a| self.kind(a) == "NamedExpression" && self.named_label(a) == Some(name))
    }

    /// A expressão de uma `NamedExpression`.
    pub(crate) fn named_expression(&self, named: usize) -> Option<usize> {
        self.children(named).last().copied().filter(|&e| self.kind(e) != "Label")
    }

    /// `findArgumentNamed`: o identificador é o rótulo `name:` de um
    /// argumento de uma criação de widget.
    fn named_argument_in_label(&self, n: usize, name: &str) -> Option<usize> {
        if self.kind(n) != "SimpleIdentifier" || self.node_text(n) != name {
            return None;
        }
        let label = self.parent(n).filter(|&p| self.kind(p) == "Label")?;
        let named = self.parent(label).filter(|&p| self.kind(p) == "NamedExpression")?;
        let invocation = self.parent(named).and_then(|l| self.parent(l))?;
        (self.kind(invocation) == "InstanceCreationExpression" && self.widget_creation(invocation)).then_some(named)
    }

    /// `sessionHelper.getClass(uri, name)`: a classe exportada pela
    /// biblioteca `uri`.
    pub(crate) fn flutter_class(&self, uri: &str, name: &str) -> Option<ClassId> {
        let p = self.p.program();
        let lib = p.libraries.iter().position(|l| l.uri == uri)?;
        let s = self.p.query.names.lookup(name)?;
        match p.library(dartforge_elements::model::LibraryId(lib as u32)).exported.get(&s)?.getter? {
            Element::Class(c) => Some(c),
            _ => None,
        }
    }

    /// [`Self::action_with_change`] com a seleção e os grupos ligados de `e`,
    /// que é o texto da edição em `snippet` (as outras edições, de imports,
    /// tiram a ação do caminho do snippet).
    fn linked_action(&self, title: &str, kind: &str, m: Change, snippet: Span, e: &crate::snippets::Write) -> Option<CodeAction> {
        let a = self.action_with_change(title, kind, m)?;
        Some(match a.edits.iter().position(|x| x.span == snippet) {
            Some(i) => a.with_write(i, e),
            None => a,
        })
    }

    fn action_with_change(&self, title: &str, kind: &str, m: Change) -> Option<CodeAction> {
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: title.into(),
            kind: kind.into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// As assistências do Flutter no cursor.
    pub(crate) fn flutter_assists(&self, uri: &str, start: usize, end: usize) -> Vec<CodeAction> {
        let Some(no) = self.tree.localizar(start, end) else { return Vec::new() };
        let mut v = Vec::new();
        v.extend(self.flutter_to_children(uri, no));
        v.extend(self.flutter_move(uri, no, false));
        v.extend(self.flutter_move(uri, no, true));
        v.extend(self.flutter_remove(uri, no));
        v.extend(self.flutter_swap_with_child(uri, no));
        v.extend(self.flutter_swap_with_parent(uri, no));
        v.extend(self.flutter_wrap_builder(uri, no, false));
        v.extend(self.flutter_wrap_list(uri, no));
        v.extend(self.flutter_wrap_builder(uri, no, true));
        v.extend(self.flutter_child_last(uri, no));
        v.extend(self.flutter_wrap(uri, no, start, end));
        v
    }

    /// O nome declarado (`library a.b;`) da biblioteca da classe.
    fn class_library_name(&self, c: ClassId) -> String {
        let p = self.p.program();
        p.library(p.class(c).library).name.as_ref().map(|ns| ns.iter().map(|&s| self.p.name(s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
    }

    /// O tipo e os supertipos (`[self, ...allSupertypes]`) têm a classe
    /// `name` (pelo critério `accepts`).
    fn type_or_supertype(&self, t: TypeId, accepts: &dyn Fn(ClassId) -> bool) -> bool {
        let Some(c) = self.flutter_type_class(t) else { return false };
        accepts(c) || self.p.supertypes(c).into_iter().any(accepts)
    }

    /// `AddDiagnosticPropertyReference` (o assist, sem o modo em lote).
    pub(crate) fn add_diagnostic_reference(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let p = self.p.program();
        let query = &self.p.query;
        let no = self.tree.localizar(start, end)?;
        // O nome e o tipo (`_getReturnType`: o getter declarado ou o campo).
        let (name, ty, written_type): (String, TypeId, Option<usize>) = match self.kind(no) {
            "MethodDeclaration" => {
                let crate::arvore_analyzer::Marca::Funcao(fid) = self.tree.nos[no].marca else { return None };
                let func = self.ast.function(fid);
                let n = func.name?;
                if func.kind != dartforge_frontend::ast::FunctionKind::Getter {
                    return None;
                }
                let f = self.p.node_function(self.unit, fid)?;
                let ret = query.outline.functions.get(f.0 as usize)?.return_type;
                let annotation = self.children(no).iter().copied().find(|&k| matches!(self.kind(k), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"));
                (self.p.name(n.sym).to_string(), ret, annotation)
            }
            "VariableDeclaration" => {
                let list = self.parent(no).filter(|&l| self.kind(l) == "VariableDeclarationList")?;
                self.parent(list).filter(|&d| self.kind(d) == "FieldDeclaration")?;
                let name = crate::refactorings_exec::name_at_start_pub(self.node_text(no)).to_string();
                let class_node = self.with_parents(no).find(|&k| self.kind(k) == "ClassDeclaration")?;
                let crate::arvore_analyzer::Marca::Decl(d) = self.tree.nos[class_node].marca else { return None };
                let class = self.declaration_class(self.unit, d)?;
                let v = p.class(class).fields.iter().copied().find(|&v| self.p.name(p.variable(v).name) == name)?;
                let ty = query.variable_type(v)?;
                let annotation = self.children(list).iter().copied().find(|&k| matches!(self.kind(k), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"));
                (name, ty, annotation)
            }
            _ => return None,
        };
        let class_node = self.with_parents(no).find(|&k| self.kind(k) == "ClassDeclaration")?;
        let crate::arvore_analyzer::Marca::Decl(d) = self.tree.nos[class_node].marca else { return None };
        let class = self.declaration_class(self.unit, d)?;
        let diagnosable = |c: ClassId| self.p.name(p.class(c).name) == "Diagnosticable" && p.class(c).decl.is_some_and(|x| p.unit(x.unit).uri == "package:flutter/src/foundation/diagnostics.dart");
        if !(diagnosable(class) || self.p.supertypes(class).into_iter().any(diagnosable)) {
            return None;
        }
        let table = &query.table;
        let dynamic = matches!(table.get(ty), Type::Dynamic) || table.e_invalido(ty);
        let mut arguments: Option<Vec<TypeId>> = None;
        let mut constructor_name = "";
        let id = if matches!(table.get(ty), Type::Function { .. }) {
            arguments = Some(vec![ty]);
            constructor_name = ".has";
            "ObjectFlagProperty"
        } else if self.flutter_type_class(ty) == query.core.int_class && query.core.int_class.is_some() {
            "IntProperty"
        } else if self.flutter_type_class(ty) == query.core.double_class && query.core.double_class.is_some() {
            "DoubleProperty"
        } else if self.flutter_type_class(ty) == query.core.string_class && query.core.string_class.is_some() {
            "StringProperty"
        } else if self.flutter_type_class(ty).is_some_and(|c| p.class(c).kind == ClassKind::Enum) {
            arguments = Some(vec![ty]);
            "EnumProperty"
        } else if let Some(iterable) = query.core.iterable_class
            && self.type_or_supertype(ty, &|c| c == iterable)
        {
            arguments = Some(match table.get(ty) {
                Type::Interface { args, .. } => args.to_vec(),
                _ => Vec::new(),
            });
            "IterableProperty"
        } else if self.type_or_supertype(ty, &|c| self.p.name(p.class(c).name) == "Color" && self.class_library_name(c) == "dart.ui") {
            "ColorProperty"
        } else if self.type_or_supertype(ty, &|c| self.p.name(p.class(c).name) == "Matrix4" && self.class_library_name(c) == "vector_math_64") {
            "TransformProperty"
        } else {
            if !dynamic {
                arguments = Some(vec![ty]);
            }
            "DiagnosticsProperty"
        };
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let mut writer = crate::write_type::Writer::new(self, self.tree.nos[no].inicio);
        let mut reference = |prefix: &str, constructor: &str| -> Option<String> {
            let mut s = format!("{prefix}{constructor}.add({id}");
            match &arguments {
                Some(args) => {
                    let texts: Vec<String> = args.iter().map(|&a| writer.write(a, false).unwrap_or_default()).collect();
                    s.push_str(&format!("<{}>", texts.join(", ")));
                }
                None if dynamic => {
                    if let Some(a) = written_type {
                        let text = self.node_text(a);
                        if text != "dynamic" {
                            s.push_str(&format!("<{text}>"));
                        }
                    }
                }
                None => {}
            }
            s.push_str(&format!("{constructor_name}('{name}', {name}));{eol}"));
            Some(s)
        };
        let mut m = Change::default();
        let members = self.container_members(class_node);
        let fill: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&k| self.kind(k) == "MethodDeclaration" && matches!(self.tree.nos[k].marca, crate::arvore_analyzer::Marca::Funcao(f) if self.ast.function(f).name.is_some_and(|n| self.p.name(n.sym) == "debugFillProperties")))
            .collect();
        match fill[..] {
            [] => {
                let indent1 = ONE_INDENT;
                let indent2 = format!("{ONE_INDENT}{ONE_INDENT}");
                let body = reference(&indent2, "properties")?;
                let text = format!("@override{eol}{indent1}void debugFillProperties(DiagnosticPropertiesBuilder properties) {{{eol}{indent2}super.debugFillProperties(properties);{eol}{body}{indent1}}}");
                let (offset, inserted) = self.insert_in_member(class_node, crate::insert::Filter::Method, &text)?;
                m.add(uri, Span { start: offset, end: offset }, inserted);
            }
            [method] => {
                let body = *self.children(method).last()?;
                if self.kind(body) != "BlockFunctionBody" {
                    return None;
                }
                let block = *self.children(body).first()?;
                let commands = self.children(block).to_vec();
                let (offset, prefix) = match commands.last() {
                    None => {
                        let o = self.tree.nos[block].inicio;
                        (o, format!("{}{ONE_INDENT}", tx.line_prefix(o)))
                    }
                    Some(&c) => {
                        let o = self.previous_token(self.tree.nos[c].fim)?.start;
                        (o, tx.line_prefix(o).to_string())
                    }
                };
                // O parâmetro `DiagnosticPropertiesBuilder x`.
                let list = self.children(method).iter().copied().find(|&k| self.kind(k) == "FormalParameterList")?;
                let constructor = self.children(list).iter().copied().find_map(|q| {
                    if self.kind(q) != "SimpleFormalParameter" {
                        return None;
                    }
                    let ty = self.children(q).iter().copied().find(|&k| self.kind(k) == "NamedType")?;
                    let type_name_ = crate::refactorings_exec::name_at_start_pub(self.node_text(ty).rsplit('.').next().unwrap_or(""));
                    if type_name_ != "DiagnosticPropertiesBuilder" {
                        return None;
                    }
                    let n = self.previous_token(self.tree.nos[q].fim)?;
                    Some(self.source[n.start..n.end].to_string())
                })?;
                let text = reference(&prefix, &constructor)?;
                let next = tx.next_line(offset);
                m.add(uri, Span { start: next, end: next }, text);
            }
            _ => return None,
        }
        crate::refactorings_move::builder_imports(self, &mut m, &writer.import);
        self.action_with_change("Add a debug reference to this property", "refactor.add.diagnosticPropertyReference", m)
    }

    /// `FlutterConvertToChildren`.
    fn flutter_to_children(&self, uri: &str, n: usize) -> Option<CodeAction> {
        if self.kind(n) != "SimpleIdentifier" || self.node_text(n) != "child" {
            return None;
        }
        let label = self.parent(n).filter(|&p| self.kind(p) == "Label")?;
        let named = self.parent(label).filter(|&p| self.kind(p) == "NamedExpression")?;
        // `node.element != null`: o rótulo resolve para o parâmetro.
        let invocation = self.parent(named).and_then(|l| self.parent(l))?;
        let f = match self.node_expr(invocation).and_then(|x| self.bodies.get_resolved(x)) {
            Some(Resolved::Constructor(f)) | Some(Resolved::Element(Element::Function(f))) => *f,
            Some(Resolved::Member { member: dartforge_types::MemberRef::Function(f), .. }) => *f,
            _ => return None,
        };
        let child = self.p.query.names.lookup("child")?;
        if !self.p.query.outline.functions.get(f.0 as usize)?.parameters.iter().any(|q| q.kind == dartforge_frontend::ast::ParameterKind::Named && q.name == Some(child)) {
            return None;
        }
        let arg = self.named_expression(named)?;
        if !self.is_expression_widget(arg) {
            return None;
        }
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let mut m = Change::default();
        let named_start = self.tree.nos[named].inicio;
        let (start_arg, end_arg) = (self.tree.nos[arg].inicio, self.tree.nos[arg].fim);
        let empty = |o: usize| Span { start: o, end: o };
        m.add(uri, empty(named_start + "child".len()), "ren");
        let source_arg = self.node_text(arg);
        if !source_arg.contains(eol) {
            m.add(uri, empty(start_arg), "[");
            m.add(uri, empty(end_arg), "]");
        } else {
            let mut new_line = source_arg.rfind(eol).unwrap_or(0);
            if new_line == source_arg.len() {
                new_line -= 1;
            }
            let old_indent = tx.line_prefix(start_arg + eol.len() + new_line).to_string();
            let new_indent = format!("{old_indent}{ONE_INDENT}");
            let separator = &self.source[named_start..start_arg];
            let prefix = if separator.contains(eol) { String::new() } else { format!("{eol}{new_indent}") };
            if prefix.is_empty() {
                m.add(uri, empty(named_start + "child:".len()), " [");
                m.add(uri, Span { start: start_arg - 2, end: start_arg }, "");
            } else {
                m.add(uri, empty(start_arg), "[");
            }
            let new = tx.swap_indent(source_arg, &old_indent, &new_indent, false, false);
            m.add(uri, Span { start: start_arg, end: end_arg }, format!("{prefix}{new},{eol}{old_indent}]"));
        }
        self.action_with_change("Convert to children:", "refactor.flutter.convert.childToChildren", m)
    }

    /// `FlutterMoveUp`/`FlutterMoveDown`.
    fn flutter_move(&self, uri: &str, n: usize, downward: bool) -> Option<CodeAction> {
        let widget = self.find_expression_widget(n)?;
        let list = self.parent(widget).filter(|&p| self.kind(p) == "ListLiteral")?;
        let elements: Vec<usize> = self.children(list).iter().copied().filter(|&k| self.kind(k) != "TypeArgumentList").collect();
        let i = elements.iter().position(|&k| k == widget)?;
        let other = if downward {
            if i + 1 == elements.len() {
                return None;
            }
            elements[i + 1]
        } else {
            if i == 0 {
                return None;
            }
            elements[i - 1]
        };
        let mut m = Change::default();
        m.add(uri, self.tree.span(other), self.node_text(widget));
        m.add(uri, self.tree.span(widget), self.node_text(other));
        let (title, kind) = if downward { ("Move widget down", "refactor.flutter.move.down") } else { ("Move widget up", "refactor.flutter.move.up") };
        self.action_with_change(title, kind, m)
    }

    /// `FlutterRemoveWidget`.
    fn flutter_remove(&self, uri: &str, n: usize) -> Option<CodeAction> {
        let creation = self.find_creation(n)?;
        if !self.widget_creation(creation) {
            return None;
        }
        let list_elements = |list: usize| -> Vec<usize> { self.children(list).iter().copied().filter(|&k| self.kind(k) != "TypeArgumentList").collect() };
        let m = if let Some(a) = self.flutter_named_argument(creation, "children") {
            let e = self.named_expression(a)?;
            if self.kind(e) != "ListLiteral" || list_elements(e).is_empty() {
                return None;
            }
            self.flutter_remove_children(uri, creation, &list_elements(e))?
        } else if let Some(a) = self.flutter_named_argument(creation, "child") {
            self.flutter_remove_one(uri, creation, self.named_expression(a)?)
        } else if let Some(a) = self.flutter_named_argument(creation, "builder") {
            let e = self.named_expression(a)?;
            if self.kind(e) != "FunctionExpression" {
                return None;
            }
            // O primeiro parâmetro do builder sem uso no corpo.
            let list = self.children(e).iter().copied().find(|&k| self.kind(k) == "FormalParameterList")?;
            let parameter = *self.children(list).first()?;
            let name = self.previous_token(self.tree.nos[parameter].fim)?;
            let body = *self.children(e).last()?;
            // `_UsageFinder`: um identificador do corpo que lê o parâmetro.
            let used = self.tree.nos.iter().any(|no| {
                no.especie == "SimpleIdentifier"
                    && no.inicio >= self.tree.nos[body].inicio
                    && no.fim <= self.tree.nos[body].fim
                    && matches!(no.marca, crate::arvore_analyzer::Marca::Expr(x) if self.bodies.declaracao_local(x) == Some(name.start))
            });
            if used {
                return None;
            }
            let expression = match self.kind(body) {
                "BlockFunctionBody" => {
                    let block = *self.children(body).first()?;
                    let [r] = self.children(block)[..] else { return None };
                    if self.kind(r) != "ReturnStatement" {
                        return None;
                    }
                    *self.children(r).first()?
                }
                "ExpressionFunctionBody" => *self.children(body).first()?,
                _ => return None,
            };
            self.flutter_remove_one(uri, creation, expression)
        } else if let Some(a) = self.flutter_named_argument(creation, "slivers") {
            let e = self.named_expression(a)?;
            if self.kind(e) != "ListLiteral" || list_elements(e).is_empty() {
                return None;
            }
            self.flutter_remove_children(uri, creation, &list_elements(e))?
        } else if let Some(a) = self.flutter_named_argument(creation, "sliver") {
            self.flutter_remove_one(uri, creation, self.named_expression(a)?)
        } else {
            // `_removeSingleWhenInList`.
            let list = self.parent(creation).filter(|&p| self.kind(p) == "ListLiteral")?;
            let mut m = Change::default();
            m.add(uri, self.node_in_list(&list_elements(list), creation), "");
            m
        };
        self.action_with_change("Remove this widget", "refactor.flutter.removeWidget", m)
    }

    fn flutter_remove_children(&self, uri: &str, creation: usize, children: &[usize]) -> Option<Change> {
        if children.len() > 1 && self.parent(creation).is_none_or(|p| self.kind(p) != "ListLiteral") {
            return None;
        }
        let tx = Text::new(self.source);
        let (first, last) = (*children.first()?, *children.last()?);
        let text = &self.source[self.tree.nos[first].inicio..self.tree.nos[last].fim];
        let old = tx.line_prefix(self.tree.nos[first].inicio);
        let new = tx.line_prefix(self.tree.nos[creation].inicio);
        let mut m = Change::default();
        m.add(uri, self.tree.span(creation), tx.swap_indent(text, old, new, false, false));
        Some(m)
    }

    fn flutter_remove_one(&self, uri: &str, creation: usize, expression: usize) -> Change {
        let tx = Text::new(self.source);
        let old = tx.line_prefix(self.tree.nos[expression].inicio);
        let new = tx.line_prefix(self.tree.nos[creation].inicio);
        let mut m = Change::default();
        m.add(uri, self.tree.span(creation), tx.swap_indent(self.node_text(expression), old, new, false, false));
        m
    }

    /// `_singleChildInChildren`.
    fn single_child_in_children(&self, creation: usize) -> Option<usize> {
        let a = self.flutter_named_argument(creation, "children")?;
        let list = self.named_expression(a).filter(|&e| self.kind(e) == "ListLiteral")?;
        let elements: Vec<usize> = self.children(list).iter().copied().filter(|&k| self.kind(k) != "TypeArgumentList").collect();
        let [single] = elements[..] else { return None };
        (self.kind(single) == "InstanceCreationExpression").then_some(single)
    }

    /// `FlutterSwapWithChild`.
    fn flutter_swap_with_child(&self, uri: &str, n: usize) -> Option<CodeAction> {
        let parent = self.find_creation(n)?;
        if !self.widget_creation(parent) {
            return None;
        }
        let mut parent_with_one_child = true;
        let mut child = None;
        if let Some(first) = self.single_child_in_children(parent) {
            child = Some(first);
            parent_with_one_child = false;
        }
        let child = child.or_else(|| self.flutter_named_argument(parent, "child").and_then(|a| self.named_expression(a)))?;
        if self.kind(child) != "InstanceCreationExpression" || !self.widget_creation(child) {
            return None;
        }
        self.swap_parent_and_child(uri, parent, child, parent_with_one_child, "Swap with child", "refactor.flutter.swap.withChild")
    }

    /// `FlutterSwapWithParent`.
    fn flutter_swap_with_parent(&self, uri: &str, n: usize) -> Option<CodeAction> {
        let child = self.find_creation(n)?;
        if !self.widget_creation(child) {
            return None;
        }
        let mut parent_with_one_child = true;
        let mut named = None;
        if let Some(list) = self.parent(child).filter(|&p| self.kind(p) == "ListLiteral") {
            let elements = self.children(list).iter().filter(|&&k| self.kind(k) != "TypeArgumentList").count();
            if elements != 1 {
                return None;
            }
            if let Some(p) = self.parent(list).filter(|&p| self.kind(p) == "NamedExpression") {
                named = Some(p);
                parent_with_one_child = false;
            }
        }
        let base = named.or_else(|| self.parent(child))?;
        let expr = self.parent(base).and_then(|x| self.parent(x))?;
        if self.kind(expr) != "InstanceCreationExpression" {
            return None;
        }
        self.swap_parent_and_child(uri, expr, child, parent_with_one_child, "Swap with parent", "refactor.flutter.swap.withParent")
    }

    /// `swapParentAndChild`.
    fn swap_parent_and_child(&self, uri: &str, parent: usize, child: usize, parent_with_one_child: bool, title: &str, kind: &str) -> Option<CodeAction> {
        let stable = match self.single_child_in_children(child) {
            Some(first) => first,
            None => self.flutter_named_argument(child, "child")?,
        };
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let child_args = self.flutter_argument_list(child)?;
        let parent_args = self.flutter_argument_list(parent)?;
        let text_child = &self.source[self.tree.nos[child].inicio..self.tree.nos[child_args].inicio];
        let text_parent = &self.source[self.tree.nos[parent].inicio..self.tree.nos[parent_args].inicio];
        let parent_indent = tx.line_prefix(self.tree.nos[parent].inicio).to_string();
        let child_indent = format!("{parent_indent}  ");
        let mut s = String::new();
        s.push_str(text_child);
        s.push('(');
        s.push_str(eol);
        for &a in self.children(child_args) {
            if a != stable {
                let text = tx.swap_indent(self.node_text(a), &child_indent, &parent_indent, false, false);
                s.push_str(&format!("{parent_indent}  {text},{eol}"));
            }
        }
        s.push_str(&format!("{parent_indent}  child: {text_parent}({eol}"));
        for &a in self.children(parent_args) {
            let label = if self.kind(a) == "NamedExpression" { self.named_label(a) } else { None };
            if !matches!(label, Some("child") | Some("children")) {
                let text = tx.swap_indent(self.node_text(a), &parent_indent, &child_indent, false, false);
                s.push_str(&format!("{child_indent}  {text},{eol}"));
            }
        }
        {
            let mut text = self.node_text(stable).to_string();
            if text.trim().starts_with("child: ") {
                text = text["child: ".len()..].to_string();
            } else if text.trim().starts_with("children: ") {
                text = text["children: ".len()..].to_string();
            }
            s.push_str(&child_indent);
            s.push_str("  ");
            if parent_with_one_child {
                s.push_str("child: ");
            } else {
                s.push_str("children: [");
                s.push_str(eol);
                s.push_str(&child_indent);
                s.push_str("    ");
            }
            s.push_str(&text);
            s.push(',');
            s.push_str(eol);
        }
        s.push_str(&child_indent);
        if !parent_with_one_child {
            s.push_str("  ]");
            s.push(',');
            s.push_str(eol);
            s.push_str(&child_indent);
        }
        s.push_str("),");
        s.push_str(eol);
        s.push_str(&parent_indent);
        s.push(')');
        let mut m = Change::default();
        m.add(uri, self.tree.span(parent), s);
        self.action_with_change(title, kind, m)
    }

    /// `FlutterWrapBuilder` e `FlutterWrapStreamBuilder`.
    fn flutter_wrap_builder(&self, uri: &str, n: usize, stream: bool) -> Option<CodeAction> {
        let widget = self.find_expression_widget(n)?;
        let t = self.flutter_node_type(widget)?;
        let (name, uri_class) = if stream { ("StreamBuilder", URI_ASYNC) } else { ("Builder", URI_BASIC) };
        if self.flutter_type_class(t).is_some_and(|c| self.flutter_exactly(c, name, uri_class)) {
            return None;
        }
        let class = self.flutter_class(URI_WIDGETS, name)?;
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let mut writer = crate::write_type::Writer::new(self, self.tree.nos[widget].inicio);
        let reference = writer.reference_of(Element::Class(class), name);
        let indent = tx.line_prefix(self.tree.nos[widget].inicio).to_string();
        let indent1 = format!("{indent}{ONE_INDENT}");
        let indent2 = format!("{indent}{ONE_INDENT}{ONE_INDENT}");
        let commas = crate::refactorings_exec::enabled_rule(self.p, self.unit, "require_trailing_commas");
        let source_widget = tx.swap_indent(self.node_text(widget), &indent, &indent2, false, false);
        let mut s = reference;
        let mut marks = crate::snippets::Write::default();
        if stream {
            // `addSimpleLinkedEdit('type', 'Object')`.
            s.push('<');
            marks.text = s.clone();
            marks.linked("type", "Object", &[]);
            s = marks.text.clone();
            s.push_str(">(");
        } else {
            s.push('(');
        }
        s.push_str(eol);
        if stream {
            s.push_str(&format!("{indent1}stream: null,{eol}"));
            s.push_str(&format!("{indent1}builder: (context, snapshot) {{{eol}"));
        } else {
            s.push_str(&format!("{indent1}builder: (context) {{{eol}"));
        }
        s.push_str(&format!("{indent2}return {source_widget};{eol}"));
        s.push_str(&format!("{indent1}}}{}{eol}", if commas { "," } else { "" }));
        s.push_str(&indent);
        s.push(')');
        let mut m = Change::default();
        m.add(uri, self.tree.span(widget), s);
        crate::refactorings_move::builder_imports(self, &mut m, &writer.import);
        let (title, kind) = if stream { ("Wrap with StreamBuilder", "refactor.flutter.wrap.streamBuilder") } else { ("Wrap with Builder", "refactor.flutter.wrap.builder") };
        self.linked_action(title, kind, m, self.tree.span(widget), &marks)
    }

    /// `FlutterWrapGeneric` (o produtor da lista de widgets).
    fn flutter_wrap_list(&self, uri: &str, n: usize) -> Option<CodeAction> {
        if self.kind(n) != "ListLiteral" {
            return None;
        }
        let elements: Vec<usize> = self.children(n).iter().copied().filter(|&k| self.kind(k) != "TypeArgumentList").collect();
        if elements.iter().any(|&e| !(self.kind(e) == "InstanceCreationExpression" && self.widget_creation(e))) {
            return None;
        }
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let source = self.node_text(n);
        let new_line = source.rfind(eol)?;
        if new_line == source.len() - 1 {
            return None;
        }
        let indent = tx.line_prefix(self.tree.nos[n].inicio + eol.len() + new_line).to_string();
        let indent_arg = format!("{indent}{ONE_INDENT}");
        let list_indent = format!("{indent}{ONE_INDENT}{ONE_INDENT}");
        let list = tx.swap_indent(source, &indent, &list_indent, false, false);
        // `addSimpleLinkedEdit('WIDGET', 'widget')`.
        let mut marks = crate::snippets::Write::default();
        marks
            .write(&format!("[{eol}{indent_arg}"))
            .linked("WIDGET", "widget", &[])
            .write(&format!("({eol}{list_indent}children: {list},{eol}{indent_arg}),{eol}{indent}]"));
        let mut m = Change::default();
        m.add(uri, self.tree.span(n), marks.text.clone());
        self.linked_action("Wrap with widget...", "refactor.flutter.wrap.generic", m, self.tree.span(n), &marks)
    }

    /// `SortChildPropertyLast`.
    fn flutter_child_last(&self, uri: &str, n: usize) -> Option<CodeAction> {
        let child = if self.kind(n) == "NamedExpression" && matches!(self.named_label(n), Some("child") | Some("children")) {
            n
        } else {
            self.named_argument_in_label(n, "child").or_else(|| self.named_argument_in_label(n, "children"))?
        };
        let list = self.parent(child)?;
        let creation = self.parent(list)?;
        if self.kind(creation) != "InstanceCreationExpression" || !self.widget_creation(creation) {
            return None;
        }
        let arguments = self.children(list).to_vec();
        let last = *arguments.last()?;
        if last == child {
            return None;
        }
        let after = |k: usize| self.next_token(self.tree.nos[k].fim);
        let trailing_comma = after(last).is_some_and(|t| &self.source[t.start..t.end] == ",");
        let child_start = self.previous_token(self.tree.nos[child].inicio)?.end;
        let mut child_end = after(child)?.end;
        let mut exclusion = Span { start: child_start, end: child_end };
        if arguments.first() == Some(&child) {
            exclusion = Span { start: self.tree.nos[child].inicio, end: self.tree.nos[*arguments.get(1)?].inicio };
        }
        if !trailing_comma {
            child_end = self.tree.nos[child].fim;
        }
        let mut text = self.source[child_start..child_end].to_string();
        let mut insertion = self.tree.nos[last].fim;
        if trailing_comma {
            insertion = after(last)?.end;
        } else if child_start == self.tree.nos[child].inicio {
            text = format!(", {text}");
        } else {
            text = format!(",{text}");
        }
        let mut m = Change::default();
        m.add(uri, exclusion, "");
        m.add(uri, Span { start: insertion, end: insertion }, text);
        self.action_with_change("Move child property to end of arguments", "refactor.sort.child.properties.last", m)
    }

    /// `FlutterWrap`: o embrulho genérico e os de `Center`, `Container`,
    /// `Padding`, `SizedBox` de um widget; `Column` e `Row` de um ou mais
    /// widgets selecionados.
    fn flutter_wrap(&self, uri: &str, n: usize, start: usize, end: usize) -> Vec<CodeAction> {
        let mut v = Vec::new();
        if let Some(widget) = self.find_expression_widget(n)
            && let Some(t) = self.flutter_node_type(widget)
        {
            let class = self.flutter_type_class(t);
            let exact = |name: &str, uri_c: &str| class.is_some_and(|c| self.flutter_exactly(c, name, uri_c));
            v.extend(self.wrap_one(uri, widget, None));
            if !exact("Center", URI_BASIC) {
                v.extend(self.wrap_one(uri, widget, Some("Center")));
            }
            if !exact("Container", URI_CONTAINER) {
                v.extend(self.wrap_one(uri, widget, Some("Container")));
            }
            if !exact("Padding", URI_BASIC) {
                v.extend(self.wrap_one(uri, widget, Some("Padding")));
            }
            if !exact("SizedBox", URI_BASIC) {
                v.extend(self.wrap_one(uri, widget, Some("SizedBox")));
            }
        }
        // `_wrapMultipleWidgets`.
        let analysis = self.analyze(start, end);
        let mut widgets: Vec<usize> = Vec::new();
        if !analysis.selected.is_empty() {
            for &s in &analysis.selected {
                let mut s = s;
                if self.kind(s) == "ConstructorName"
                    && let Some(p) = self.parent(s).filter(|&p| self.kind(p) == "InstanceCreationExpression")
                {
                    s = p;
                }
                if !self.is_expression(s) || !self.is_expression_widget(s) {
                    return v;
                }
                widgets.push(s);
            }
        } else if let Some(mut coverage) = analysis.coverage {
            if self.kind(coverage) == "ArgumentList"
                && self.tree.nos[coverage].inicio == start
                && let Some(p) = self.parent(coverage)
            {
                coverage = p;
            }
            if let Some(w) = self.find_expression_widget(coverage) {
                widgets.push(w);
            }
        }
        let (Some(&first), Some(&last)) = (widgets.first(), widgets.last()) else { return v };
        for name in ["Column", "Row"] {
            v.extend(self.wrap_several(uri, first, last, name));
        }
        v
    }

    /// `_WrapSingleWidget`.
    fn wrap_one(&self, uri: &str, widget: usize, name: Option<&str>) -> Option<CodeAction> {
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let mut source = self.node_text(widget).to_string();
        let class = match name {
            Some(x) => Some(self.flutter_class(URI_WIDGETS, x)?),
            None => None,
        };
        let mut writer = crate::write_type::Writer::new(self, self.tree.nos[widget].inicio);
        // Sem classe, os grupos `WIDGET` e `CHILD`; com ela, a seleção logo
        // depois do `(` (`flutter_wrap.dart:302-345`).
        let mut marks = crate::snippets::Write::default();
        match (class, name) {
            (Some(c), Some(x)) => {
                marks.write(&writer.reference_of(Element::Class(c), x));
                marks.write("(").here();
            }
            _ => {
                marks.linked("WIDGET", "widget", &[]).write("(");
            }
        }
        let mut s = marks.text.clone();
        let lines: Vec<String> = if name == Some("Padding") {
            let word = if crate::create::in_constant_context(self, widget) { "" } else { " const" };
            let value = if crate::refactorings_exec::enabled_rule(self.p, self.unit, "prefer_int_literals") { "8" } else { "8.0" };
            vec![format!("padding:{word} EdgeInsets.all({value}),")]
        } else {
            Vec::new()
        };
        if source.contains(eol) || !lines.is_empty() {
            let indent = tx.line_prefix(self.tree.nos[widget].inicio).to_string();
            let new_indent = format!("{indent}{ONE_INDENT}");
            for l in &lines {
                s.push_str(eol);
                s.push_str(&new_indent);
                s.push_str(l);
            }
            s.push_str(eol);
            s.push_str(&new_indent);
            source = tx.swap_indent(&source, &indent, &new_indent, false, false);
            source = format!("{source},{eol}{indent}");
        }
        marks.text = s;
        if class.is_none() {
            marks.linked("CHILD", "child", &[]);
        } else {
            marks.write("child");
        }
        marks.write(": ").write(&source).write(")");
        let mut m = Change::default();
        m.add(uri, self.tree.span(widget), marks.text.clone());
        crate::refactorings_move::builder_imports(self, &mut m, &writer.import);
        let (title, kind) = match name {
            None => ("Wrap with widget...".to_string(), "refactor.flutter.wrap.generic".to_string()),
            Some("SizedBox") => ("Wrap with SizedBox".to_string(), "refactor.flutter.wrap.sizedBox".to_string()),
            Some(x) => (format!("Wrap with {x}"), format!("refactor.flutter.wrap.{}", x.to_lowercase())),
        };
        self.linked_action(&title, &kind, m, self.tree.span(widget), &marks)
    }

    /// `_WrapMultipleWidgets`.
    fn wrap_several(&self, uri: &str, first: usize, last: usize, name: &str) -> Option<CodeAction> {
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let range = Span { start: self.tree.nos[first].inicio, end: self.tree.nos[last].fim };
        let source = &self.source[range.start..range.end];
        let class = self.flutter_class(URI_WIDGETS, name)?;
        self.flutter_class(URI_WIDGETS, "Widget")?;
        let mut writer = crate::write_type::Writer::new(self, range.start);
        let mut s = writer.reference_of(Element::Class(class), name);
        s.push('(');
        let indent = tx.line_prefix(range.start).to_string();
        let indent1 = format!("{indent}{ONE_INDENT}");
        let indent2 = format!("{indent}{ONE_INDENT}{ONE_INDENT}");
        s.push_str(eol);
        s.push_str(&indent1);
        s.push_str("children: [");
        s.push_str(eol);
        s.push_str(&indent2);
        s.push_str(&tx.swap_indent(source, &indent, &indent2, false, false));
        s.push(',');
        s.push_str(eol);
        s.push_str(&indent1);
        s.push_str("],");
        s.push_str(eol);
        s.push_str(&indent);
        s.push(')');
        let mut m = Change::default();
        m.add(uri, range, s);
        crate::refactorings_move::builder_imports(self, &mut m, &writer.import);
        self.action_with_change(&format!("Wrap with {name}"), &format!("refactor.flutter.wrap.{}", name.to_lowercase()), m)
    }
}
