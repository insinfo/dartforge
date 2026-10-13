//! `ConvertClassToEnum` (convert_class_to_enum.dart) from `analysis_server`
//! 3.6.2: a private class (or one with private constructors) whose static
//! constant fields are its instances becomes an `enum`.
//!
//! | Title | Kind |
//! |---|---|
//! | `Convert class to an enum` | `refactor.convert.classToEnum` |

use crate::actions::CodeAction;
use crate::arvore_analyzer::{Ligacao, Marca};
use crate::refactorings::Context;
use crate::refactorings_exec::{Text, ONE_INDENT};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_types::constantes::avaliador::Motor;
use dartforge_types::constantes::valor::{Campo, Estado, Valor};
use dartforge_types::{Resolved, Type};
use std::collections::HashSet;

/// `_Field`: the member (its position in `members`), the
/// `VariableDeclaration` node and the nodes of the list's variables.
struct ClassField {
    member: usize,
    variable: usize,
    variables: Vec<usize>,
}

/// `_ConstantField`.
struct ConstantField {
    field: ClassField,
    name: String,
    creation: usize,
    constructor: FunctionElementId,
    index: i64,
}

/// `_Parameter` of a used constructor: the constructor, the member and the
/// position of the `this.index` parameter in the list.
struct IndexParameter {
    constructor: FunctionElementId,
    member: usize,
    position: usize,
}

impl Context<'_> {
    /// The value of `IntegerLiteral.value` (decimal or `0x`, up to 64 bits in
    /// hexadecimal).
    fn literal_integer_value(&self, n: usize) -> Option<i64> {
        let text: String = self.node_text(n).chars().filter(|c| *c != '_').collect();
        match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            Some(hex) => u64::from_str_radix(hex, 16).ok().map(|v| v as i64),
            None => text.parse::<i64>().ok(),
        }
    }

    /// `ConvertClassToEnum`.
    pub(crate) fn convert_class_to_enum(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let prog = self.p.program();
        let query = &self.p.query;
        let lib = prog.unit(self.unit).library;
        if prog.library(lib).features.versao() < dartforge_frontend::features::LanguageVersion::new(2, 17) {
            return None;
        }
        // With parts, the creations and subclasses in them cannot be seen.
        if prog.library(lib).units.len() > 1 {
            return None;
        }
        let declaration = self.tree.localizar(start, end)?;
        if self.kind(declaration) != "ClassDeclaration" {
            return None;
        }
        let Marca::Decl(d) = self.tree.nos[declaration].marca else { return None };
        let DeclKind::Class(k) = &self.ast.decl(d).kind else { return None };
        // `declaration.name == token`.
        if k.mixin_application || start < k.name.span.start || start > k.name.span.end {
            return None;
        }
        let class_ = self.declaration_class(self.unit, d)?;
        if k.modifiers.abstract_ || k.modifiers.sealed {
            return None;
        }
        let data = query.outline.classes.get(class_.0 as usize)?;
        if k.extends.is_some()
            && let Some(s) = data.supertype
            && !matches!(query.table.get(s), Type::Interface { class, .. } if Some(*class) == query.core.object_class)
        {
            return None;
        }
        let class_children = self.children(declaration);
        let member_nodes: Vec<usize> = k
            .members
            .iter()
            .map(|&m| class_children.iter().copied().find(|&f| self.tree.nos[f].ligacao == Ligacao::Membro(m)))
            .collect::<Option<_>>()?;
        let name_of = |s: Span| &self.source[s.start..s.end];

        // `_validateConstructors`.
        let private = name_of(k.name.span).starts_with('_');
        let mut constructors: Vec<(FunctionElementId, usize)> = Vec::new();
        for (i, &m) in k.members.iter().enumerate() {
            let MemberKind::Constructor(c) = &self.ast.member(m).kind else { continue };
            let f = self.p.node_constructor(self.unit, m)?;
            let private_constructor = c.name.is_some_and(|n| name_of(n.span).starts_with('_'));
            if !private && !private_constructor {
                return None;
            }
            if !c.factory && !c.const_ {
                return None;
            }
            constructors.push((f, i));
        }

        // `_validateMethods`.
        for &m in k.members.iter() {
            if let MemberKind::Method(fid) = self.ast.member(m).kind
                && let Some(n) = self.ast.function(fid).name
                && matches!(name_of(n.span), "==" | "hashCode")
            {
                return None;
            }
        }

        // `_validateFields`.
        let field_variable = |m: ast::MemberId, j: usize| -> Option<VariableId> {
            prog.class(class_).fields.iter().copied().find(|&v| prog.variable(v).node == VariableRef::Field { unit: self.unit, member: m, index: j })
        };
        let mut table = query.table.clone();
        let mut engine = Motor::novo(prog, &query.names, &mut table, &query.core, &query.outline, &query.bodies, &self.p.libraries);
        let mut groups: Vec<(Valor, Vec<ConstantField>)> = Vec::new();
        let mut index_field: Option<ClassField> = None;
        for (i, &m) in k.members.iter().enumerate() {
            let MemberKind::Field(l) = &self.ast.member(m).kind else { continue };
            let list = self.children(member_nodes[i]).iter().copied().find(|&f| self.kind(f) == "VariableDeclarationList")?;
            let variables: Vec<usize> = self.children(list).iter().copied().filter(|&f| self.kind(f) == "VariableDeclaration").collect();
            if variables.len() != l.variables.len() {
                return None;
            }
            if l.static_ {
                for (j, v) in l.variables.iter().enumerate() {
                    let Some(var) = field_variable(m, j) else { continue };
                    if !prog.variable(var).const_ {
                        continue;
                    }
                    let Some(t) = query.variable_type(var) else { continue };
                    if !matches!(query.table.get(t), Type::Interface { class, .. } if *class == class_) {
                        continue;
                    }
                    if v.initializer.is_none() {
                        continue;
                    }
                    let Some(&creation) = self.children(variables[j]).first() else { continue };
                    if self.kind(creation) != "InstanceCreationExpression" {
                        continue;
                    }
                    let Some(Resolved::Constructor(f)) = self.node_expr(creation).and_then(|x| self.bodies.get_resolved(x)) else { continue };
                    let f = *f;
                    if prog.function(f).factory || prog.function(f).class != Some(class_) {
                        continue;
                    }
                    let Some(value) = engine.valor_de_variavel(var).and_then(|c| c.valor().cloned()) else { continue };
                    if l.variables.len() != 1 {
                        return None;
                    }
                    // `fieldValue.getField('index')?.toIntValue() ?? -1`.
                    let index = match &value.estado {
                        Estado::Generico { campos: fields, .. } => fields.iter().find_map(|(c, x)| match (c, &x.estado) {
                            (Campo::Nome(s), Estado::Int(Some(n))) if self.p.name(*s) == "index" => Some(*n),
                            _ => None,
                        }),
                        _ => None,
                    }
                    .unwrap_or(-1);
                    let field = ConstantField {
                        field: ClassField { member: i, variable: variables[j], variables: variables.clone() },
                        name: name_of(v.name.span).to_string(),
                        creation,
                        constructor: f,
                        index,
                    };
                    let mut found = None;
                    for (g, (key, _)) in groups.iter().enumerate() {
                        if engine.iguais(key, &value) {
                            found = Some(g);
                            break;
                        }
                    }
                    match found {
                        Some(g) => groups[g].1.push(field),
                        None => groups.push((value, vec![field])),
                    }
                }
            } else {
                for (j, v) in l.variables.iter().enumerate() {
                    if !l.final_ {
                        return None;
                    }
                    let Some(var) = field_variable(m, j) else { continue };
                    let is_int = query
                        .variable_type(var)
                        .is_some_and(|t| matches!(query.table.get(t), Type::Interface { class, .. } if Some(*class) == query.core.int_class));
                    if name_of(v.name.span) == "index" && is_int {
                        index_field = Some(ClassField { member: i, variable: variables[j], variables: variables.clone() });
                    }
                }
            }
        }
        drop(engine);
        let mut fields: Vec<ConstantField> = Vec::new();
        for (_, list) in groups {
            if list.len() != 1 {
                return None;
            }
            fields.extend(list);
        }
        if fields.is_empty() {
            return None;
        }

        // `_EnumVisitor` and `_NonEnumVisitor`: no creation through a generative
        // constructor of the class outside the initializer of a field to convert.
        let (ci, cf) = (self.tree.nos[declaration].inicio, self.tree.nos[declaration].fim);
        let allowed: HashSet<usize> = fields.iter().map(|c| c.creation).collect();
        for n in 0..self.tree.nos.len() {
            if self.kind(n) != "InstanceCreationExpression" {
                continue;
            }
            let generator = matches!(
                self.node_expr(n).and_then(|x| self.bodies.get_resolved(x)),
                Some(Resolved::Constructor(f)) if !prog.function(*f).factory && prog.function(*f).class == Some(class_)
            );
            if !generator {
                continue;
            }
            let inside = self.tree.nos[n].inicio >= ci && self.tree.nos[n].fim <= cf;
            if !inside || !allowed.contains(&n) {
                return None;
            }
        }
        for &other in self.p.program().unit(self.unit).unit.declarations.iter() {
            let DeclKind::Class(ko) = &self.ast.decl(other).kind else { continue };
            if ko.mixin_application || other == d {
                continue;
            }
            let Some(c) = self.declaration_class(self.unit, other) else { return None };
            let ce = prog.class(c);
            if ce.supertype_class == Some(class_) || ce.interface_classes.contains(&class_) || ce.mixin_classes.contains(&class_) {
                return None;
            }
        }

        // `_computeUsedConstructors`.
        let mut used: Vec<(FunctionElementId, usize)> = Vec::new();
        for c in fields.iter() {
            if used.iter().any(|&(f, _)| f == c.constructor) {
                continue;
            }
            if let Some(&pair) = constructors.iter().find(|&&(f, _)| f == c.constructor) {
                used.push(pair);
            }
        }
        // `_indexFieldData`.
        let map: Option<Vec<IndexParameter>> = match &index_field {
            None => None,
            Some(_) => {
                let mut map = Vec::new();
                for &(f, i) in used.iter() {
                    let MemberKind::Constructor(kc) = &self.ast.member(k.members[i]).kind else { return None };
                    let position = kc.parameters.iter().position(|p| p.this_ && p.name.is_some_and(|n| name_of(n.span) == "index"))?;
                    map.push(IndexParameter { constructor: f, member: i, position });
                }
                let mut values: Vec<i64> = Vec::new();
                for c in fields.iter() {
                    let pair = map.iter().find(|p| p.constructor == c.constructor)?;
                    let MemberKind::Constructor(kc) = &self.ast.member(k.members[pair.member]).kind else { return None };
                    // `getArgument`: the parameter's argument; that of a named one is the
                    // `NamedExpression`, which is not a literal.
                    if kc.parameters[pair.position].kind == ParameterKind::Named {
                        return None;
                    }
                    let arguments = self.list_arguments(c.creation);
                    let argument = *arguments.iter().filter(|&&a| self.kind(a) != "NamedExpression").nth(pair.position)?;
                    if self.kind(argument) != "IntegerLiteral" {
                        return None;
                    }
                    let v = self.literal_integer_value(argument)?;
                    if values.contains(&v) {
                        return None;
                    }
                    values.push(v);
                }
                values.sort_unstable();
                if !(values.len() == fields.len() && values[0] == 0 && *values.last()? == fields.len() as i64 - 1) {
                    return None;
                }
                Some(map)
            }
        };

        // `applyChanges`.
        let mut edits: Vec<(Span, String)> = Vec::new();
        let mut members_to_exclude: Vec<usize> = Vec::new();
        let class_word = self
            .tokens
            .iter()
            .filter(|t| t.span.end <= k.name.span.start && &self.source[t.span.start..t.span.end] == "class")
            .map(|t| t.span)
            .next_back()?;
        edits.push((class_word, "enum".to_string()));
        if let Some(&extends) = class_children.iter().find(|&&f| self.kind(f) == "ExtendsClause") {
            let next = self.next_token(self.tree.nos[extends].fim)?;
            edits.push((Span { start: self.tree.nos[extends].inicio, end: next.start }, String::new()));
        }
        let eol = Text::new(self.source).eol();
        let indent = ONE_INDENT;
        let mut constants = String::new();
        fields.sort_by_key(|c| c.index);
        let exclude_field = |field: &ClassField, edits: &mut Vec<(Span, String)>, members: &mut Vec<usize>| {
            if field.variables.len() == 1 {
                members.push(field.member);
            } else {
                edits.push((self.node_in_list(&field.variables, field.variable), String::new()));
            }
        };
        for field in fields.iter() {
            let documentation = self.children(member_nodes[field.field.member]).iter().copied().find(|&f| self.kind(f) == "Comment");
            if !constants.is_empty() {
                constants.push(',');
                constants.push_str(eol);
                if documentation.is_some() {
                    constants.push_str(eol);
                }
                constants.push_str(indent);
            }
            if let Some(doc) = documentation {
                constants.push_str(self.node_text(doc));
                constants.push_str(eol);
                constants.push_str(indent);
            }
            constants.push_str(&field.name);
            let parameter = map.as_ref().and_then(|m| m.iter().find(|p| p.constructor == field.constructor));
            let constructor_name = self.children(field.creation).iter().copied().find(|&f| self.kind(f) == "ConstructorName")?;
            let name_children = self.children(constructor_name);
            let ty = *name_children.first()?;
            let type_arguments = self.children(ty).iter().copied().find(|&f| self.kind(f) == "TypeArgumentList");
            if let Some(a) = type_arguments {
                constants.push_str(self.node_text(a));
            }
            let name = name_children.get(1).copied().filter(|&f| self.kind(f) == "SimpleIdentifier");
            if let Some(n) = name {
                constants.push('.');
                constants.push_str(self.node_text(n));
            }
            let list = self.children(field.creation).iter().copied().find(|&f| self.kind(f) == "ArgumentList")?;
            let arguments = self.list_arguments(field.creation);
            let how_many = arguments.len() - usize::from(parameter.is_some());
            if how_many == 0 {
                if type_arguments.is_some() || name.is_some() {
                    constants.push_str("()");
                }
            } else if let Some(pair) = parameter {
                constants.push('(');
                let index = pair.position;
                let last = arguments.len() - 1;
                let start_ = |a: usize| self.tree.nos[a].inicio;
                let end_of = |a: usize| self.tree.nos[a].fim;
                if index == 0 {
                    constants.push_str(&self.source[start_(arguments[1])..end_of(arguments[last])]);
                } else if index == last {
                    let comma = self.next_token(end_of(arguments[last])).is_some_and(|t| &self.source[t.start..t.end] == ",");
                    let until = if comma { start_(arguments[last]) } else { end_of(arguments[last - 1]) };
                    constants.push_str(&self.source[start_(arguments[0])..until]);
                } else {
                    constants.push_str(&self.source[start_(arguments[0])..start_(arguments[index])]);
                    let closes = self.tree.nos[list].fim - 1;
                    constants.push_str(&self.source[start_(arguments[index + 1])..closes]);
                }
                constants.push(')');
            } else {
                constants.push_str(self.node_text(list));
            }
            exclude_field(&field.field, &mut edits, &mut members_to_exclude);
        }
        if let Some(index) = &index_field {
            exclude_field(index, &mut edits, &mut members_to_exclude);
        }

        // `_removeUnnamedConstructor`.
        let mut removed: Option<usize> = None;
        let declared: Vec<usize> = (0..k.members.len()).filter(|&i| matches!(self.ast.member(k.members[i]).kind, MemberKind::Constructor(_))).collect();
        if let [i] = declared[..]
            && let MemberKind::Constructor(kc) = &self.ast.member(k.members[i]).kind
            && kc.name.is_none_or(|n| name_of(n.span) == "new")
        {
            let remaining = kc.parameters.len() as i64 - i64::from(map.as_ref().is_some_and(|m| !m.is_empty()));
            if remaining == 0 {
                members_to_exclude.push(i);
                removed = Some(i);
            }
        }
        // `_transformConstructors`.
        if let Some(map) = &map {
            for pair in map.iter() {
                if removed == Some(pair.member) {
                    continue;
                }
                let list = self.children(member_nodes[pair.member]).iter().copied().find(|&f| self.kind(f) == "FormalParameterList")?;
                let parameters = self.children(list).to_vec();
                edits.push((self.node_in_list(&parameters, *parameters.get(pair.position)?), String::new()));
            }
        }

        let title = "Convert class to an enum";
        let kind = "refactor.convert.classToEnum";
        let (first, last) = (*member_nodes.first()?, *member_nodes.last()?);
        if members_to_exclude.len() == member_nodes.len() {
            edits.push((Span { start: self.tree.nos[first].inicio, end: self.tree.nos[last].fim }, constants));
            return Some(self.simple_action(uri, title, kind, edits));
        }
        let key = self.previous_token(self.tree.nos[first].inicio)?;
        if &self.source[key.start..key.end] != "{" {
            return None;
        }
        edits.push((Span { start: key.end, end: key.end }, format!("{eol}{indent}{constants};{eol}")));
        // `range.nodesInList(members, membersToDelete)`.
        members_to_exclude.sort_unstable();
        let mut index_groups: Vec<(usize, usize)> = Vec::new();
        for &i in members_to_exclude.iter() {
            match index_groups.last_mut() {
                Some((_, sup)) if *sup + 1 == i => *sup = i,
                _ => index_groups.push((i, i)),
            }
        }
        for (inf, sup) in index_groups {
            let span = if inf == 0 {
                Span { start: self.tree.nos[member_nodes[inf]].inicio, end: self.tree.nos[*member_nodes.get(sup + 1)?].inicio }
            } else {
                Span { start: self.tree.nos[member_nodes[inf - 1]].fim, end: self.tree.nos[member_nodes[sup]].fim }
            };
            edits.push((span, String::new()));
        }
        Some(self.simple_action(uri, title, kind, edits))
    }
}
