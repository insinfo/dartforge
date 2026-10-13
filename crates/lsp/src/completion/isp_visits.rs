//! Os `visit*` do `InScopeCompletionPass` (um por espécie de nó) e os
//! auxiliares `_forX` (`in_scope_completion_pass.dart:241-3929`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Flags, Initial, Isp};
use super::*;
use dartforge_elements::model::UnitId;
use crate::arvore_analyzer::Ligacao;
use dartforge_frontend::token::Op;

/// O texto de um `completionLocation` de argumento.
fn argument_role(parent: &str) -> &'static str {
    match parent {
        "Annotation" => "annotation",
        "EnumConstantArguments" => "enumConstantArguments",
        "ExtensionOverride" => "extensionOverride",
        "FunctionExpressionInvocation" => "function",
        "InstanceCreationExpression" => "constructor",
        "MethodInvocation" => "method",
        "RedirectingConstructorInvocation" | "SuperConstructorInvocation" => "constructorRedirect",
        _ => "",
    }
}

/// Um parâmetro da invocação (`invokedFormalParameters`).
#[derive(Debug, Clone)]
pub(super) struct InvokedParameter {
    pub name: String,
    pub named: bool,
    pub required: bool,
    pub ty: TypeId,
    /// `isFlutterWidgetParameter`: o construtor é de um widget do Flutter.
    pub widget: bool,
    /// `hasRequired`: a anotação `@required` do `package:meta`.
    pub annotated_required: bool,
    /// Os nomes dos posicionais do tipo de função escrito no parâmetro (o
    /// `FunctionType.parameters` do analyzer tem os nomes; o tipo do
    /// DartForge não).
    pub type_names: Vec<String>,
}

impl<'a, 'c> Isp<'a, 'c> {
    /// `completionNode.accept(this)`: o `visit*` da espécie (nenhum para as
    /// espécies sem `visit` próprio: o `SimpleAstVisitor` não faz nada).
    pub(super) fn visit(&mut self, n: usize) {
        match self.kind(n) {
            "AdjacentStrings" | "DoubleLiteral" | "IntegerLiteral" | "StringInterpolation" => self.visit_parent_if_node_or_before(n),
            "Annotation" => self.v_annotation(n),
            "ArgumentList" => self.v_argument_list(n),
            "AsExpression" => self.v_as(n),
            "AssertInitializer" => self.v_assert_initializer(n),
            "AssertStatement" => self.v_assert(n),
            "AssignmentExpression" => {
                self.local("AssignmentExpression_rightHandSide");
                self.to_expression(n, false, true);
            }
            "AwaitExpression" => {
                self.local("AwaitExpression_expression");
                self.to_expression(n, false, true);
            }
            "BinaryExpression" => self.v_binary(n),
            "Block" => self.v_block(n),
            "BooleanLiteral" | "NullLiteral" | "ConstructorReference" | "ExtensionOverride" | "FunctionExpressionInvocation" | "FunctionReference"
            | "SymbolLiteral" | "ThisExpression" | "TypeLiteral" => self.to_expression(n, false, false),
            "BreakStatement" | "ContinueStatement" => self.v_break_continue(n),
            "CascadeExpression" => {
                self.local("CascadeExpression_cascadeSection");
                self.to_expression(n, false, false);
            }
            "CaseClause" => {
                self.local("CaseClause_pattern");
                self.to_pattern(n, true);
            }
            "CastPattern" => self.v_cast_pattern(n),
            "CatchClause" => self.v_catch(n),
            "ClassDeclaration" => self.v_class(n),
            "CommentReference" => {
                self.local("CommentReference_identifier");
                self.dh(Flags { prefer_without_invocation: true, ..Flags::default() });
                self.lexical_dh(n);
            }
            "CompilationUnit" => self.v_unit(n),
            "ConditionalExpression" => self.v_conditional(n),
            "ConstantPattern" => {
                if self.children(n).first().is_some_and(|&f| self.kind(f) == "SimpleIdentifier")
                    && let Some(p) = self.parent(n)
                {
                    self.visit(p);
                }
            }
            "ConstructorDeclaration" => self.v_constructor(n),
            "ConstructorFieldInitializer" => self.v_field_initializer(n),
            "ConstructorName" => self.v_constructor_name(n),
            "ConstructorSelector" => self.v_constructor_selector(n),
            "DeclaredIdentifier" => {
                if let Some(p) = self.parent(n) {
                    self.visit(p);
                }
            }
            "DeclaredVariablePattern" => self.v_declared_variable_pattern(n),
            "DefaultFormalParameter" => self.v_default_parameter(n),
            "DoStatement" => self.v_do(n),
            "EmptyStatement" => self.v_empty_command(n),
            "EnumDeclaration" => self.v_enum(n),
            "ExportDirective" => {
                if let Some(i) = self.word_in("export", self.start(n), self.end(n))
                    && self.offset <= self.span_tok(i).end
                {
                    self.to_unit_member_before(n);
                }
            }
            "ExpressionFunctionBody" => self.v_expression_body(n),
            "ExpressionStatement" => self.v_expression_statement(n),
            "ExtendsClause" => self.v_extends(n),
            "ExtensionDeclaration" => self.v_extension(n),
            "ExtensionOnClause" => self.v_extension_on(n),
            "ExtensionTypeDeclaration" => self.v_extension_type(n),
            "FieldDeclaration" => self.v_field(n),
            "FieldFormalParameter" => self.v_field_parameter(n),
            "ForEachPartsWithDeclaration" | "ForEachPartsWithIdentifier" => {
                self.local("ForEachPartsWithDeclaration_iterable");
                self.visit_for_each_parts(n);
            }
            "ForEachPartsWithPattern" => {
                self.local("visitForEachPartsWithPattern_iterable");
                self.visit_for_each_parts(n);
            }
            "ForElement" => {
                self.local("ForElement_body");
                if let Some(lit) = self.ancestor(n, &["ListLiteral", "SetOrMapLiteral"]) {
                    let elements = self.literal_elements(lit);
                    self.to_collection_element(lit, &elements);
                }
            }
            "FormalParameterList" => self.v_parameter_list(n),
            "ForPartsWithDeclarations" => self.v_for_parts_with_declarations(n),
            "ForPartsWithExpression" => {
                if self.all_synthetic(n)
                    && let Some(p) = self.parent(n)
                {
                    self.visit(p);
                }
            }
            "ForStatement" => self.v_for(n),
            "FunctionDeclaration" => self.v_function_declaration(n),
            "FunctionExpression" => self.v_function_expression(n),
            "FunctionTypeAlias" => self.v_old_typedef(n),
            "FunctionTypedFormalParameter" => self.v_function_parameter(n),
            "GenericTypeAlias" => self.v_typedef(n),
            "HideCombinator" => {
                self.local("HideCombinator_hiddenName");
                self.to_combinator(n);
            }
            "ShowCombinator" => {
                self.local("ShowCombinator_shownName");
                self.to_combinator(n);
            }
            "IfElement" => self.v_if_element(n),
            "IfStatement" => self.v_if(n),
            "ImplementsClause" => self.v_implements(n),
            "ImportDirective" => self.v_import(n),
            "ImportPrefixReference" => self.v_prefix_reference(n),
            "IndexExpression" => self.v_index(n),
            "InstanceCreationExpression" => self.v_creation(n),
            "InterpolationExpression" => {
                self.local("InterpolationExpression_expression");
                let is_static = self.static_context(n);
                self.dh(Flags { is_static, non_void: true, ..Flags::default() });
                self.lexical_dh(n);
            }
            "IsExpression" => self.v_is(n),
            "Label" => self.v_label(n),
            "LibraryDirective" => self.v_library(n),
            "ListLiteral" | "SetOrMapLiteral" => self.v_collection_literal(n),
            "ListPattern" => {
                self.local("ListPattern_element");
                self.to_pattern(n, true);
            }
            "LogicalAndPattern" => {
                self.local("LogicalAndPattern_rightOperand");
                self.to_pattern(n, true);
            }
            "LogicalOrPattern" => {
                self.local("LogicalOrPattern_rightOperand");
                self.to_pattern(n, true);
            }
            "MapLiteralEntry" => self.v_map_entry(n),
            "MapPattern" => {
                self.local("MapPatternEntry_key");
                self.to_expression_constant(n);
            }
            "MapPatternEntry" => self.v_map_pattern_entry(n),
            "MethodDeclaration" => self.v_method(n),
            "MethodInvocation" => self.v_invocation(n),
            "MixinDeclaration" => self.v_mixin(n),
            "MixinOnClause" => self.v_mixin_on(n),
            "NamedExpression" => self.v_named_expression(n),
            "NamedType" => self.v_named_type(n),
            "ObjectPattern" => self.v_object_pattern(n),
            "ParenthesizedExpression" => self.v_parens(n),
            "ParenthesizedPattern" => {
                self.local("ParenthesizedPattern_expression");
                self.to_pattern(n, true);
            }
            "PartDirective" | "PartOfDirective" => {
                if let Some(i) = self.word_in("part", self.start(n), self.end(n))
                    && self.offset <= self.span_tok(i).end
                {
                    self.local("CompilationUnit_directive");
                    self.to_unit_member_before(n);
                }
            }
            "PatternAssignment" => {
                self.local("PatternAssignment_expression");
                self.to_expression(n, false, false);
            }
            "PatternField" => self.v_pattern_field(n),
            "PatternFieldName" => self.v_pattern_field_name(n),
            "PatternVariableDeclaration" => {
                self.local("PatternVariableDeclaration_expression");
                self.to_expression(n, false, false);
            }
            "PostfixExpression" => self.v_postfix(n),
            "PrefixedIdentifier" => self.v_prefixed_identifier(n),
            "PrefixExpression" => self.v_prefix(n),
            "PropertyAccess" => self.v_property_access(n),
            "RecordLiteral" => {
                self.local("RecordLiteral_fields");
                self.suggest_record_named_fields(n, Some(n), true);
                self.to_expression(n, false, false);
            }
            "RecordPattern" => self.v_record_pattern(n),
            "RecordTypeAnnotation" => self.v_record_type(n),
            "RecordTypeAnnotationNamedField" => self.v_record_type_named_field(n),
            "RecordTypeAnnotationNamedFields" => {
                self.local("RecordTypeAnnotationNamedFields_fields");
                self.to_type(n, TypeOptions::default());
            }
            "RecordTypeAnnotationPositionalField" => {
                if let Some(t) = self.children(n).first().copied()
                    && self.covers_node(t)
                {
                    self.local("RecordTypeAnnotation_positionalFields");
                    self.to_type(n, TypeOptions::default());
                }
            }
            "RedirectingConstructorInvocation" => self.v_redirecting_invocation(n),
            "RelationalPattern" => self.v_relational_pattern(n),
            "RepresentationDeclaration" => self.v_representation(n),
            "RestPatternElement" => {
                self.local("RestPatternElement_pattern");
                self.to_pattern(n, true);
            }
            "ReturnStatement" => self.v_return(n),
            "SimpleFormalParameter" => self.v_simple_parameter(n),
            "SimpleStringLiteral" => self.v_simple_string(n),
            "SpreadElement" => {
                self.local("SpreadElement_expression");
                self.to_expression(n, false, false);
            }
            "SuperConstructorInvocation" => self.v_super_invocation(n),
            "SuperFormalParameter" => self.dh_super_parameters(n),
            "SwitchCase" => {
                self.local("SwitchMember_statement");
                self.to_command(n);
            }
            "SwitchDefault" => self.v_default(n),
            "SwitchExpression" => self.v_switch_expression(n),
            "SwitchExpressionCase" => self.v_switch_expression_case(n),
            "SwitchPatternCase" => self.v_pattern_case(n),
            "SwitchStatement" => self.v_switch(n),
            "ThrowExpression" => {
                self.local("ThrowExpression_expression");
                self.to_expression(n, false, false);
            }
            "TopLevelVariableDeclaration" => self.v_top_variable(n),
            "TryStatement" => self.v_try(n),
            "TypeArgumentList" => self.to_type(n, TypeOptions::default()),
            "TypeParameter" => self.v_type_parameter(n),
            "VariableDeclaration" => self.v_variable_declaration(n),
            "VariableDeclarationList" => self.v_variable_list(n),
            "VariableDeclarationStatement" => self.v_variables_statement(n),
            "WhenClause" => {
                if let Some(i) = self.word_in("when", self.start(n), self.end(n))
                    && self.offset > self.span_tok(i).end
                {
                    self.local("WhenClause_expression");
                    self.to_expression(n, false, false);
                }
            }
            "WhileStatement" => self.v_while(n),
            "WildcardPattern" => {
                if let Some(t) = self.child(n, "NamedType")
                    && self.covers_node(t)
                {
                    self.dh(Flags { ty: true, ..Flags::default() });
                    self.lexical_dh(n);
                }
            }
            "WithClause" => self.v_with(n),
            "YieldStatement" => self.v_yield(n),
            _ => {}
        }
    }

    // -- Auxiliares do Dart ------------------------------------------------------

    /// `_visitParentIfAtOrBeforeNode`.
    pub(super) fn visit_parent_if_node_or_before(&mut self, n: usize) {
        if self.offset <= self.start(n)
            && let Some(p) = self.parent(n)
        {
            self.visit(p);
        }
    }

    /// `_forExpression`.
    pub(super) fn to_expression(&mut self, n: usize, assignable: bool, non_void: bool) {
        let is_expr = super::isp::EXPRESSION_KINDS.contains(&self.kind(n));
        let constant = is_expr && (self.constant_context(n) || self.parent(n).is_some_and(|p| self.kind(p) == "DefaultFormalParameter"));
        let is_static = self.static_context(n);
        self.kw_expression(Some(n), constant, is_static);
        self.dh(Flags { assignable, constant, non_void, is_static, ..Flags::default() });
        self.lexical_dh(n);
    }

    /// `_forStatement`.
    pub(super) fn to_command(&mut self, n: usize) {
        self.to_expression(n, false, false);
        self.kw_command(n);
    }

    /// `_forTypeAnnotation`.
    pub(super) fn to_type(&mut self, n: usize, o: TypeOptions) {
        if !(o.extensible || o.implementable || o.mixable) {
            self.kw("dynamic");
            if !o.non_void {
                self.kw("void");
            }
        }
        if self.kind(n) == "NamedType" && let Some(prefix) = self.child(n, "ImportPrefixReference") {
            if self.import_prefix(prefix).is_some() {
                self.dh(Flags {
                    extensible: o.extensible,
                    implementable: o.implementable,
                    mixable: o.mixable,
                    non_void: o.non_void,
                    excluded: o.excluded.clone(),
                    exclude_type_names: o.exclude_type_names,
                    ..Flags::default()
                });
                if let Some(p) = self.import_prefix(prefix) {
                    self.dh_by_prefix(p);
                }
            }
            return;
        }
        self.dh(Flags {
            extensible: o.extensible,
            implementable: o.implementable,
            mixable: o.mixable,
            ty: true,
            non_void: o.non_void,
            excluded: o.excluded,
            ..Flags::default()
        });
        self.lexical_dh(n);
    }

    /// O prefixo de import (o nome) de um `ImportPrefixReference` ou de um
    /// identificador, se o elemento dele é um `PrefixElement`.
    pub(super) fn import_prefix(&self, n: usize) -> Option<SymbolId> {
        let text = &self.source[self.start(n)..self.end(n)];
        let name = text.trim_end_matches('.').trim();
        let s = self.query.names.lookup(name)?;
        self.query.program.library(self.library()).prefixes.contains_key(&s).then_some(s)
    }

    /// `_forPattern`.
    pub(super) fn to_pattern(&mut self, n: usize, constant: bool) {
        let covered = self.covered_node();
        if self.kind(n) == "CaseClause"
            && let Some(gp) = self.child(n, "GuardedPattern")
            && let Some(&p) = self.children(gp).first()
        {
            match self.kind(p) {
                "ConstantPattern" => {
                    if let Some(&id) = self.children(p).first()
                        && self.kind(id) == "SimpleIdentifier"
                        && self.offset < self.start(id)
                    {
                        self.dh(Flags { ty: true, ..Flags::default() });
                        self.lexical_dh(n);
                        return;
                    }
                }
                "WildcardPattern" => {
                    // O nome `_` é o último token do padrão.
                    let name = self.end(p).saturating_sub(1);
                    if self.offset < name {
                        self.dh(Flags { ty: true, ..Flags::default() });
                        self.lexical_dh(n);
                        return;
                    }
                }
                _ => {}
            }
        }
        if let Some(c) = covered
            && self.kind(c) == "SimpleIdentifier"
            && self.parent(c).is_some_and(|p| self.kind(p) == "ConstantPattern")
        {
            self.kw_pattern();
            self.prefers_constants = true;
            self.dh(Flags { non_void: true, ..Flags::default() });
            self.lexical_dh(n);
            return;
        }
        if let Some(c) = covered
            && self.kind(c) == "NamedType"
            && let Some(p) = self.parent(c)
        {
            let local = match self.kind(p) {
                "DeclaredVariablePattern" => Some("DeclaredVariablePattern_type"),
                "ObjectPattern" => Some("ObjectPattern_type"),
                "WildcardPattern" => Some("WildcardPattern_type"),
                _ => None,
            };
            if let Some(l) = local {
                self.local(l);
                self.visit(c);
                return;
            }
        }
        self.kw_pattern();
        self.prefers_constants = true;
        let is_static = self.static_context(n);
        self.dh(Flags { constant, is_static, object_pattern: true, ..Flags::default() });
        self.lexical_dh(n);
    }

    /// `_forAnnotation`.
    pub(super) fn to_annotation(&mut self, n: usize) {
        self.dh(Flags { constant: true, ..Flags::default() });
        self.lexical_dh(n);
    }

    /// `_forClassLikeMember`.
    pub(super) fn to_container_member(&mut self, c: usize) {
        match self.kind(c) {
            "ClassDeclaration" => self.to_class_member(c),
            "EnumDeclaration" => self.to_enum_member(c),
            "ExtensionDeclaration" => self.to_extension_member(c),
            "ExtensionTypeDeclaration" => self.to_extension_type_member(c),
            "MixinDeclaration" => self.to_mixin_member(c),
            _ => {}
        }
    }

    /// `_forClassMember`.
    pub(super) fn to_class_member(&mut self, c: usize) {
        self.class_member_kw();
        self.dh(Flags { ty: true, ..Flags::default() });
        self.lexical_dh(c);
        let class = self.node_class(c);
        self.suggest_overrides(class, false);
    }

    /// `_forEnumMember`.
    pub(super) fn to_enum_member(&mut self, c: usize) {
        self.enum_member_kw();
        self.dh(Flags { ty: true, ..Flags::default() });
        self.lexical_dh(c);
    }

    /// `_forExtensionMember`.
    pub(super) fn to_extension_member(&mut self, c: usize) {
        self.extension_member_kw(false);
        self.dh(Flags { ty: true, ..Flags::default() });
        self.lexical_dh(c);
    }

    /// `_forExtensionTypeMember`.
    pub(super) fn to_extension_type_member(&mut self, c: usize) {
        self.extension_type_member_kw(false);
        self.dh(Flags { ty: true, ..Flags::default() });
        self.lexical_dh(c);
    }

    /// `_forMixinMember`.
    pub(super) fn to_mixin_member(&mut self, c: usize) {
        self.mixin_member_kw();
        self.dh(Flags { ty: true, ..Flags::default() });
        self.lexical_dh(c);
        let class = self.node_class(c);
        self.suggest_overrides(class, false);
    }

    /// Os elementos de um literal de coleção (sem o `TypeArgumentList`).
    pub(super) fn literal_elements(&self, lit: usize) -> Vec<usize> {
        self.children(lit).iter().copied().filter(|&f| self.kind(f) != "TypeArgumentList").collect()
    }

    /// `_forCollectionElement`.
    pub(super) fn to_collection_element(&mut self, literal: usize, elements: &[usize]) {
        let is_static = self.static_context(literal);
        let constant = self.constant_context(literal) || self.starts_with_const(literal);
        self.collection_element_kw(literal, elements, constant, is_static);
        let previous = self.element_before(elements);
        self.dh(Flags { is_static, constant, ..Flags::default() });
        self.lexical_dh(previous.unwrap_or(literal));
    }

    /// `_forCombinator`: os nomes exportados da biblioteca da diretiva,
    /// menos os já listados (fora o que está sob o cursor).
    pub(super) fn to_combinator(&mut self, n: usize) {
        let Some(directive) = self.parent(n) else { return };
        if !matches!(self.kind(directive), "ImportDirective" | "ExportDirective") {
            return;
        }
        let Some(lib) = self.directive_library(directive) else { return };
        let covered = self.covered_node();
        let excluded: HashSet<String> = self
            .children(n)
            .iter()
            .copied()
            .filter(|&k| Some(k) != covered)
            .map(|k| self.source[self.start(k)..self.end(k)].to_string())
            .collect();
        self.dh(Flags { prefer_without_invocation: true, ..Flags::default() });
        self.library_dh(lib, &excluded);
    }

    /// A biblioteca referida por um `import`/`export` (`referencedLibrary`).
    pub(super) fn directive_library(&self, directive: usize) -> Option<LibraryId> {
        let d = self.real_directive(directive)?;
        let target = self.map(d.span.start);
        let p = &self.query.program;
        let u = p.unit(self.unit);
        let lib = p.library(u.library);
        let index = u.unit.directives.iter().position(|x| x.span.start == target)?;
        lib.imports
            .iter()
            .filter(|i| i.unit == self.unit && i.directive == index)
            .map(|i| i.library)
            .chain(lib.exports.iter().filter(|e| e.unit == self.unit && e.directive == index).map(|e| e.library))
            .next()
    }

    /// `_forCompilationUnitDeclaration`.
    pub(super) fn to_unit_declaration(&mut self) {
        self.unit_declaration_kw();
        self.dh(Flags { ty: true, ..Flags::default() });
        self.lexical_dh(0);
    }

    /// `_forCompilationUnitMember(unit, (before, after))`.
    pub(super) fn to_unit_member(&mut self, before: Option<usize>, after: Option<usize>) {
        if before.is_none_or(|a| self.is_directive(a)) {
            self.local("CompilationUnit_directive");
            self.kw_directive(before);
        }
        if after.is_none_or(|d| !self.is_directive(d)) {
            self.local_if_empty("CompilationUnit_declaration");
            self.to_unit_declaration();
        }
    }

    /// `_forCompilationUnitMemberBefore`.
    pub(super) fn to_unit_member_before(&mut self, m: usize) {
        if self.parent(m) == Some(0) {
            let (a, d) = self.member_neighbors(m);
            self.to_unit_member(a, d);
        }
    }

    /// `_forConstantExpression`.
    pub(super) fn to_expression_constant(&mut self, n: usize) {
        let in_const = super::isp::EXPRESSION_KINDS.contains(&self.kind(n)) && self.constant_context(n);
        self.constant_expression_kw(in_const);
        let is_static = self.static_context(n);
        self.dh(Flags { constant: true, is_static, ..Flags::default() });
        self.lexical_dh(n);
    }

    /// `_forConstructorInitializer`.
    pub(super) fn to_constructor_initializer(&mut self, ctor: usize, init: Option<usize>) {
        let field = init.and_then(|i| self.initializer_field(ctor, i));
        self.constructor_initializer_kw(ctor, init);
        self.dh(Flags::default());
        self.dh_fields_for_initializers(ctor, field);
    }

    /// O campo de `x = e` (`fieldName.staticElement`).
    pub(super) fn initializer_field(&self, ctor: usize, init: usize) -> Option<dartforge_elements::model::VariableId> {
        let ast::Initializer::Field { name, .. } = self.real_initializer(init)? else { return None };
        let class = self.parent(ctor).and_then(|c| self.node_class(c))?;
        let text = self.real_name(*name);
        let p = &self.query.program;
        p.class(class).fields.iter().copied().find(|&v| self.query.name(p.variable(v).name) == text)
    }

    /// `_forMemberAccess`.
    pub(super) fn to_member_access(&mut self, n: usize, ty: TypeId, so_super: bool) {
        if so_super {
            // `request.target.dotTarget is SuperExpression`: o método que o
            // contém dá o `superMatches` e o `isNoSuchMethod`.
            self.super_method = self.ancestor(n, &["MethodDeclaration"]).and_then(|m| self.real_function(m)).and_then(|f| f.name).map(|x| self.real_name(x).to_string());
        }
        let assignable = self.must_be_assignable(n);
        let constant = self.constant_context(n);
        let non_void = self.parent(n).is_some_and(|p| self.kind(p) == "ArgumentList");
        self.dh(Flags { assignable, constant, non_void, ..Flags::default() });
        self.dh_instance_members(ty, so_super);
    }

    /// `_computeMustBeAssignable`: o nó é o lado esquerdo de uma atribuição e
    /// o cursor está na linha do operador.
    pub(super) fn must_be_assignable(&self, n: usize) -> bool {
        let Some(p) = self.parent(n) else { return false };
        if self.kind(p) != "AssignmentExpression" || self.children(p).first() != Some(&n) {
            return false;
        }
        let Some(op) = self.tok_after(self.end(n)) else { return false };
        let line = |o: usize| self.source[..o.min(self.source.len())].matches('\n').count();
        line(self.offset) == line(self.span_tok(op).start)
    }

    /// `_forNameInDeclaredVariablePattern`.
    pub(super) fn to_name_in_variable_pattern(&mut self, n: usize) {
        let Some(p) = self.parent(n) else { return };
        match self.kind(p) {
            "GuardedPattern" => {
                if self.variable_pattern_name(n).is_some() {
                    self.kw("when");
                    if let Some(t) = self.child(n, "NamedType") {
                        let name = self.named_type_name(t);
                        self.ih(false);
                        self.type_name_ih(&name);
                    }
                }
            }
            "PatternField" => {
                if let Some(external) = self.parent(p)
                    && super::isp::is_pattern(self.kind(external))
                {
                    self.local("PatternField_pattern");
                    self.to_field_name_in_pattern(external);
                }
            }
            _ => {}
        }
    }

    /// O nome declarado de um `DeclaredVariablePattern` (o último token).
    pub(super) fn variable_pattern_name(&self, n: usize) -> Option<usize> {
        let i = self.tok_before(self.end(n))?;
        (self.span_tok(i).start >= self.start(n) && self.word(i)).then_some(i)
    }

    /// O nome (sem prefixo e sem argumentos) de um `NamedType`.
    pub(super) fn named_type_name(&self, t: usize) -> String {
        let mut start = self.start(t);
        if let Some(p) = self.child(t, "ImportPrefixReference") {
            start = self.end(p);
        }
        let i = self.tok_after(start).unwrap_or(0);
        self.text_tok(i).to_string()
    }

    /// `_forPatternFieldNameInPattern`.
    pub(super) fn to_field_name_in_pattern(&mut self, pattern: usize) {
        let excluded = self.field_names(pattern);
        match self.kind(pattern) {
            "ObjectPattern" => {
                let ty = self.child(pattern, "NamedType").and_then(|t| self.type_node_type(t));
                if let Some(t) = ty {
                    self.dh(Flags { non_void: true, ..Flags::default() });
                    self.dh_getters(t, &excluded);
                }
            }
            "RecordPattern" => {
                if let Some(t) = self.matched_type(pattern) {
                    self.dh(Flags { non_void: true, ..Flags::default() });
                    self.dh_getters(t, &excluded);
                }
            }
            _ => {}
        }
    }

    /// `fieldNames` de um padrão objeto ou record: os nomes efetivos.
    pub(super) fn field_names(&self, pattern: usize) -> HashSet<String> {
        let mut names = HashSet::new();
        for c in self.children_of(pattern, &["PatternField"]) {
            if let Some(name) = self.field_effective_name(c) {
                names.insert(name);
            }
        }
        names
    }

    /// `effectiveName`: o nome escrito, ou o da variável de `:var x`.
    pub(super) fn field_effective_name(&self, c: usize) -> Option<String> {
        let name = self.child(c, "PatternFieldName")?;
        let i = self.tok_after(self.start(name))?;
        if self.word(i) && self.span_tok(i).end <= self.end(name) {
            return Some(self.text_tok(i).to_string());
        }
        // `:var x`: o nome da variável do subpadrão.
        let sub = self.children(c).iter().copied().find(|&k| k != name)?;
        if self.kind(sub) == "DeclaredVariablePattern" {
            return self.variable_pattern_name(sub).map(|j| self.text_tok(j).to_string());
        }
        None
    }

    /// O tipo resolvido de um nó de tipo (`type.typeOrThrow`): o dos corpos
    /// (`annotation_types`) ou o do outline (`written_types`).
    pub(super) fn type_node_type(&self, t: usize) -> Option<TypeId> {
        let Ligacao::Tipo(tid) = self.link(t) else {
            // O `NamedType` que a árvore monta para a criação sem `new`
            // (`A()`, `A.n()`): o tipo criado.
            let name = self.parent(t).filter(|&p| self.kind(p) == "ConstructorName")?;
            let creation = self.parent(name).filter(|&p| self.kind(p) == "InstanceCreationExpression")?;
            let Ligacao::Expr(e) = self.link(creation) else { return None };
            return self.static_type(e);
        };
        let s = self.a.ty(tid).span;
        let (start, end) = (self.map(s.start), self.map_end(s.end));
        let ast_c = &self.query.program.unit(self.unit).ast;
        let id = ast_c.types.iter().position(|x| x.span.start == start && x.span.end == end).map(|i| ast::TypeId(i as u32))?;
        let bodies = &self.query.bodies.units[self.unit.0 as usize];
        bodies.tipos_de_anotacoes.get(&id).copied().or_else(|| self.query.outline.tipos_escritos.get(&(self.unit, id)).copied())
    }

    /// O `matchedValueType` de um padrão.
    pub(super) fn matched_type(&self, pattern: usize) -> Option<TypeId> {
        let Ligacao::Padrao(pid) = self.link(pattern) else { return None };
        let s = self.a.pattern(pid).span;
        let (start, end) = (self.map(s.start), self.map_end(s.end));
        let ast_c = &self.query.program.unit(self.unit).ast;
        let bodies = &self.query.bodies.units[self.unit.0 as usize];
        ast_c.patterns.iter().enumerate().find(|(_, x)| x.span.start == start && x.span.end == end).and_then(|(i, _)| bodies.tipos_casados.get(&ast::PatternId(i as u32)).copied())
    }

    /// `_forRedirectingConstructorInvocation`.
    pub(super) fn to_redirecting_invocation(&mut self, ctor: usize) {
        let Some(container) = self.parent(ctor) else { return };
        if !matches!(self.kind(container), "ClassDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration") {
            return;
        }
        let Some(class) = self.node_class(container) else { return };
        let Some(this) = self.class_this_type(class) else { return };
        let name = self.real_constructor(ctor).and_then(|k| k.name).map(|n| self.real_name(n).to_string());
        let constant = self.real_constructor(ctor).is_some_and(|k| k.const_);
        self.dh(Flags { constant, ..Flags::default() });
        self.dh_type_constructor_names(this, name);
    }

    /// `thisType` de uma classe.
    pub(super) fn class_this_type(&mut self, c: ClassId) -> Option<TypeId> {
        class_type(self.query, c)
    }

    /// `_forVariablePattern`.
    pub(super) fn to_variable_pattern(&mut self) {
        self.variable_pattern_kw();
    }

    /// `_forIncompletePrecedingClassMember`.
    pub(super) fn previous_incomplete_member(&mut self, m: usize) -> bool {
        let Some(i) = self.tok_after(self.start(m)) else { return false };
        if self.offset > self.span_tok(i).end {
            return false;
        }
        let Some(previous) = self.previous_member(m) else { return false };
        if self.kind(previous) == "MethodDeclaration" {
            let body = self.children(previous).iter().copied().find(|&k| matches!(self.kind(k), "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"));
            // O corpo todo sintético: o método sem corpo no texto.
            if body.is_none() {
                self.local("ClassDeclaration_member");
                self.body_modifiers_kw(None);
                return true;
            }
        }
        false
    }

    /// `_forIncompletePrecedingStatement`.
    pub(super) fn previous_incomplete_command(&mut self, s: usize) -> bool {
        let Some(i) = self.tok_after(self.start(s)) else { return false };
        if self.offset > self.span_tok(i).end {
            return false;
        }
        let Some(previous) = self.previous_command(s) else { return false };
        match self.kind(previous) {
            "IfStatement" => {
                if self.word_in("else", self.start(previous), self.end(previous)).is_none() {
                    self.kw("else");
                    return false;
                }
            }
            "TryStatement" => {
                if self.word_in("finally", self.start(previous), self.end(previous)).is_none() {
                    self.v_try(previous);
                    return self.children_of(previous, &["CatchClause"]).is_empty();
                }
            }
            _ => {}
        }
        false
    }

    /// `_handledIncompletePrecedingUnitMember`.
    pub(super) fn incomplete_unit_member(&mut self, previous: usize) -> bool {
        match self.kind(previous) {
            "ClassDeclaration" => {
                if self.without_body(previous) {
                    self.local("CompilationUnit_declaration");
                    self.class_declaration_kw(previous);
                    return true;
                }
            }
            "ExtensionTypeDeclaration" => {
                if self.without_body(previous) {
                    self.local("CompilationUnit_declaration");
                    self.v_extension_type(previous);
                    return true;
                }
            }
            "FunctionDeclaration" => {
                self.local("CompilationUnit_declaration");
                let body = self.function_body(previous);
                if body.is_none_or(|c| self.kind(c) == "EmptyFunctionBody") {
                    self.body_modifiers_kw(body);
                }
            }
            "ImportDirective" => {
                let has_semicolon = self.tok_before(self.end(previous)).is_some_and(|i| self.is_op(i, Op::Semicolon));
                if !has_semicolon {
                    self.v_import(previous);
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// O corpo de uma função/método (o nó do corpo).
    pub(super) fn function_body(&self, n: usize) -> Option<usize> {
        let expr = if self.kind(n) == "FunctionDeclaration" { self.child(n, "FunctionExpression")? } else { n };
        self.children(expr).iter().copied().find(|&k| matches!(self.kind(k), "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"))
    }

    /// A declaração não tem corpo (`{` e `}` sintéticos).
    pub(super) fn without_body(&self, n: usize) -> bool {
        let (_, _, left, dir) = self.keys(n);
        left && dir
    }

    /// As chaves do corpo de uma declaração de classe, mixin, enum,
    /// extensão ou tipo de extensão: `({, }, { sintético, } sintético)`.
    pub(super) fn keys(&self, n: usize) -> (Span, Span, bool, bool) {
        if let Some(i) = self.tok_before(self.end(n))
            && self.is_op(i, Op::RBrace)
            && self.span_tok(i).start >= self.start(n)
            && let Some(j) = self.pair(i)
            && self.span_tok(j).start >= self.start(n)
        {
            return (self.span_tok(j), self.span_tok(i), false, false);
        }
        if let Some(j) = self.op_in(Op::LBrace, self.start(n), self.end(n)) {
            let s = self.synthetic(self.end(n));
            return (self.span_tok(j), s, false, true);
        }
        let s = self.synthetic(self.end(n));
        (s, s, true, true)
    }

    /// `_handledPossibleClosure`.
    pub(super) fn handled_possible_closure(&mut self, e: usize) -> bool {
        let Some(next) = self.tok_after(self.end(e)) else { return false };
        if self.offset > self.span_tok(next).start {
            return false;
        }
        match self.kind(e) {
            "ParenthesizedExpression" => {
                if self.children(e).first().is_some_and(|&f| self.kind(f) == "SimpleIdentifier") {
                    self.body_modifiers_kw(None);
                    return true;
                }
            }
            "RecordLiteral" => {
                if self.children(e).iter().all(|&f| self.kind(f) == "SimpleIdentifier") {
                    self.body_modifiers_kw(None);
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// `_handledRecovery` de uma `TopLevelVariableDeclaration`.
    pub(super) fn handled_recovery(&mut self, decl: usize) -> bool {
        if self.parent(decl) != Some(0) {
            return false;
        }
        let Some(first) = self.tok_after(self.start(decl)) else { return false };
        let first_node = self.offset <= self.span_tok(first).end;
        if first_node {
            let (before, _) = self.member_neighbors(decl);
            if let Some(a) = before
                && self.incomplete_unit_member(a)
            {
                return true;
            }
        }
        if self.loose_top_declaration(decl) && first_node {
            let (a, d) = self.member_neighbors(decl);
            self.to_unit_member(a, d);
            return true;
        }
        false
    }

    /// `TopLevelVariableDeclaration.isSingleIdentifier`: uma palavra seguida
    /// do `;` que o fasta criaria (aqui: sem `;` no texto).
    pub(super) fn loose_top_declaration(&self, decl: usize) -> bool {
        let Some(i) = self.tok_after(self.start(decl)) else { return false };
        if !self.word(i) {
            return false;
        }
        let last = self.tok_before(self.end(decl));
        last == Some(i)
    }

    /// `_locationFor(argumentList, isNamed)`.
    pub(super) fn argument_location(&self, list: usize, named: bool) -> String {
        let parent = self.parent(list).map_or("", |p| self.kind(p));
        format!("ArgumentList_{}_{}", argument_role(parent), if named { "named" } else { "unnamed" })
    }

    /// `_tryAnnotationAtEndOfClassBody`: o token deslocado é o `}` e os dois
    /// anteriores são `@` e um identificador.
    pub(super) fn annotation_at_body_end(&mut self, decl: usize) -> bool {
        let Some(key) = self.tok_after(self.offset) else { return false };
        if !self.is_op(key, Op::RBrace) {
            return false;
        }
        let Some(id) = key.checked_sub(1) else { return false };
        if !self.is_identifier(id) || id == 0 || !self.is_op(id - 1, Op::At) {
            return false;
        }
        self.local("Annotation_name");
        self.to_annotation(decl);
        self.try_annotation_override(id, decl);
        true
    }

    /// `_tryOverrideAnnotation`.
    pub(super) fn try_annotation_override(&mut self, id: usize, decl: usize) {
        let lexeme = self.text_tok(id);
        if !lexeme.is_empty() && "override".starts_with(lexeme) {
            let class = self.node_class(decl);
            if class.is_some() {
                self.suggest_overrides(class, true);
            }
        }
    }

    /// `_suggestOverridesFor`.
    pub(super) fn suggest_overrides(&mut self, class: Option<ClassId>, without_at_sign: bool) {
        if let Some(c) = class {
            self.overrides(c, without_at_sign);
        }
    }

    /// `_visitForEachParts`.
    pub(super) fn visit_for_each_parts(&mut self, n: usize) {
        let Some(em) = self.word_in("in", self.start(n), self.end(n)) else { return };
        let s = self.span_tok(em);
        if self.covers(s.start, s.end) {
            let previous = em.checked_sub(1);
            if previous.is_some_and(|i| self.is_op(i, Op::Assign)) {
                self.kw("const");
                self.kw("false");
                self.kw("null");
                self.kw("true");
            } else {
                self.kw("in");
            }
        } else {
            self.kw("await");
            let is_static = self.static_context(n);
            self.dh(Flags { is_static, ..Flags::default() });
            self.lexical_dh(n);
        }
    }

    // -- Os parâmetros da invocação ------------------------------------------------

    /// `invokedFormalParameters` de uma `ArgumentList`.
    pub(super) fn invoked_parameters(&mut self, list: usize) -> Option<Vec<InvokedParameter>> {
        let parent = self.parent(list)?;
        let query = &*self.query;
        let by_element = |f: FunctionElementId| -> Vec<InvokedParameter> {
            query.outline.functions[f.0 as usize]
                .parameters
                .iter()
                .filter_map(|p| {
                    let name = query.name(p.externo.or(p.name)?).to_string();
                    Some(InvokedParameter {
                        name,
                        named: p.kind == ast::ParameterKind::Named,
                        required: p.required,
                        ty: p.ty,
                        widget: false,
                        annotated_required: false,
                        type_names: Vec::new(),
                    })
                })
                .collect()
        };
        let by_type = |t: TypeId| -> Option<Vec<InvokedParameter>> {
            let Type::Function { positional, optional, named, .. } = query.table.get(t).clone() else { return None };
            let mut v: Vec<InvokedParameter> = positional
                .iter()
                .chain(optional.iter())
                .map(|&t| InvokedParameter { name: String::new(), named: false, required: false, ty: t, widget: false, annotated_required: false, type_names: Vec::new() })
                .collect();
            for (n, t, r) in named.iter() {
                v.push(InvokedParameter { name: query.name(*n).to_string(), named: true, required: *r, ty: *t, widget: false, annotated_required: false, type_names: Vec::new() });
            }
            Some(v)
        };
        match self.kind(parent) {
            "MethodInvocation" | "FunctionExpressionInvocation" | "InstanceCreationExpression" => {
                let e = self.real_expr(parent)?;
                let c = self.query_expr(e)?;
                let bodies = &self.query.bodies.units[self.unit.0 as usize];
                if let Some(Resolved::Constructor(f)) = bodies.get_resolved(c) {
                    return Some(self.enriched(by_element(*f), *f));
                }
                let target_ = match &self.query.program.unit(self.unit).ast.expr(c).kind {
                    ExprKind::Call { target, .. } => Some(*target),
                    _ => None,
                };
                if let Some(a) = target_ {
                    if let Some(Resolved::Constructor(f)) = bodies.get_resolved(a) {
                        return Some(self.enriched(by_element(*f), *f));
                    }
                    let function = match bodies.get_resolved(a) {
                        Some(Resolved::Element(Element::Function(f))) => Some(*f),
                        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => Some(*f),
                        Some(Resolved::ExtensionMember { member, .. }) => Some(*member),
                        _ => None,
                    };
                    if let Some(t) = bodies.get_type(a)
                        && let Some(v) = by_type(t)
                    {
                        return Some(match function {
                            Some(f) => self.element_names(v, f),
                            None => v,
                        });
                    }
                }
                None
            }
            "Annotation" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "EnumConstantArguments" => {
                let f = self.invoked_constructor(parent)?;
                Some(self.enriched(by_element(f), f))
            }
            _ => None,
        }
    }

    /// Os parâmetros declarados de um executável no AST (o da unidade dele).
    pub(super) fn parameters_ast(&self, f: FunctionElementId) -> Option<(UnitId, &[ast::Parameter])> {
        let p = &self.query.program;
        match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => Some((unit, p.unit(unit).ast.function(function).parameters.as_deref()?)),
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => match &p.unit(unit).ast.member(member).kind {
                ast::MemberKind::Constructor(k) => Some((unit, &k.parameters[..])),
                _ => None,
            },
            dartforge_elements::model::FunctionRef::None => None,
        }
    }

    /// Os nomes dos posicionais de um tipo de função escrito (`void
    /// Function(int a)`) or da shape antiga (`void f(int a)`) de um
    /// parâmetro, na unidade `u` da consulta.
    pub(super) fn written_type_names(&self, u: UnitId, prm: &ast::Parameter) -> Vec<String> {
        let unit = self.query.program.unit(u);
        let names = |ps: &[ast::Parameter]| -> Vec<String> {
            ps.iter().filter(|x| x.kind != ast::ParameterKind::Named).map(|x| x.name.map(|n| self.query.name(n.sym).to_string()).unwrap_or_default()).collect()
        };
        if let Some(fps) = &prm.function_parameters {
            return names(fps);
        }
        match prm.ty.map(|t| &unit.ast.ty(t).kind) {
            Some(ast::TypeKind::Function { parameters, .. }) => names(parameters),
            _ => Vec::new(),
        }
    }

    /// Completa os parâmetros pelo elemento: `widget`, `@required` e os
    /// nomes do tipo de função escrito.
    pub(super) fn enriched(&self, mut v: Vec<InvokedParameter>, f: FunctionElementId) -> Vec<InvokedParameter> {
        let p = &self.query.program;
        let widget = matches!(p.function(f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) && p.function(f).class.is_some_and(|c| self.is_widget(c));
        let declared: Vec<(UnitId, &ast::Parameter)> = match self.parameters_ast(f) {
            Some((u, ps)) => ps.iter().filter(|x| x.nome_externo().is_some()).map(|x| (u, x)).collect(),
            None => Vec::new(),
        };
        for (i, x) in v.iter_mut().enumerate() {
            x.widget = widget;
            if let Some(&(u, prm)) = declared.get(i) {
                x.annotated_required = prm.metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| self.query.name(n.sym) == "required"));
                x.type_names = self.written_type_names(u, prm);
            }
        }
        v
    }

    /// Os nomes do tipo de função escrito nos parâmetros de `f` (os
    /// parâmetros vieram do tipo, na ordem: posicionais, opcionais,
    /// nomeados).
    pub(super) fn element_names(&self, mut v: Vec<InvokedParameter>, f: FunctionElementId) -> Vec<InvokedParameter> {
        let Some((u, ps)) = self.parameters_ast(f) else { return v };
        let positional: Vec<&ast::Parameter> = ps.iter().filter(|x| x.kind != ast::ParameterKind::Named).collect();
        let mut k = 0usize;
        for x in v.iter_mut() {
            if x.named {
                if let Some(prm) = ps.iter().find(|q| q.kind == ast::ParameterKind::Named && q.nome_externo().is_some_and(|n| self.query.name(n.sym) == x.name)) {
                    x.type_names = self.written_type_names(u, prm);
                    x.annotated_required = prm.metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| self.query.name(n.sym) == "required"));
                }
            } else {
                if let Some(prm) = positional.get(k) {
                    x.type_names = self.written_type_names(u, prm);
                }
                k += 1;
            }
        }
        v
    }

    /// `ClassElement.isWidget`: a classe é (subtipo de) `Widget` do Flutter.
    pub(super) fn is_widget(&self, c: ClassId) -> bool {
        let p = &self.query.program;
        let mut stack = vec![c];
        let mut seen = HashSet::new();
        while let Some(x) = stack.pop() {
            if !seen.insert(x) {
                continue;
            }
            let cl = p.class(x);
            if self.query.name(cl.name) == "Widget" && p.library(cl.library).uri == "package:flutter/src/widgets/framework.dart" {
                return true;
            }
            stack.extend(cl.supertype_class);
            stack.extend(cl.mixin_classes.iter().copied());
            stack.extend(cl.interface_classes.iter().copied());
        }
        false
    }

    /// O construtor que uma anotação, `super(...)`, `this(...)` ou os
    /// argumentos de uma constante de enum invocam.
    pub(super) fn invoked_constructor(&self, n: usize) -> Option<FunctionElementId> {
        let p = &self.query.program;
        match self.kind(n) {
            "SuperConstructorInvocation" | "RedirectingConstructorInvocation" => {
                let ctor = self.parent(n)?;
                let container = self.parent(ctor)?;
                let class = self.node_class(container)?;
                let (ast::Initializer::Super { constructor, .. } | ast::Initializer::Redirect { constructor, .. }) = self.real_initializer(n)? else {
                    return None;
                };
                let target = if self.kind(n) == "SuperConstructorInvocation" { p.class(class).supertype_class? } else { class };
                let name = constructor.map(|k| self.real_name(k).to_string()).unwrap_or_default();
                p.class(target).construtores().into_iter().find(|(s, _)| self.query.name(*s) == name).map(|(_, f)| f)
            }
            "EnumConstantArguments" => {
                let constant = self.parent(n)?;
                let e = self.parent(constant)?;
                let class = self.node_class(e)?;
                let name = self.child(n, "ConstructorSelector").map(|s| self.source[self.start(s)..self.end(s)].trim_start_matches('.').trim().to_string()).unwrap_or_default();
                p.class(class).construtores().into_iter().find(|(s, _)| self.query.name(*s) == name).map(|(_, f)| f)
            }
            "Annotation" => {
                let text = self.source[self.start(n)..self.end(n)].trim_start_matches('@');
                let name: String = text.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$' || *c == '.').collect();
                let parts: Vec<&str> = name.split('.').collect();
                let lib = p.library(self.library());
                let class = parts.first().and_then(|n0| self.query.names.lookup(n0)).and_then(|s| lib.scope.get(&s)).and_then(|b| match b.getter {
                    Some(Element::Class(c)) => Some(c),
                    _ => None,
                })?;
                let ctor = parts.get(1).copied().unwrap_or("");
                p.class(class).construtores().into_iter().find(|(s, _)| self.query.name(*s) == ctor).map(|(_, f)| f)
            }
            _ => None,
        }
    }
}

/// As opções de `_forTypeAnnotation`.
#[derive(Debug, Clone, Default)]
pub(super) struct TypeOptions {
    pub extensible: bool,
    pub implementable: bool,
    pub mixable: bool,
    pub non_void: bool,
    pub exclude_type_names: bool,
    pub excluded: Vec<usize>,
}

impl<'a, 'c> Isp<'a, 'c> {
    // -- Os `visit*` -------------------------------------------------------------

    pub(super) fn v_annotation(&mut self, n: usize) {
        self.local("Annotation_name");
        self.to_annotation(n);
        // `@over^` seguido de linha em branco: pedido de sobrescrita.
        let Some(m) = self.real_annotation(n) else { return };
        let without_constructor = m.name.len() == 1 && m.arguments.is_none();
        if !without_constructor {
            return;
        }
        let Some(decl) = self.parent(n) else { return };
        let Some(container) = self.parent(decl) else { return };
        if !super::isp::is_declaration(self.kind(container)) {
            return;
        }
        let name = m.name[0];
        let Some(id) = self.tok_at(name.span.start) else { return };
        let Some(next) = self.tok_after(name.span.end) else { return };
        let line = |o: usize| self.source[..o.min(self.source.len())].matches('\n').count();
        if line(self.span_tok(next).start) > line(name.span.start) + 1 {
            self.try_annotation_override(id, container);
        }
    }

    /// O `ast::Annotation` de um nó `Annotation`.
    pub(super) fn real_annotation(&self, n: usize) -> Option<&'a ast::Annotation> {
        let Ligacao::Anotacao(end) = self.link(n) else { return None };
        let all = self
            .a
            .decls
            .iter()
            .flat_map(|d| d.metadata.iter())
            .chain(self.a.members.iter().flat_map(|m| m.metadata.iter()))
            .chain(self.cu.directives.iter().flat_map(|d| d.metadata.iter()))
            .chain(self.a.metadados_locais.iter().flat_map(|(_, m)| m.iter()))
            .chain(self.a.functions.iter().flat_map(|f| f.parameters.iter().flatten()).flat_map(|p| p.metadata.iter()))
            .chain(self.a.members.iter().flat_map(|m| match &m.kind {
                ast::MemberKind::Constructor(k) => k.parameters.iter().flat_map(|p| p.metadata.iter()).collect::<Vec<_>>(),
                _ => Vec::new(),
            }));
        for m in all {
            if crate::arvore_analyzer::endereco(m) == end {
                return Some(m);
            }
        }
        None
    }

    pub(super) fn v_argument_list(&mut self, n: usize) {
        let opens = self.start(n);
        let closes = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::RParen)).map_or(self.end(n), |i| self.span_tok(i).start);
        if self.offset <= opens {
            if let Some(p) = self.parent(n) {
                self.visit(p);
            }
            return;
        }
        if self.offset > closes {
            self.local("ExpressionStatement_expression");
            return;
        }
        let Some(parent) = self.parent(n) else { return };
        let l = self.argument_location(n, false);
        self.local(&l);
        let arguments: Vec<usize> = self.children(n).to_vec();
        // `argumentsBeforeAndAfterOffset`.
        let mut before = None;
        let mut after = None;
        for &a in &arguments {
            if self.offset < self.start(a) || (self.offset == self.start(a) && before.is_some_and(|p| self.end(p) == self.offset)) {
                after = Some(a);
                break;
            }
            before = Some(a);
        }
        let mut index = 0usize;
        if let Some(b) = before {
            if self.handled_possible_closure(b) {
                self.to_expression(b, false, true);
                return;
            }
            index = arguments.iter().position(|&k| k == b).unwrap_or(0);
            if self.offset > self.end(b) {
                index += 1;
            }
        }
        // `argumentContext(index)`.
        let mut positional = 0usize;
        let mut used: HashSet<String> = HashSet::new();
        for (i, &a) in arguments.iter().enumerate() {
            if self.kind(a) == "NamedExpression" {
                if let Some(r) = self.child(a, "Label") {
                    used.insert(self.source[self.start(r)..self.end(r)].trim_end_matches(':').trim().to_string());
                }
            } else if i < index {
                positional += 1;
            }
        }
        match self.invoked_parameters(n) {
            Some(parameters) => {
                let mut positional_count = 0usize;
                let mut available = Vec::new();
                for p in &parameters {
                    if p.named {
                        if !used.contains(&p.name) {
                            available.push(p.clone());
                        }
                    } else {
                        positional_count += 1;
                    }
                }
                if positional < positional_count {
                    self.to_expression(parent, false, true);
                    let p = parameters[positional].clone();
                    if matches!(self.query.table.get(p.ty), Type::Function { .. }) {
                        let argument = arguments.get(index).copied();
                        let comma = argument.is_none_or(|a| !self.followed_by_comma(a));
                        self.suggest_closure_with_names(p.ty, &p.type_names, comma);
                    }
                } else {
                    let l = self.argument_location(n, true);
                    self.local(&l);
                }
                // A vírgula depois do nome sugerido (sem tokens sintéticos: a
                // vírgula está no texto ou não está).
                let mut comma = false;
                if let Some(d) = after {
                    let possible = self.tok_before(self.start(d));
                    match possible {
                        Some(i) if self.is_op(i, Op::Comma) => {
                            if self.offset >= self.span_tok(i).end {
                                comma = true;
                            }
                        }
                        _ => comma = true,
                    }
                } else if self.kind(parent) == "InstanceCreationExpression"
                    && let Some(e) = self.real_expr(parent)
                    && let Some(t) = self.static_type(e)
                    && let Type::Interface { class, .. } = self.query.table.get(t)
                    && self.is_widget(*class)
                {
                    // `parent.isWidgetCreation`.
                    comma = true;
                }
                let mut replace = None;
                if before.is_some_and(|b| self.offset == self.start(b)) {
                    replace = Some(0usize);
                    comma = false;
                }
                for p in available {
                    self.suggest_named_argument(&p, true, comma, replace);
                }
            }
            None => {
                if super::isp::EXPRESSION_KINDS.contains(&self.kind(parent)) {
                    self.to_expression(parent, false, true);
                }
            }
        }
    }

    /// `isFollowedByComma`.
    pub(super) fn followed_by_comma(&self, n: usize) -> bool {
        self.tok_after(self.end(n)).is_some_and(|i| self.is_op(i, Op::Comma))
    }

    pub(super) fn v_as(&mut self, n: usize) {
        let Some(&expr) = self.children(n).first() else { return };
        if self.offset <= self.end(expr) {
            let is_static = self.static_context(n);
            self.dh(Flags { non_void: true, is_static, ..Flags::default() });
            self.lexical_dh(n);
            return;
        }
        if let Some(op) = self.word_in("as", self.end(expr), self.end(n)) {
            let s = self.span_tok(op);
            if self.covers(s.start, s.end) {
                if self.kind(expr) == "ParenthesizedExpression" {
                    self.body_modifiers_kw(None);
                } else {
                    self.kw("as");
                }
                return;
            }
        }
        let ty = self.children(n).get(1).copied();
        let covers_start = ty.is_some_and(|t| self.tok_after(self.start(t)).is_some_and(|i| self.covers(self.span_tok(i).start, self.span_tok(i).end)));
        if ty.is_none() || covers_start {
            self.local("AsExpression_type");
            self.to_type(n, TypeOptions { non_void: true, ..TypeOptions::default() });
        }
    }

    pub(super) fn v_assert_initializer(&mut self, n: usize) {
        let opens = self.op_in(Op::LParen, self.start(n), self.end(n));
        if let Some(a) = opens
            && self.span_tok(a).end <= self.offset
        {
            let comma = self.op_in(Op::Comma, self.span_tok(a).end, self.end(n));
            let condition = self.children(n).first().copied().unwrap_or(n);
            if comma.is_none_or(|v| self.offset <= self.span_tok(v).start) {
                self.local("AssertInitializer_condition");
            } else {
                self.local("AssertInitializer_message");
            }
            self.to_expression(condition, false, false);
            return;
        }
        self.local("ConstructorDeclaration_initializer");
        if let Some(ctor) = self.parent(n) {
            self.constructor_initializer_kw(ctor, Some(n));
        }
    }

    pub(super) fn v_assert(&mut self, n: usize) {
        if let Some(a) = self.op_in(Op::LParen, self.start(n), self.end(n))
            && self.span_tok(a).end <= self.offset
        {
            let comma = self.op_in(Op::Comma, self.span_tok(a).end, self.end(n));
            let condition = self.children(n).first().copied().unwrap_or(n);
            if comma.is_none_or(|v| self.offset <= self.span_tok(v).start) {
                self.local("AssertStatement_condition");
            } else {
                self.local("AssertStatement_message");
            }
            self.to_expression(condition, false, false);
            return;
        }
        if let Some(i) = self.tok_after(self.start(n))
            && self.offset <= self.span_tok(i).end
        {
            self.local("Block_statement");
            self.to_command(n);
        }
    }

    pub(super) fn v_binary(&mut self, n: usize) {
        let (Some(&l), Some(&r)) = (self.children(n).first(), self.children(n).get(1)) else { return };
        let op = self.source[self.end(l)..self.start(r)].trim().to_string();
        self.local(&format!("BinaryExpression_{op}_rightOperand"));
        self.to_expression(n, false, true);
    }

    pub(super) fn v_block(&mut self, n: usize) {
        let left = self.tok_at(self.start(n)).filter(|&i| self.is_op(i, Op::LBrace));
        if left.is_none() {
            if let Some(p) = self.parent(n) {
                self.visit(p);
            }
        }
        if self.offset <= self.start(n) {
            if let Some(p) = self.parent(n)
                && self.kind(p) == "BlockFunctionBody"
                && let Some(pp) = self.parent(p)
            {
                self.visit(pp);
            }
            return;
        }
        self.local("Block_statement");
        let commands: Vec<usize> = self.children(n).to_vec();
        if let Some(previous) = self.element_before(&commands) {
            match self.kind(previous) {
                "TryStatement" => {
                    if self.word_in("finally", self.start(previous), self.end(previous)).is_none() {
                        self.kw("on");
                        self.kw("catch");
                        self.kw("finally");
                        if self.children_of(previous, &["CatchClause"]).is_empty() {
                            return;
                        }
                    }
                }
                "IfStatement" => {
                    if self.word_in("else", self.start(previous), self.end(previous)).is_none() {
                        self.kw("else");
                    }
                }
                _ => {}
            }
        }
        self.to_command(n);
        if self.in_catch(n) {
            self.kw("rethrow");
        }
    }

    pub(super) fn v_break_continue(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        let end_word = self.span_tok(word).end;
        let is_break = self.kind(n) == "BreakStatement";
        if self.offset <= end_word {
            self.local("Block_statement");
            self.kw(if is_break { "break" } else { "continue" });
        } else {
            let pv = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::Semicolon)).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
            if self.offset <= pv {
                self.labels(n);
            }
        }
    }

    pub(super) fn v_cast_pattern(&mut self, n: usize) {
        if let Some(i) = self.word_in("as", self.start(n), self.end(n))
            && self.covers(self.span_tok(i).start, self.span_tok(i).end)
        {
            self.kw("as");
        } else {
            self.local("CastPattern_type");
            self.to_type(n, TypeOptions { non_void: true, ..TypeOptions::default() });
        }
    }

    pub(super) fn v_catch(&mut self, n: usize) {
        let on = self.word_in("on", self.start(n), self.end(n));
        let catch = self.word_in("catch", self.start(n), self.end(n));
        let body = self.child(n, "Block").map_or(self.end(n), |b| self.start(b));
        if let Some(o) = on {
            if self.offset <= self.span_tok(o).end {
                self.kw("on");
            } else if catch.is_none() && self.offset <= body {
                self.local("CatchClause_exceptionType");
                self.to_type(n, TypeOptions::default());
            } else if let Some(c) = catch
                && self.offset < self.span_tok(c).start
            {
                self.local("CatchClause_exceptionType");
                self.to_type(n, TypeOptions::default());
            }
        }
        if let Some(c) = catch
            && self.offset >= self.span_tok(c).start
            && self.offset <= self.span_tok(c).end
        {
            self.kw("catch");
        }
    }

    pub(super) fn v_class(&mut self, n: usize) {
        if self.offset == self.start(n) {
            self.to_unit_member_before(n);
            return;
        }
        let Some(d) = self.real_decl(n) else { return };
        let ast::DeclKind::Class(x) = &d.kind else { return };
        let class_kw = self.word_in("class", self.start(n), x.name.span.start);
        let (left, dir, left_s, dir_s) = self.keys(n);
        if let Some(k) = class_kw {
            if self.offset < self.span_tok(k).start {
                self.class_modifiers_kw(n);
                return;
            }
            if self.offset <= self.span_tok(k).end {
                self.kw("class");
                return;
            }
        }
        if self.offset <= x.name.span.end {
            self.ih(false);
            self.top_name_ih(left_s && dir_s);
        } else if self.offset <= left.start {
            self.class_declaration_kw(n);
        } else if self.offset >= left.end && self.offset <= dir.start {
            if self.annotation_at_body_end(n) {
                return;
            }
            self.local("ClassDeclaration_member");
            let members = self.children_of(n, &["FieldDeclaration", "MethodDeclaration", "ConstructorDeclaration"]);
            let previous = self.element_before(&members);
            let tok = match previous {
                Some(m) => self.tok_after(self.start(m)),
                None => self.tok_after(left.end),
            };
            if tok.is_some_and(|i| self.text_tok(i) == "final") {
                self.to_type(n, TypeOptions::default());
                return;
            }
            self.to_class_member(n);
            if let Some(m) = previous
                && self.kind(m) == "MethodDeclaration"
                && self.function_body(m).is_none_or(|c| self.kind(c) == "EmptyFunctionBody")
            {
                let body = self.function_body(m);
                self.body_modifiers_kw(body);
            }
        } else if let Some(p) = self.parent(n) {
            self.visit(p);
        }
    }

    pub(super) fn v_unit(&mut self, _n: usize) {
        let (before, after) = self.offset_neighbors();
        if let Some(a) = before
            && self.incomplete_unit_member(a)
        {
            return;
        }
        self.to_unit_member(before, after);
    }

    pub(super) fn v_conditional(&mut self, n: usize) {
        let (Some(&c), Some(&t)) = (self.children(n).first(), self.children(n).get(1)) else {
            self.to_expression(n, false, false);
            return;
        };
        let question = self.op_in(Op::Question, self.end(c), self.start(t));
        let colon = self.op_in(Op::Colon, self.end(t), self.end(n));
        if let (Some(q), Some(d)) = (question, colon) {
            if self.offset >= self.span_tok(q).end && self.offset <= self.span_tok(d).start {
                self.local("ConditionalExpression_thenExpression");
            } else if self.offset >= self.span_tok(d).end {
                self.local("ConditionalExpression_elseExpression");
            }
        }
        self.to_expression(n, false, false);
    }

    pub(super) fn v_constructor(&mut self, n: usize) {
        let Some(k) = self.real_constructor(n) else { return };
        if self.offset <= k.class_name.span.end {
            self.local("ClassDeclaration_member");
            if let Some(p) = self.parent(n) {
                self.to_container_member(p);
            }
            return;
        }
        let after_parameters = self.child(n, "FormalParameterList").map_or(k.class_name.span.end, |l| self.end(l));
        let separator = self.tok_after(after_parameters).filter(|&i| self.is_op(i, Op::Colon) || self.is_op(i, Op::Assign));
        let Some(sep) = separator else { return };
        if self.is_op(sep, Op::Colon) {
            let body = self.function_body(n).map_or(self.end(n), |c| self.start(c));
            if self.offset >= self.span_tok(sep).end && self.offset <= body {
                self.local("ConstructorDeclaration_initializer");
                self.to_constructor_initializer(n, None);
            }
        } else {
            let Some(f) = self.declared_constructor(n) else { return };
            let constant = self.query.program.function(f).const_;
            self.dh(Flags { constant, ..Flags::default() });
            self.dh_redirections(f);
        }
    }

    pub(super) fn v_field_initializer(&mut self, n: usize) {
        let Some(ctor) = self.parent(n) else { return };
        if self.kind(ctor) != "ConstructorDeclaration" {
            return;
        }
        let equal = self.op_in(Op::Assign, self.start(n), self.end(n));
        let after_equal = equal.map_or(self.synthetic(self.start(n)).start, |i| self.span_tok(i).start);
        if self.offset <= after_equal {
            self.local("ConstructorDeclaration_initializer");
            self.to_constructor_initializer(ctor, Some(n));
        } else {
            self.local("ConstructorFieldInitializer_expression");
            self.to_expression(n, false, true);
        }
    }

    pub(super) fn v_constructor_name(&mut self, n: usize) {
        let Some(ty) = self.child(n, "NamedType") else { return };
        let parent = self.parent(n);
        if parent.is_some_and(|p| self.kind(p) == "ConstructorReference") {
            if let Some(c) = self.named_type_class(ty) {
                self.dh(Flags { prefer_without_invocation: true, ..Flags::default() });
                self.dh_element_constructor_names(c);
                self.static_dh(Element::Class(c));
            }
            return;
        }
        let Some(t) = self.type_node_type(ty) else { return };
        if !matches!(self.query.table.get(t), Type::Interface { .. } | Type::ExtensionType { .. }) {
            return;
        }
        if let Some(p) = parent
            && self.kind(p) == "ConstructorDeclaration"
            && let Some(k) = self.real_constructor(p)
            && k.factory
            && k.redirect.is_some()
        {
            let exclude = k.name.map(|x| self.real_name(x).to_string());
            self.dh(Flags { constant: k.const_, prefer_without_invocation: true, ..Flags::default() });
            self.dh_type_constructor_names(t, exclude);
            return;
        }
        self.dh(Flags::default());
        self.dh_type_constructor_names(t, None);
    }

    /// A classe que um `NamedType` nomeia.
    pub(super) fn named_type_class(&self, t: usize) -> Option<ClassId> {
        match self.query.table.get(self.type_node_type(t)?) {
            Type::Interface { class, .. } => Some(*class),
            Type::ExtensionType { decl, .. } => Some(*decl),
            _ => None,
        }
    }

    pub(super) fn v_constructor_selector(&mut self, n: usize) {
        self.local("ConstructorSelector_name");
        if !self.has_feature(super::isp::Feature::EnhancedEnums) {
            return;
        }
        let Some(args) = self.parent(n).filter(|&a| self.kind(a) == "EnumConstantArguments") else { return };
        let Some(constant) = self.parent(args).filter(|&c| self.kind(c) == "EnumConstantDeclaration") else { return };
        let Some(e) = self.parent(constant).filter(|&e| self.kind(e) == "EnumDeclaration") else { return };
        let Some(class) = self.node_class(e) else { return };
        self.dh(Flags { unnamed_as_new: true, ..Flags::default() });
        self.dh_element_constructor_names(class);
    }

    pub(super) fn v_declared_variable_pattern(&mut self, n: usize) {
        let name = self.variable_pattern_name(n);
        let ty = self.child(n, "NamedType").or_else(|| self.children(n).first().copied());
        let word = self.tok_after(self.start(n)).filter(|&i| matches!(self.text_tok(i), "var" | "final"));
        let Some(name) = name else {
            if ty.is_none() && word.is_none() {
                self.to_type(n, TypeOptions { non_void: true, ..TypeOptions::default() });
                return;
            }
            self.to_name_in_variable_pattern(n);
            return;
        };
        let s = self.span_tok(name);
        if self.covers(s.start, s.end) {
            self.to_name_in_variable_pattern(n);
            return;
        }
        if word.is_some() {
            if ty.is_none() && self.offset < s.start {
                self.dh(Flags { ty: true, ..Flags::default() });
                self.lexical_dh(n);
                return;
            }
        }
        let parent = self.parent(n);
        let has_when = parent.is_some_and(|p| self.kind(p) == "GuardedPattern" && self.child(p, "WhenClause").is_some());
        if !has_when {
            self.kw("when");
        }
    }

    pub(super) fn v_default_parameter(&mut self, n: usize) {
        let value = self.children(n).get(1).copied();
        if let Some(v) = value
            && self.covers_node(v)
        {
            self.local("DefaultFormalParameter_defaultValue");
            self.to_expression(v, false, true);
        } else if let Some(&p) = self.children(n).first() {
            self.visit(p);
        }
    }

    pub(super) fn v_do(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.local("Block_statement");
            self.to_command(n);
            return;
        }
        let body = self.children(n).first().copied();
        let after_body = body.map_or(self.span_tok(word).end, |c| self.end(c));
        let Some(opens) = self.op_in(Op::LParen, after_body, self.end(n)) else { return };
        let closes = self.pair(opens).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if self.span_tok(opens).end <= self.offset && self.offset <= closes {
            let condition = self.children(n).get(1).copied();
            let ok = match condition {
                None => true,
                Some(c) => self.offset <= self.start(c) || self.offset == self.end(c),
            };
            if ok {
                self.local("DoStatement_condition");
                self.to_expression(n, false, true);
            }
        }
    }

    pub(super) fn v_empty_command(&mut self, n: usize) {
        if let Some(p) = self.parent(n) {
            match self.kind(p) {
                "Block" => {
                    self.local("Block_statement");
                    let commands: Vec<usize> = self.children(p).to_vec();
                    if let Some(i) = commands.iter().position(|&k| k == n)
                        && i > 0
                    {
                        let previous = commands[i - 1];
                        if self.kind(previous) == "TryStatement" && self.word_in("finally", self.start(previous), self.end(previous)).is_none() {
                            self.try_clauses_kw(true);
                            if self.children_of(previous, &["CatchClause"]).is_empty() {
                                return;
                            }
                        }
                    }
                }
                "IfStatement" => {
                    let then = self.children(p).iter().copied().filter(|&k| super::isp::is_command(self.kind(k))).next();
                    if then == Some(n) {
                        self.local("IfStatement_thenStatement");
                    } else {
                        self.local("IfStatement_elseStatement");
                    }
                }
                _ => {}
            }
        }
        if self.offset <= self.start(n) {
            self.to_command(n);
        }
    }

    pub(super) fn v_enum(&mut self, n: usize) {
        if !self.has_feature(super::isp::Feature::EnhancedEnums) {
            return;
        }
        let Some(d) = self.real_decl(n) else { return };
        let ast::DeclKind::Enum(x) = &d.kind else { return };
        let Some(enum_kw) = self.word_in("enum", self.start(n), x.name.span.start) else { return };
        if self.offset < self.span_tok(enum_kw).start {
            return;
        }
        if self.offset <= self.span_tok(enum_kw).end {
            self.kw("enum");
            return;
        }
        let (left, dir, left_s, dir_s) = self.keys(n);
        if self.offset <= x.name.span.end {
            self.ih(false);
            self.top_name_ih(left_s && dir_s);
            return;
        }
        if self.offset <= left.start {
            self.enum_declaration_kw(n);
            return;
        }
        if !dir_s && self.offset >= dir.end {
            return;
        }
        // O `;` depois das constantes.
        let last_constant = self.children_of(n, &["EnumConstantDeclaration"]).last().copied();
        let pv = self.op_in(Op::Semicolon, last_constant.map_or(left.end, |c| self.end(c)), dir.start);
        if let Some(p) = pv
            && self.offset >= self.span_tok(p).end
        {
            self.local("EnumDeclaration_member");
            self.to_enum_member(n);
        }
    }

    pub(super) fn v_expression_body(&mut self, n: usize) {
        let Some(&expr) = self.children(n).first() else { return };
        let arrow = self.op_in(Op::Arrow, self.start(n), self.start(expr));
        if let Some(s) = arrow
            && self.offset >= self.span_tok(s).end
            && self.offset <= self.end(expr)
        {
            self.local("ExpressionFunctionBody_expression");
            self.to_expression(expr, false, false);
        }
    }

    pub(super) fn v_expression_statement(&mut self, n: usize) {
        let parent = self.parent(n);
        if parent.is_some_and(|p| matches!(self.kind(p), "SwitchPatternCase" | "SwitchCase")) {
            self.local("SwitchMember_statement");
        } else {
            self.local("Block_statement");
        }
        if self.previous_incomplete_command(n) && self.loose_command(n) {
            if let Some(a) = self.previous_command(n)
                && self.kind(a) == "TryStatement"
            {
                return;
            }
        }
        let pv = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::Semicolon));
        if let Some(p) = pv
            && self.offset >= self.span_tok(p).end
        {
            self.to_command(n);
            return;
        }
        let Some(&expr) = self.children(n).first() else {
            self.to_expression(n, false, false);
            return;
        };
        match self.kind(expr) {
            "AsExpression" | "IsExpression" => self.visit(expr),
            "AssignmentExpression" => {
                if let Some(&side) = self.children(expr).first()
                    && self.offset <= self.end(side)
                {
                    match self.kind(side) {
                        "PrefixedIdentifier" => self.visit(side),
                        "SimpleIdentifier" => self.to_command(n),
                        _ => {}
                    }
                }
            }
            "CascadeExpression" => {
                if let Some(&target) = self.children(expr).first()
                    && self.offset <= self.end(target)
                {
                    let is_static = self.static_context(n);
                    self.dh(Flags { non_void: true, is_static, ..Flags::default() });
                    self.lexical_dh(n);
                }
            }
            "InstanceCreationExpression" | "MethodInvocation" => {
                if let Some(i) = self.tok_after(self.start(expr))
                    && self.offset <= self.span_tok(i).end
                {
                    self.to_command(n);
                }
            }
            "FunctionReference" => {
                if self.offset > self.end(expr)
                    && let Some(&f) = self.children(expr).first()
                    && self.kind(f) == "SimpleIdentifier"
                {
                    let name = self.source[self.start(f)..self.end(f)].to_string();
                    self.ih(false);
                    self.type_name_ih(&name);
                }
            }
            "PrefixedIdentifier" => {
                let (Some(&p), Some(&id)) = (self.children(expr).first(), self.children(expr).get(1)) else { return };
                if self.offset <= self.end(p) {
                    let is_static = self.static_context(n);
                    self.dh(Flags { non_void: true, is_static, ..Flags::default() });
                    self.lexical_dh(n);
                } else if self.offset <= self.end(id) {
                } else {
                    let name = self.source[self.start(id)..self.end(id)].to_string();
                    self.ih(false);
                    self.type_name_ih(&name);
                }
            }
            "SimpleIdentifier" => {
                if self.offset <= self.end(expr) {
                    self.to_command(n);
                } else {
                    let name = self.source[self.start(expr)..self.end(expr)].to_string();
                    self.ih(false);
                    self.type_name_ih(&name);
                }
            }
            _ => self.to_expression(n, false, false),
        }
    }

    /// `ExpressionStatement.isSingleIdentifier`: uma palavra sem `;` no
    /// texto.
    pub(super) fn loose_command(&self, n: usize) -> bool {
        let Some(i) = self.tok_after(self.start(n)) else { return false };
        self.word(i) && self.tok_before(self.end(n)) == Some(i)
    }

    pub(super) fn v_extends(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.kw("extends");
            return;
        }
        let Some(t) = self.child(n, "NamedType") else {
            self.local("ExtendsClause_superclass");
            self.to_type(n, TypeOptions { extensible: true, ..TypeOptions::default() });
            return;
        };
        let name_start = self.child(t, "ImportPrefixReference").map_or(self.start(t), |p| self.end(p));
        let name = self.tok_after(name_start);
        if name.is_some_and(|i| self.covers(self.span_tok(i).start, self.span_tok(i).end)) {
            self.local("ExtendsClause_superclass");
            self.to_type(n, TypeOptions { extensible: true, ..TypeOptions::default() });
        }
    }

    pub(super) fn v_extension(&mut self, n: usize) {
        if self.offset == self.start(n) {
            self.to_unit_member_before(n);
            return;
        }
        let Some(d) = self.real_decl(n) else { return };
        let ast::DeclKind::Extension(x) = &d.kind else { return };
        let Some(ext) = self.word_in("extension", self.start(n), self.end(n)) else { return };
        if self.offset < self.span_tok(ext).start {
            return;
        }
        if self.offset <= self.span_tok(ext).end {
            self.kw("extension");
            return;
        }
        match x.name {
            Some(name) if self.offset <= name.span.end => {
                self.kw("on");
                if self.has_feature(super::isp::Feature::InlineClass) {
                    self.annotated_text("type");
                }
                self.ih(false);
                self.top_name_ih(false);
                return;
            }
            _ => {
                self.ih(false);
                self.top_name_ih(false);
            }
        }
        let (left, dir, _, _) = self.keys(n);
        if self.offset <= left.start {
            self.local("ExtensionDeclaration_onClause");
            // `onClause.onKeyword.isSynthetic`: a cláusula sem `on` escrito.
            let on = self.word_in("on", self.start(n), left.start);
            if self.child(n, "ExtensionOnClause").is_some() && on.is_none() {
                self.extension_declaration_kw(n);
            }
            return;
        }
        if self.offset >= left.end && self.offset <= dir.start {
            self.local("ExtensionDeclaration_member");
            self.to_extension_member(n);
        }
    }

    pub(super) fn v_extension_on(&mut self, n: usize) {
        if let Some(on) = self.tok_after(self.start(n))
            && self.offset <= self.span_tok(on).end
        {
            self.kw("on");
            return;
        }
        self.local("ExtensionOnClause_extendedType");
        self.to_type(n, TypeOptions::default());
    }

    pub(super) fn v_extension_type(&mut self, n: usize) {
        if self.offset == self.start(n) {
            self.to_unit_member_before(n);
            return;
        }
        let Some(d) = self.real_decl(n) else { return };
        let ast::DeclKind::ExtensionType(x) = &d.kind else { return };
        let (left, dir, left_s, dir_s) = self.keys(n);
        if self.offset <= x.name.span.end {
            self.ih(false);
            self.top_name_ih(left_s && dir_s);
        } else if self.offset >= x.representation_span.end && (self.offset <= left.start || left_s) {
            self.kw("implements");
        } else if self.offset >= left.end && self.offset <= dir.start {
            self.local("ExtensionTypeDeclaration_member");
            self.to_extension_type_member(n);
        }
    }

    pub(super) fn v_field(&mut self, n: usize) {
        self.previous_incomplete_member(n);
        if let Some(i) = self.first_after_annotations(n)
            && self.offset <= self.span_tok(i).end
        {
            self.local("ClassDeclaration_member");
        }
        let Some(list) = self.child(n, "VariableDeclarationList") else { return };
        let Some(l) = self.real_list(list) else { return };
        match l.ty {
            None => {
                let Some(first) = l.variables.first() else { return };
                self.local("FieldDeclaration_fields");
                let name = first.name;
                let text = self.real_name(name);
                let is_word = ANALYZER_PUB_WORDS.contains(&text);
                if l.variables.len() == 1 && is_word && self.offset > name.span.end {
                    self.field_declaration_kw(n, Some(text));
                    self.dh(Flags { ty: true, ..Flags::default() });
                    self.lexical_dh(n);
                } else if self.offset < name.span.start {
                    self.field_declaration_kw(n, None);
                    self.to_type(n, TypeOptions { non_void: first.initializer.is_some(), ..TypeOptions::default() });
                } else if self.offset <= name.span.end {
                    self.field_declaration_kw(n, None);
                }
            }
            Some(t) => {
                let ts = self.a.ty(t).span;
                let previous = self.previous_member(n);
                if self.offset <= ts.start && previous.is_none_or(|a| self.offset >= self.end(a)) {
                    if let Some(p) = self.parent(n) {
                        self.to_container_member(p);
                    }
                } else if self.offset <= ts.end {
                    if self.loose_field(n) {
                        if let Some(p) = self.parent(n) {
                            self.to_container_member(p);
                        }
                    } else {
                        self.field_declaration_kw(n, None);
                        self.local("ClassDeclaration_member");
                        self.kw("var");
                        self.dh(Flags { ty: true, ..Flags::default() });
                        self.lexical_dh(n);
                    }
                }
            }
        }
    }

    /// `FieldDeclaration.isSingleIdentifier`: uma palavra e nenhum outro
    /// token (o `;` sintético).
    pub(super) fn loose_field(&self, n: usize) -> bool {
        let Some(i) = self.first_after_annotations(n) else { return false };
        self.word(i) && self.tok_before(self.end(n)) == Some(i)
    }

    pub(super) fn v_field_parameter(&mut self, n: usize) {
        let mut ctor = self.parent(n).and_then(|p| self.parent(p));
        if ctor.is_some_and(|c| self.kind(c) == "FormalParameterList") {
            ctor = ctor.and_then(|c| self.parent(c));
        }
        let Some(c) = ctor.filter(|&c| self.kind(c) == "ConstructorDeclaration") else { return };
        let field = self.real_parameter(n).and_then(|p| p.name).and_then(|name| {
            let class = self.parent(c).and_then(|k| self.node_class(k))?;
            let text = self.real_name(name);
            let pr = &self.query.program;
            pr.class(class).fields.iter().copied().find(|&v| self.query.name(pr.variable(v).name) == text)
        });
        self.dh(Flags::default());
        self.dh_fields_for_initializers(c, field);
    }

    pub(super) fn v_parameter_list(&mut self, n: usize) {
        if self.offset >= self.end(n)
            && let Some(p) = self.parent(n)
            && self.kind(p) == "FunctionExpression"
        {
            self.v_function_expression(p);
            return;
        }
        self.local("FormalParameterList_parameter");
        let parameters: Vec<usize> = self.children(n).to_vec();
        if let Some(previous) = self.element_before(&parameters) {
            if self.incomplete_parameter(previous) {
                self.visit(previous);
                return;
            }
            if self.kind(previous) == "SimpleFormalParameter"
                && let Some(p) = self.real_parameter(previous)
                && p.ty.is_none()
                && self.offset > self.end(previous)
                && let Some(name) = p.name
            {
                let text = self.real_name(name).to_string();
                self.ih(false);
                self.type_name_ih(&text);
            }
        }
        self.formal_parameter_kw(n);
        self.to_type(n, TypeOptions::default());
    }

    /// `FormalParameter.isIncomplete`.
    pub(super) fn incomplete_parameter(&self, n: usize) -> bool {
        let Some(p) = self.real_parameter(n) else { return false };
        match p.name {
            None => return true,
            Some(name) => {
                if ANALYZER_PUB_WORDS.contains(&self.real_name(name)) {
                    return true;
                }
            }
        }
        if self.kind(n) == "DefaultFormalParameter" {
            let separator = self.op_in(Op::Assign, self.start(n), self.end(n).max(self.start(n) + 1)).or_else(|| self.op_in(Op::Colon, self.start(n), self.end(n).max(self.start(n) + 1)));
            if separator.is_some() && p.default_value.is_none() {
                return true;
            }
        }
        false
    }

    pub(super) fn v_for_parts_with_declarations(&mut self, n: usize) {
        let parts = self.tree.partes_de_for.get(&n).cloned().unwrap_or_default();
        let list = parts.inicio;
        let after_list = list.map_or(self.start(n), |l| self.end(l));
        let p1 = self.op_in(Op::Semicolon, after_list, self.end(n).max(self.offset + 1));
        let p2 = p1.and_then(|a| self.op_in(Op::Semicolon, self.span_tok(a).end, usize::MAX));
        let end_p1 = p1.map_or(self.synthetic(after_list).end, |i| self.span_tok(i).end);
        let start_p2 = p2.map_or(self.synthetic(end_p1).start, |i| self.span_tok(i).start);
        if self.offset >= end_p1 && self.offset <= start_p2 {
            self.local("ForParts_condition");
            if let Some(c) = parts.condicao
                && self.kind(c) == "SimpleIdentifier"
                && p1.is_none()
                && p2.is_none()
            {
                self.kw("in");
                return;
            }
            self.to_expression(n, false, false);
        } else if p2.is_some_and(|i| self.offset >= self.span_tok(i).end) {
            self.local("ForParts_updater");
            self.to_expression(n, false, false);
        }
    }

    pub(super) fn v_for(&mut self, n: usize) {
        let Some(word) = self.word_in("for", self.start(n), self.end(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.local("Block_statement");
            self.to_command(n);
            return;
        }
        let opens = self.op_in(Op::LParen, self.span_tok(word).end, self.end(n));
        let closes = opens.and_then(|a| self.pair(a));
        let (ab, fe) = match (opens, closes) {
            (Some(a), Some(f)) => (self.span_tok(a).end, self.span_tok(f).start),
            (Some(a), None) => (self.span_tok(a).end, self.synthetic(self.end(n)).start),
            _ => (usize::MAX, 0),
        };
        if self.offset >= ab && self.offset <= fe {
            self.local("ForStatement_forLoopParts");
            let Some(&parts) = self.children(n).first() else { return };
            match self.kind(parts) {
                "ForEachPartsWithDeclaration" => {
                    if let Some(d) = self.child(parts, "DeclaredIdentifier") {
                        let name = self.tok_before(self.end(d)).unwrap_or(0);
                        if self.offset < self.span_tok(name).start {
                            let ty = self.child(d, "NamedType").or_else(|| self.children_of(d, &["GenericFunctionType", "RecordTypeAnnotation"]).first().copied());
                            let ok = match ty {
                                None => true,
                                Some(t) if self.kind(t) == "NamedType" => {
                                    let name_t = self.tok_after(self.child(t, "ImportPrefixReference").map_or(self.start(t), |p| self.end(p)));
                                    name_t.is_some_and(|i| self.offset <= self.span_tok(i).end)
                                }
                                _ => false,
                            };
                            if ok {
                                self.to_type(n, TypeOptions::default());
                            }
                        }
                    }
                }
                "ForEachPartsWithIdentifier" => {
                    if let Some(&id) = self.children(parts).first()
                        && self.offset < self.start(id)
                    {
                        self.to_type(n, TypeOptions::default());
                    }
                }
                "ForEachPartsWithPattern" | "ForPartsWithPattern" => {}
                "ForPartsWithDeclarations" => {
                    if let Some(list) = self.child(parts, "VariableDeclarationList")
                        && let Some(l) = self.real_list(list)
                    {
                        let word_list = self.tok_after(self.start(list)).filter(|&i| matches!(self.text_tok(i), "var" | "final" | "const"));
                        let p1 = self.op_in(Op::Semicolon, self.end(list), self.end(parts).max(self.offset + 1));
                        if l.variables.len() == 1 && word_list.is_some() && p1.is_none() {
                            let after = word_list.map(|i| i + 1);
                            if let Some(d) = after
                                && self.is_op(d, Op::LParen)
                                && let Some(f) = self.pair(d)
                                && self.offset >= self.span_tok(f).end
                            {
                                self.kw("in");
                            }
                        }
                        if let Some(t) = l.ty {
                            let ts = self.a.ty(t).span;
                            if self.covers(ts.start, ts.end)
                                && let Some(node_t) = self.children(list).iter().copied().find(|&k| self.link(k) == Ligacao::Tipo(t))
                            {
                                self.visit(node_t);
                            }
                        }
                    }
                }
                "ForPartsWithExpression" => {
                    let p1 = self.op_in(Op::Semicolon, self.start(parts), self.end(parts).max(self.offset + 1));
                    let initialization = self.tree.partes_de_for.get(&parts).and_then(|p| p.inicio);
                    if p1.is_none() && initialization.is_some_and(|i| self.kind(i) == "SimpleIdentifier") {
                        self.kw("final");
                        self.kw("var");
                        self.to_type(n, TypeOptions::default());
                    }
                }
                _ => {}
            }
        } else {
            self.local("ForStatement_body");
            self.to_command(n);
        }
    }

    pub(super) fn v_function_declaration(&mut self, n: usize) {
        if self.offset == self.start(n) {
            self.to_unit_member_before(n);
        }
        let Some(f) = self.real_function(n) else { return };
        let Some(name) = f.name else { return };
        let one_token = match f.return_type {
            None => true,
            Some(r) => {
                let s = self.a.ty(r).span;
                self.tok_after(s.start).is_some_and(|i| self.span_tok(i).end == s.end)
            }
        };
        if one_token && self.offset <= name.span.start {
            self.local("FunctionDeclaration_returnType");
            self.to_type(n, TypeOptions::default());
        }
    }

    pub(super) fn v_function_expression(&mut self, n: usize) {
        let parameters = self.child(n, "FormalParameterList").map(|p| self.end(p));
        let types = self.child(n, "TypeParameterList").map(|p| self.end(p));
        let start = parameters.or(types).unwrap_or(self.start(n));
        let body = self.function_body(n);
        let body_start = body.map_or(self.end(n), |c| self.start(c));
        if self.offset >= start && self.offset <= body_start {
            self.body_modifiers_kw(body);
            let declaration = self.parent(n);
            let unit = declaration.and_then(|d| self.parent(d));
            if body.is_some_and(|c| self.kind(c) == "EmptyFunctionBody")
                && declaration.is_some_and(|d| self.kind(d) == "FunctionDeclaration")
                && unit == Some(0)
            {
                self.to_unit_declaration();
            }
        }
    }

    pub(super) fn v_old_typedef(&mut self, n: usize) {
        if self.offset == self.start(n) {
            self.to_unit_member_before(n);
            return;
        }
        let Some(td) = self.word_in("typedef", self.start(n), self.end(n)) else { return };
        if self.offset <= self.span_tok(td).end {
            self.local("CompilationUnit_declaration");
            self.kw("typedef");
        } else if self.toks.get(td + 1).is_some_and(|t| self.offset <= t.span.end) {
            self.dh(Flags { ty: true, ..Flags::default() });
            self.lexical_dh(n);
        }
    }

    pub(super) fn v_function_parameter(&mut self, n: usize) {
        self.local("FormalParameterList_parameter");
        let Some(p) = self.real_parameter(n) else { return };
        match p.ty {
            Some(r) if self.offset <= self.a.ty(r).span.end => {
                if let Some(list) = self.parameter_list_of(n) {
                    self.formal_parameter_kw(list);
                }
                self.to_type(n, TypeOptions::default());
            }
            None if p.name.is_some_and(|nm| self.offset < nm.span.start) => self.to_type(n, TypeOptions::default()),
            _ => {}
        }
    }

    /// `parentFormalParameterList`.
    pub(super) fn parameter_list_of(&self, n: usize) -> Option<usize> {
        let mut p = self.parent(n)?;
        if self.kind(p) == "DefaultFormalParameter" {
            p = self.parent(p)?;
        }
        (self.kind(p) == "FormalParameterList").then_some(p)
    }

    pub(super) fn v_typedef(&mut self, n: usize) {
        if self.offset == self.start(n) {
            self.to_unit_member_before(n);
            return;
        }
        if let Some(td) = self.word_in("typedef", self.start(n), self.end(n))
            && self.covers(self.span_tok(td).start, self.span_tok(td).end)
        {
            self.kw("typedef");
            return;
        }
        let equal = self.op_in(Op::Assign, self.start(n), self.end(n));
        let pv = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::Semicolon)).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if let Some(i) = equal
            && self.offset >= self.span_tok(i).end
            && self.offset <= pv
        {
            self.local("GenericTypeAlias_type");
            self.to_type(n, TypeOptions::default());
        }
    }

    pub(super) fn v_if_element(&mut self, n: usize) {
        let Some(&expr) = self.children(n).first() else { return };
        let opens = self.op_in(Op::LParen, self.start(n), self.start(expr) + 1);
        let closes = opens.and_then(|a| self.pair(a));
        let fe = closes.map_or(self.synthetic(self.end(expr)).start, |i| self.span_tok(i).start);
        let case = self.child(n, "CaseClause");
        if self.offset > self.end(expr) && self.offset <= fe {
            match case {
                None => {
                    self.kw("case");
                    self.kw("is");
                }
                Some(c) => {
                    let gp = self.child(c, "GuardedPattern");
                    if gp.is_some_and(|g| self.has_when(g)) {
                        let without_expression = gp.and_then(|g| self.child(g, "WhenClause")).is_none_or(|w| self.children(w).is_empty());
                        if without_expression {
                            let is_static = self.static_context(n);
                            self.kw_expression(Some(n), false, is_static);
                        }
                    } else {
                        self.kw("when");
                    }
                }
            }
        } else if opens.is_some_and(|a| self.offset >= self.span_tok(a).end) && self.offset <= fe {
            self.local("IfElement_condition");
            self.to_expression(n, false, true);
        } else if closes.is_some_and(|f| self.offset >= self.span_tok(f).end) {
            let else_ = self.word_in("else", self.span_tok(closes.unwrap_or(0)).end, self.end(n));
            if else_.is_none_or(|e| self.offset <= self.span_tok(e).start) {
                self.local("IfElement_thenElement");
            } else {
                self.local("IfElement_elseElement");
            }
            if let Some(lit) = self.ancestor(n, &["ListLiteral", "SetOrMapLiteral"]) {
                let elements = self.literal_elements(lit);
                self.to_collection_element(lit, &elements);
            }
        }
    }

    /// `GuardedPattern.hasWhen`.
    pub(super) fn has_when(&self, gp: usize) -> bool {
        if self.child(gp, "WhenClause").is_some() {
            return true;
        }
        if let Some(&p) = self.children(gp).first()
            && self.kind(p) == "DeclaredVariablePattern"
            && let Some(name) = self.variable_pattern_name(p)
            && self.text_tok(name) == "when"
            && let Some(t) = self.child(p, "NamedType")
        {
            return self.child(t, "TypeArgumentList").is_none();
        }
        false
    }

    pub(super) fn v_if(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        let opens = self.op_in(Op::LParen, self.span_tok(word).end, self.end(n));
        let closes = opens.and_then(|a| self.pair(a));
        if opens.is_some() && closes.is_none() {
            self.kw("is");
            return;
        }
        let Some(&expr) = self.children(n).first() else { return };
        let fe = closes.map_or(self.synthetic(self.end(expr)).start, |i| self.span_tok(i).start);
        if self.offset <= self.span_tok(word).end {
            self.local("Block_statement");
            self.to_command(n);
        } else if self.offset > self.end(expr) && self.offset <= fe {
            self.local("IfStatement_condition");
            match self.child(n, "CaseClause") {
                None => {
                    self.kw("case");
                    self.kw("is");
                }
                Some(c) => {
                    let gp = self.child(c, "GuardedPattern");
                    if gp.is_some_and(|g| self.has_when(g)) {
                        let without_expression = gp.and_then(|g| self.child(g, "WhenClause")).is_none_or(|w| self.children(w).is_empty());
                        if without_expression {
                            self.to_expression(n, false, false);
                        }
                    } else {
                        self.kw("when");
                        if let Some(g) = gp
                            && let Some(&p) = self.children(g).first()
                            && self.kind(p) == "ConstantPattern"
                            && let Some(&e) = self.children(p).first()
                            && matches!(self.kind(e), "SimpleIdentifier" | "PrefixedIdentifier" | "TypeLiteral")
                            && self.end(e) < self.offset
                            && self.is_type_literal(e)
                        {
                            let name = self.source[self.start(e)..self.end(e)].rsplit('.').next().unwrap_or("").to_string();
                            self.ih(false);
                            self.type_name_ih(&name);
                        }
                    }
                }
            }
        } else if opens.is_some_and(|a| self.offset >= self.span_tok(a).end) && self.offset <= fe {
            self.local("IfStatement_condition");
            self.to_expression(n, false, true);
        } else if closes.is_some_and(|f| self.offset >= self.span_tok(f).end) {
            let else_ = closes.and_then(|f| self.word_in("else", self.span_tok(f).end, self.end(n)));
            if else_.is_none_or(|e| self.offset <= self.span_tok(e).start) {
                self.local("IfStatement_thenStatement");
            } else {
                self.local("IfStatement_elseStatement");
            }
            self.to_command(n);
        }
    }

    /// A expressão é um `TypeLiteral` (o nome resolve a um tipo).
    pub(super) fn is_type_literal(&self, e: usize) -> bool {
        let Some(id) = self.real_expr(e) else { return false };
        matches!(self.resolved(id), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))))
    }

    pub(super) fn v_implements(&mut self, n: usize) {
        if let Some(p) = self.tok_after(self.start(n))
            && self.offset <= self.span_tok(p).end
        {
            self.kw("implements");
        } else {
            self.local("ImplementsClause_interface");
            self.to_type(n, TypeOptions { implementable: true, ..TypeOptions::default() });
        }
    }

    pub(super) fn v_import(&mut self, n: usize) {
        let Some(word) = self.word_in("import", self.start(n), self.end(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.local("CompilationUnit_directive");
            self.to_unit_member_before(n);
            return;
        }
        let Some(uri) = self.children_of(n, &["SimpleStringLiteral", "StringInterpolation", "AdjacentStrings"]).first().copied() else { return };
        if self.offset <= self.start(uri) {
            return;
        }
        if self.offset >= self.end(uri) {
            self.local("CompilationUnit_directive");
            self.import_directive_kw(n);
        }
    }

    pub(super) fn v_prefix_reference(&mut self, n: usize) {
        let Some(parent) = self.parent(n) else { return };
        if self.kind(parent) != "NamedType" {
            return;
        }
        let name_start = self.end(n);
        let name = self.tok_after(name_start).filter(|&i| self.span_tok(i).start < self.end(parent).max(name_start + 1));
        let name_off = name.map_or(self.synthetic(name_start).start, |i| self.span_tok(i).start);
        if self.offset > name_off {
            return;
        }
        self.local("PropertyAccess_propertyName");
        if let Some(p) = self.import_prefix(n) {
            let creation = self.parent(parent).and_then(|x| self.parent(x)).is_some_and(|x| self.kind(x) == "InstanceCreationExpression");
            self.dh(Flags { exclude_type_names: creation, ty: !creation, non_void: creation, ..Flags::default() });
            self.dh_by_prefix(p);
            return;
        }
        // Não é prefixo: o "prefixo" é um valor (`x.^ y`) ou um tipo.
        let text = self.source[self.start(n)..self.end(n)].trim_end_matches('.').trim().to_string();
        let Some(s) = self.query.names.lookup(&text) else { return };
        let lib = self.query.program.library(self.library());
        let el = lib.scope.get(&s).and_then(|b| b.getter);
        match el {
            Some(Element::Class(c)) => {
                self.dh(Flags::default());
                self.static_dh(Element::Class(c));
            }
            Some(Element::Extension(x)) => {
                self.dh(Flags::default());
                self.static_dh(Element::Extension(x));
            }
            Some(Element::Variable(v)) => {
                if let Some(t) = self.query.variable_type(v) {
                    self.dh(Flags::default());
                    self.dh_instance_members(t, false);
                }
            }
            Some(Element::Function(f)) => {
                let fe = self.query.program.function(f);
                let data = &self.query.outline.functions[f.0 as usize];
                let t = if matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) { data.return_type } else { data.signature };
                self.dh(Flags::default());
                self.dh_instance_members(t, false);
            }
            _ => {}
        }
    }

    pub(super) fn v_index(&mut self, n: usize) {
        if let Initial::Tok(i) = self.initial(n)
            && self.offset <= self.span_tok(i).end
            && self.word(i)
        {
            if let Some(p) = self.parent(n) {
                self.visit(p);
            }
            return;
        }
        let Some(&target) = self.children(n).first() else { return };
        let opens = self.op_in(Op::LBracket, self.end(target), self.end(n)).or_else(|| self.op_in(Op::QuestionDot, self.end(target), self.end(n)).map(|i| i + 1));
        let Some(a) = opens else { return };
        let closes = self.pair(a).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if self.offset >= self.span_tok(a).end && self.offset <= closes {
            self.local("IndexExpression_index");
            self.to_expression(n, false, true);
        }
    }

    pub(super) fn v_creation(&mut self, n: usize) {
        let word = self.tok_after(self.start(n)).filter(|&i| matches!(self.text_tok(i), "new" | "const"));
        match word {
            Some(k) if self.offset > self.span_tok(k).end => {
                let name = self.child(n, "ConstructorName");
                let ok = match name {
                    None => true,
                    Some(c) => self.offset < self.start(c) || self.covers_node(c),
                };
                if ok {
                    self.local("InstanceCreationExpression_constructorName");
                    self.dh(Flags::default());
                    self.dh_constructor_invocations();
                    self.dh_import_prefixes();
                }
            }
            _ => self.to_expression(n, false, false),
        }
    }

    pub(super) fn v_is(&mut self, n: usize) {
        let Some(&expr) = self.children(n).first() else { return };
        let Some(op) = self.word_in("is", self.end(expr), self.end(n)) else { return };
        let s = self.span_tok(op);
        if self.covers(s.start, s.end) {
            self.kw("is");
        } else if self.offset < s.start {
            self.to_expression(n, false, false);
        } else if self.offset > s.end {
            self.local("IsExpression_type");
            self.dh(Flags { ty: true, ..Flags::default() });
            self.lexical_dh(n);
        }
    }

    pub(super) fn v_label(&mut self, n: usize) {
        let name_end = self.tok_after(self.start(n)).map_or(self.start(n), |i| self.span_tok(i).end);
        if self.offset >= name_end
            && let Some(p) = self.parent(n)
            && self.kind(p) == "NamedExpression"
        {
            self.visit(p);
        }
    }

    pub(super) fn v_library(&mut self, n: usize) {
        if self.offset >= self.end(n) && self.parent(n) == Some(0) {
            self.kw_directive(Some(n));
            let (_, after) = self.member_neighbors(n);
            if after.is_none_or(|d| !self.is_directive(d)) {
                self.to_unit_declaration();
            }
        }
    }

    pub(super) fn v_collection_literal(&mut self, n: usize) {
        let opens = self.op_in(if self.kind(n) == "ListLiteral" { Op::LBracket } else { Op::LBrace }, self.start(n), self.end(n));
        let Some(a) = opens else { return };
        let closes = self.pair(a).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if self.offset >= self.span_tok(a).end && self.offset <= closes {
            self.local(if self.kind(n) == "ListLiteral" { "ListLiteral_element" } else { "SetOrMapLiteral_element" });
            let elements = self.literal_elements(n);
            self.to_collection_element(n, &elements);
        }
    }

    pub(super) fn v_map_entry(&mut self, n: usize) {
        if self.offset == self.start(n) {
            if let Some(p) = self.parent(n) {
                self.visit(p);
            }
            return;
        }
        let Some(&key) = self.children(n).first() else { return };
        if let Some(s) = self.op_in(Op::Colon, self.end(key), self.end(n))
            && self.offset >= self.span_tok(s).end
        {
            self.local("MapLiteralEntry_value");
            let is_static = self.static_context(n);
            self.dh(Flags { is_static, ..Flags::default() });
            self.lexical_dh(n);
        }
    }

    pub(super) fn v_map_pattern_entry(&mut self, n: usize) {
        let Some(&key) = self.children(n).first() else { return };
        let sep = self.op_in(Op::Colon, self.end(key), self.end(n).max(self.end(key) + 1));
        if sep.is_none_or(|s| self.offset <= self.span_tok(s).start) {
            if let Some(p) = self.parent(n) {
                self.visit(p);
            }
            return;
        }
        self.local("MapPatternEntry_value");
        self.to_pattern(n, false);
    }

    pub(super) fn v_method(&mut self, n: usize) {
        self.local("ClassDeclaration_member");
        let Some(f) = self.real_function(n) else { return };
        let first = self.first_after_annotations(n);
        let previous = first.and_then(|i| i.checked_sub(1));
        let start = previous.map_or(self.start(n), |i| self.span_tok(i).start);
        if let Some(name) = f.name
            && self.offset >= start
            && self.offset <= name.span.end
        {
            self.to_type(n, TypeOptions::default());
            self.class_member_kw();
        }
        let body = self.function_body(n);
        let body_start = body.map_or(self.synthetic(self.end(n)).start, |c| self.start(c));
        let before_body = self.tok_before(body_start).map_or(body_start, |i| self.span_tok(i).end);
        if self.offset >= before_body && self.offset <= body_start {
            if f.modifier == ast::AsyncModifier::None {
                self.body_modifiers_kw(body);
            }
            if body.is_none_or(|c| self.kind(c) == "EmptyFunctionBody") {
                self.class_member_kw();
            }
        }
    }

    pub(super) fn v_invocation(&mut self, n: usize) {
        if let Initial::Tok(i) = self.initial(n)
            && self.offset <= self.span_tok(i).end
            && self.word(i)
        {
            if let Some(p) = self.parent(n) {
                self.visit(p);
            }
            return;
        }
        // O operador `.`/`?.`/`..` antes do nome do método.
        let name = self.children_of(n, &["SimpleIdentifier"]).last().copied();
        let operator = name.and_then(|m| self.tok_before(self.start(m))).filter(|&i| {
            self.is_op(i, Op::Dot) || self.is_op(i, Op::QuestionDot) || self.is_op(i, Op::DotDot) || self.is_op(i, Op::QuestionDotDot)
        });
        let Some(op) = operator else {
            if self.covers_node(n) {
                let parent = self.parent(n);
                let mut non_void = false;
                if let Some(p) = parent {
                    match self.kind(p) {
                        "ArgumentList" => {
                            let l = self.argument_location(p, false);
                            self.local(&l);
                            non_void = true;
                        }
                        "NamedExpression" => {
                            if let Some(grandparent) = self.parent(p)
                                && self.kind(grandparent) == "ArgumentList"
                            {
                                let l = self.argument_location(grandparent, true);
                                self.local(&l);
                            }
                            non_void = true;
                        }
                        "RecordLiteral" => {
                            self.local("RecordLiteral_fields");
                            non_void = true;
                        }
                        _ => {}
                    }
                }
                self.to_expression(n, false, non_void);
            }
            return;
        };
        let cascade = self.is_op(op, Op::DotDot) || self.is_op(op, Op::QuestionDotDot);
        let name_end = name.map_or(self.end(n), |m| self.end(m));
        if (cascade && self.offset == self.span_tok(op).start + 1) || (self.offset >= self.span_tok(op).end && self.offset <= name_end) {
            let target = self.children(n).first().copied().filter(|&a| Some(a) != name);
            let ty = target.and_then(|a| self.real_expr(a)).and_then(|e| self.static_type(e));
            if let Some(t) = ty {
                self.to_member_access(n, t, false);
            }
            let without_type = ty.is_none_or(|t| self.query.table.e_invalido(t) || self.is_type_type(t));
            if without_type
                && let Some(a) = target
                && matches!(self.kind(a), "SimpleIdentifier" | "PrefixedIdentifier")
                && (!cascade || self.offset == self.span_tok(op).start + 1)
            {
                self.static_or_prefix(a);
            }
        }
    }

    /// `type.isDartCoreType`.
    pub(super) fn is_type_type(&self, t: TypeId) -> bool {
        t == self.query.core.type_
    }

    /// `staticElement` do identificador alvo: classe ou tipo de extensão →
    /// estáticos; prefixo → as declarações por ele.
    pub(super) fn static_or_prefix(&mut self, target: usize) {
        if let Some(p) = self.import_prefix(target) {
            self.dh(Flags::default());
            self.dh_by_prefix(p);
            return;
        }
        let Some(e) = self.real_expr(target) else { return };
        match self.resolved(e) {
            Some(Resolved::Element(Element::Class(c))) => {
                self.dh(Flags::default());
                self.static_dh(Element::Class(c));
            }
            Some(Resolved::Element(Element::Typedef(t))) => {
                self.dh(Flags::default());
                self.static_dh(Element::Typedef(t));
            }
            _ => {}
        }
    }

    pub(super) fn v_mixin(&mut self, n: usize) {
        if self.offset == self.start(n) {
            self.to_unit_member_before(n);
            return;
        }
        let Some(d) = self.real_decl(n) else { return };
        let ast::DeclKind::Mixin(x) = &d.kind else { return };
        let Some(m) = self.word_in("mixin", self.start(n), x.name.span.start) else { return };
        if self.offset < self.span_tok(m).start {
            self.mixin_modifiers_kw(n);
            return;
        }
        if self.offset <= self.span_tok(m).end {
            self.kw("mixin");
            return;
        }
        let (left, dir, left_s, dir_s) = self.keys(n);
        if self.offset <= x.name.span.end {
            self.ih(false);
            self.top_name_ih(left_s && dir_s);
            return;
        }
        if self.offset <= left.start {
            self.mixin_declaration_kw(n);
            return;
        }
        if self.offset >= left.end && self.offset <= dir.start {
            self.local("MixinDeclaration_member");
            if self.annotation_at_body_end(n) {
                return;
            }
            self.to_mixin_member(n);
            let members = self.children_of(n, &["FieldDeclaration", "MethodDeclaration", "ConstructorDeclaration"]);
            if let Some(m) = self.element_before(&members)
                && self.kind(m) == "MethodDeclaration"
                && self.function_body(m).is_none_or(|c| self.kind(c) == "EmptyFunctionBody")
            {
                let body = self.function_body(m);
                self.body_modifiers_kw(body);
            }
        }
    }

    pub(super) fn v_mixin_on(&mut self, n: usize) {
        if let Some(on) = self.tok_after(self.start(n))
            && self.offset <= self.span_tok(on).end
        {
            self.kw("on");
        } else {
            self.to_type(n, TypeOptions::default());
        }
    }

    pub(super) fn v_named_expression(&mut self, n: usize) {
        let Some(label) = self.child(n, "Label") else { return };
        let name_end = self.tok_after(self.start(label)).map_or(self.start(label), |i| self.span_tok(i).end);
        let name_text = self.source[self.start(label)..name_end].to_string();
        if self.offset <= name_end {
            let Some(parent) = self.parent(n) else { return };
            match self.kind(parent) {
                "ArgumentList" => {
                    let l = self.argument_location(parent, true);
                    self.local(&l);
                    if let Some(parameters) = self.invoked_parameters(parent) {
                        let mut used: HashSet<String> = HashSet::new();
                        for &a in self.children(parent) {
                            if let Some(r) = self.child(a, "Label") {
                                used.insert(self.source[self.start(r)..self.end(r)].trim_end_matches(':').trim().to_string());
                            }
                        }
                        used.remove(&name_text);
                        let colon = self.tok_after(name_end).is_none_or(|i| !self.is_op(i, Op::Colon));
                        for p in parameters {
                            if p.named && !used.contains(&p.name) {
                                self.suggest_named_argument(&p, colon, false, None);
                            }
                        }
                    }
                }
                "RecordLiteral" => {
                    self.local("RecordLiteral_fields");
                    self.suggest_record_named_fields(n, Some(parent), false);
                }
                _ => {}
            }
        } else if self.offset >= self.end(label) {
            let in_arguments = self.parent(n).is_some_and(|p| self.kind(p) == "ArgumentList");
            if in_arguments {
                self.local("ArgumentList_method_named");
            }
            self.to_expression(n, false, in_arguments);
            // `staticParameterElement`: o tipo do parâmetro nomeado.
            if let Some(list) = self.parent(n).filter(|&p| self.kind(p) == "ArgumentList")
                && let Some(ps) = self.invoked_parameters(list)
                && let Some(p) = ps.into_iter().find(|p| p.named && p.name == name_text)
                && matches!(self.query.table.get(p.ty), Type::Function { .. })
            {
                let comma = !self.followed_by_comma(n);
                self.suggest_closure_with_names(p.ty, &p.type_names, comma);
            }
        }
    }

    pub(super) fn v_named_type(&mut self, n: usize) {
        // `prefix.x^ print(0);` recuperado como declaração de variável.
        if let Some(p) = self.child(n, "ImportPrefixReference")
            && let Some(prefix) = self.import_prefix(p)
            && let Some(list) = self.parent(n).filter(|&l| self.kind(l) == "VariableDeclarationList")
            && let Some(cmd) = self.parent(list).filter(|&c| self.kind(c) == "VariableDeclarationStatement")
            && !self.tok_before(self.end(cmd)).is_some_and(|i| self.is_op(i, Op::Semicolon))
        {
            self.dh(Flags::default());
            self.dh_by_prefix(prefix);
            return;
        }
        if let Some(parent) = self.parent(n) {
            match self.kind(parent) {
                "ImplementsClause" => self.local("ImplementsClause_interface"),
                "TypeArgumentList" => self.local("TypeArgumentList_argument"),
                "WithClause" => self.local("WithClause_mixinType"),
                _ => {}
            }
        }
        let creation = self.parent(n).and_then(|p| self.parent(p)).is_some_and(|x| self.kind(x) == "InstanceCreationExpression");
        self.to_type(n, TypeOptions { exclude_type_names: creation, ..TypeOptions::default() });
    }

    pub(super) fn v_object_pattern(&mut self, n: usize) {
        let opens = self.op_in(Op::LParen, self.start(n), self.end(n));
        let Some(a) = opens else { return };
        let closes = self.pair(a).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if self.span_tok(a).end <= self.offset && self.offset <= closes {
            self.local("ObjectPattern_fieldName");
            let excluded = self.field_names(n);
            if let Some(t) = self.child(n, "NamedType").and_then(|t| self.type_node_type(t)) {
                self.dh(Flags { non_void: true, ..Flags::default() });
                self.dh_getters(t, &excluded);
            }
        }
    }

    pub(super) fn v_parens(&mut self, n: usize) {
        let Some(&expr) = self.children(n).first() else { return };
        if matches!(self.kind(expr), "SimpleIdentifier" | "PrefixedIdentifier" | "PropertyAccess")
            && let Some(f) = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::RParen))
            && self.offset == self.span_tok(f).start
            && self.tok_after(self.end(expr)).is_some_and(|i| self.is_identifier(i) && i < f)
        {
            self.kw("is");
            return;
        }
        self.local("ParenthesizedExpression_expression");
        if self.kind(expr) == "SimpleIdentifier" {
            self.suggest_record_named_fields(n, None, true);
        }
        self.to_expression(n, false, false);
    }

    pub(super) fn v_pattern_field(&mut self, n: usize) {
        let name = self.child(n, "PatternFieldName");
        let parent = self.parent(n);
        if let Some(nm) = name {
            let colon = self.tok_before(self.end(nm)).map_or(self.end(nm), |i| self.span_tok(i).start);
            if self.offset <= colon {
                if parent.is_some_and(|p| self.kind(p) == "ObjectPattern") {
                    self.local("ObjectPattern_fieldName");
                } else {
                    self.local("PatternField_pattern");
                }
                if let Some(p) = parent {
                    self.to_field_name_in_pattern(p);
                }
                return;
            }
        }
        match name {
            None => match parent.map(|p| self.kind(p)) {
                Some("ObjectPattern") => {
                    self.local("ObjectPattern_fieldName");
                    let p = parent.unwrap_or(n);
                    let excluded = self.field_names(p);
                    if let Some(t) = self.child(p, "NamedType").and_then(|t| self.type_node_type(t)) {
                        self.dh(Flags { non_void: true, ..Flags::default() });
                        self.dh_getters(t, &excluded);
                    }
                }
                Some("RecordPattern") => {
                    self.local("PatternField_pattern");
                    self.to_pattern(n, true);
                }
                _ => {}
            },
            Some(nm) => {
                let has_name = self.tok_after(self.start(nm)).is_some_and(|i| self.word(i) && self.span_tok(i).end <= self.end(nm));
                self.local("PatternField_pattern");
                if !has_name {
                    self.to_variable_pattern();
                    if let Some(p) = parent {
                        self.to_field_name_in_pattern(p);
                    }
                } else {
                    self.to_pattern(n, false);
                }
            }
        }
    }

    pub(super) fn v_pattern_field_name(&mut self, n: usize) {
        let colon = self.tok_before(self.end(n)).map_or(self.end(n), |i| self.span_tok(i).start);
        if self.offset <= colon {
            let external = self.parent(n).and_then(|p| self.parent(p));
            if external.is_some_and(|x| self.kind(x) == "ObjectPattern") {
                self.local("ObjectPattern_fieldName");
            } else {
                self.local("PatternField_pattern");
            }
            if let Some(x) = external
                && super::isp::is_pattern(self.kind(x))
            {
                self.to_field_name_in_pattern(x);
            }
        }
    }

    pub(super) fn v_postfix(&mut self, n: usize) {
        let Some(&operand) = self.children(n).first() else { return };
        let op = self.source[self.end(operand)..self.end(n)].trim().to_string();
        self.local(&format!("PrefixExpression_{op}_operand"));
        let assignable = op == "++" || op == "--";
        self.to_expression(n, assignable, false);
    }

    pub(super) fn v_prefixed_identifier(&mut self, n: usize) {
        let (Some(&prefix), Some(&id)) = (self.children(n).first(), self.children(n).get(1)) else { return };
        let dot = self.end(prefix);
        if self.offset <= dot {
            self.to_expression(n, false, false);
            return;
        }
        let _ = id;
        self.local("PropertyAccess_propertyName");
        let ty = self.real_expr(prefix).and_then(|e| self.static_type(e));
        // Prefixo de import ou nome de classe: o `staticType` é nulo no
        // analyzer.
        let element = self.real_expr(prefix).and_then(|e| self.resolved(e));
        let is_type_or_prefix = matches!(element, Some(Resolved::Prefix(_) | Resolved::Element(Element::Class(_) | Element::Typedef(_) | Element::Extension(_))));
        match ty {
            Some(t) if !is_type_or_prefix => {
                let so_super = self.kind(prefix) == "SuperExpression";
                self.to_member_access(n, t, so_super);
            }
            _ => {
                let assignable = self.parent(n).is_some_and(|p| self.kind(p) == "AssignmentExpression" && self.children(p).first() == Some(&n));
                if let Some(p) = self.import_prefix(prefix) {
                    self.dh(Flags { assignable, ..Flags::default() });
                    self.dh_by_prefix(p);
                } else if let Some(el) = element {
                    let element = match el {
                        Resolved::Element(e) => Some(e),
                        _ => None,
                    };
                    if let Some(e) = element {
                        let prefer = matches!(e, Element::Class(_)) && self.preferred_tear_off();
                        self.dh(Flags { assignable, prefer_without_invocation: prefer, ..Flags::default() });
                        self.static_dh(e);
                    }
                }
            }
        }
    }

    /// `request.shouldSuggestTearOff(element)`: o contexto é um tipo de
    /// função (sugere o tear-off do construtor).
    pub(super) fn preferred_tear_off(&self) -> bool {
        self.context_type.is_some_and(|t| matches!(self.query.table.get(t), Type::Function { .. }))
    }

    pub(super) fn v_prefix(&mut self, n: usize) {
        self.local("PropertyAccess_propertyName");
        let op = self.tok_after(self.start(n)).map(|i| self.text_tok(i)).unwrap_or("");
        let assignable = op == "++" || op == "--";
        self.to_expression(n, assignable, false);
    }

    pub(super) fn v_property_access(&mut self, n: usize) {
        let (Some(&target), name) = (self.children(n).first(), self.children(n).get(1).copied()) else { return };
        let operator = name.and_then(|m| self.tok_before(self.start(m))).or_else(|| self.tok_after(self.end(target)));
        let Some(op) = operator else { return };
        if self.offset <= self.span_tok(op).start {
            self.to_expression(n, false, false);
            return;
        }
        self.local("PropertyAccess_propertyName");
        let parent = self.parent(n);
        if self.kind(target) == "ThisExpression" && parent.is_some_and(|p| self.kind(p) == "ConstructorFieldInitializer") {
            if let Some(p) = parent {
                self.visit(p);
            }
            return;
        }
        let ty = self.real_expr(target).and_then(|e| self.static_type(e));
        if let Some(t) = ty {
            let so_super = self.kind(target) == "SuperExpression";
            self.to_member_access(n, t, so_super);
        }
        let cascade = self.is_op(op, Op::DotDot) || self.is_op(op, Op::QuestionDotDot);
        let without_type = ty.is_none_or(|t| self.query.table.e_invalido(t) || self.is_type_type(t));
        if without_type && matches!(self.kind(target), "SimpleIdentifier" | "PrefixedIdentifier") && (!cascade || self.offset == self.span_tok(op).start + 1) {
            self.static_or_prefix(target);
        }
    }

    pub(super) fn v_record_pattern(&mut self, n: usize) {
        let opens = self.tok_at(self.start(n)).filter(|&i| self.is_op(i, Op::LParen));
        let Some(a) = opens else { return };
        if self.offset == self.span_tok(a).start {
            self.local("ObjectPattern_type");
            self.dh(Flags { ty: true, non_void: true, ..Flags::default() });
            self.lexical_dh(n);
            return;
        }
        let closes = self.pair(a).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if self.span_tok(a).end <= self.offset && self.offset <= closes {
            self.local("PatternField_pattern");
            self.kw("dynamic");
            self.to_expression(n, false, false);
            let fields = self.children_of(n, &["PatternField"]);
            let target = fields.iter().copied().find(|&c| self.end(c) >= self.offset);
            if let Some(c) = target
                && let Some(nm) = self.child(c, "PatternFieldName")
                && self.offset <= self.tok_before(self.end(nm)).map_or(self.end(nm), |i| self.span_tok(i).start)
                && let Some(t) = self.matched_type(n)
            {
                let excluded = self.field_names(n);
                self.dh(Flags { non_void: true, ..Flags::default() });
                self.dh_getters(t, &excluded);
            }
        }
    }

    pub(super) fn v_record_type(&mut self, n: usize) {
        if self.offset <= self.start(n) {
            let mut parent = self.parent(n);
            if parent.is_some_and(|p| self.kind(p) == "DefaultFormalParameter") {
                parent = parent.and_then(|p| self.parent(p));
            }
            if let Some(p) = parent
                && super::isp::is_parameter(self.kind(p))
                && self.offset <= self.start(p)
            {
                self.local("FormalParameterList_parameter");
                self.to_type(n, TypeOptions::default());
            }
            return;
        }
        let closes = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::RParen)).map_or(self.end(n), |i| self.span_tok(i).start);
        if self.offset <= closes {
            self.local("RecordTypeAnnotation_positionalFields");
            self.to_type(n, TypeOptions::default());
        }
    }

    pub(super) fn v_record_type_named_field(&mut self, n: usize) {
        let ty = self.children(n).first().copied();
        if let Some(t) = ty
            && self.covers_node(t)
        {
            self.local("RecordTypeAnnotationNamedFields_fields");
            self.to_type(n, TypeOptions::default());
            return;
        }
        let name = self.tok_before(self.end(n));
        if let Some(i) = name
            && self.covers(self.span_tok(i).start, self.span_tok(i).end)
        {
            self.local("RecordTypeAnnotationNamedField_name");
            self.ih(false);
            self.ih_variable(ty);
        }
    }

    pub(super) fn v_redirecting_invocation(&mut self, n: usize) {
        let Some(ctor) = self.parent(n).filter(|&c| self.kind(c) == "ConstructorDeclaration") else { return };
        self.local("ConstructorDeclaration_initializer");
        let Some(this) = self.tok_after(self.start(n)) else { return };
        let list = self.child(n, "ArgumentList");
        if self.offset <= self.span_tok(this).end && list.is_none() {
            self.constructor_initializer_kw(ctor, Some(n));
            return;
        }
        let dot = self.tok_after(self.span_tok(this).end).filter(|&i| self.is_op(i, Op::Dot));
        if let Some(p) = dot
            && self.offset >= self.span_tok(p).end
            && self.offset <= list.map_or(self.synthetic(self.end(n)).start, |l| self.start(l))
        {
            self.to_redirecting_invocation(ctor);
        }
    }

    pub(super) fn v_relational_pattern(&mut self, n: usize) {
        let Some(op) = self.tok_after(self.start(n)) else { return };
        let operand = self.children(n).first().copied();
        let next = op + 1;
        if self.is_op(op, Op::Lt) && operand.is_none() && self.is_op(next, Op::Gt) {
            self.local("TypeArgumentList_argument");
            self.to_type(n, TypeOptions::default());
        } else if let Some(o) = operand
            && self.kind(o) == "SimpleIdentifier"
            && self.offset >= self.span_tok(op).end
            && self.offset <= self.end(o)
        {
            self.local("RelationalPattern_operand");
            self.to_expression(n, false, false);
        }
    }

    pub(super) fn v_representation(&mut self, n: usize) {
        let Some(d) = self.parent(n).and_then(|p| self.real_decl(p)) else { return };
        let ast::DeclKind::ExtensionType(x) = &d.kind else { return };
        let name = x.representation_name;
        if self.offset <= name.span.end {
            let ty = self.children(n).iter().copied().find(|&k| self.link(k) == Ligacao::Tipo(x.representation_type));
            match ty {
                None => {
                    let opens = self.op_in(Op::LParen, self.start(n), self.end(n));
                    let annotation = opens.is_some_and(|a| self.is_op(a + 1, Op::At));
                    if annotation {
                        self.local("Annotation_name");
                        self.to_annotation(n);
                    } else {
                        self.local("RepresentationDeclaration_fieldType");
                        self.dh(Flags { ty: true, ..Flags::default() });
                        self.lexical_dh(n);
                    }
                }
                Some(t) => {
                    self.local("RepresentationDeclaration_fieldName");
                    self.ih(true);
                    self.ih_variable(Some(t));
                }
            }
        } else {
            self.local("RepresentationDeclaration_fieldName");
            let text = self.real_name(name).to_string();
            self.ih(true);
            self.type_name_ih(&text);
        }
    }

    pub(super) fn v_return(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.local("Block_statement");
            self.to_command(n);
        } else {
            self.local("ReturnStatement_expression");
            let target = self.children(n).first().copied().unwrap_or(n);
            self.to_expression(target, false, false);
        }
    }

    pub(super) fn v_simple_parameter(&mut self, n: usize) {
        let Some(p) = self.real_parameter(n) else { return };
        let list = self.parameter_list_of(n);
        if let Some(name) = p.name
            && self.one_token_parameter(n)
        {
            self.local("FormalParameterList_parameter");
            let text = self.real_name(name);
            if ANALYZER_PUB_WORDS.contains(&text) {
                if text == "required" && !p.covariant {
                    self.kw("covariant");
                }
                self.to_type(n, TypeOptions::default());
                return;
            }
            if let Some(l) = list {
                self.formal_parameter_kw(l);
            }
            self.to_type(n, TypeOptions::default());
        }
        match p.ty {
            Some(t) => {
                self.local("FormalParameterList_parameter");
                let node_t = self.children(n).iter().copied().find(|&k| self.link(k) == Ligacao::Tipo(t));
                if let Some(nt) = node_t
                    && self.kind(nt) == "NamedType"
                    && let Some(pr) = self.child(nt, "ImportPrefixReference")
                    && let Some(prefix) = self.import_prefix(pr)
                {
                    let name_t = self.tok_after(self.end(pr));
                    if name_t.is_some_and(|i| self.covers(self.span_tok(i).start, self.span_tok(i).end)) {
                        self.dh(Flags { ty: true, ..Flags::default() });
                        self.dh_by_prefix(prefix);
                    }
                }
                let ts = self.a.ty(t).span;
                let first = self.tok_after(ts.start);
                if first.is_some_and(|i| self.covers(self.span_tok(i).start, self.span_tok(i).end)) {
                    if let Some(l) = list {
                        self.formal_parameter_kw(l);
                    }
                    self.to_type(n, TypeOptions::default());
                } else if let ast::TypeKind::Function { return_type: None, .. } = &self.a.ty(t).kind
                    && let Some(f) = self.word_in("Function", ts.start, ts.end)
                    && self.offset < self.span_tok(f).start
                {
                    self.to_type(n, TypeOptions::default());
                }
            }
            None => {
                let word = self.tok_after(self.start(n)).filter(|&i| matches!(self.text_tok(i), "final" | "var" | "const"));
                if let Some(k) = word
                    && self.offset <= self.span_tok(k).end
                {
                    self.local("FormalParameterList_parameter");
                    if let Some(parent) = self.parent(n)
                        && self.kind(parent) == "FormalParameterList"
                    {
                        self.formal_parameter_kw(parent);
                    }
                    self.to_type(n, TypeOptions::default());
                }
            }
        }
    }

    /// `FormalParameter.isSingleIdentifier`: um só token, palavra.
    pub(super) fn one_token_parameter(&self, n: usize) -> bool {
        let Some(i) = self.tok_after(self.start(n)) else { return false };
        self.word(i) && self.span_tok(i).end == self.end(n)
    }

    pub(super) fn v_simple_string(&mut self, n: usize) {
        if let Some(p) = self.parent(n)
            && matches!(self.kind(p), "Configuration" | "PartOfDirective" | "ImportDirective" | "ExportDirective" | "PartDirective" | "AugmentationImportDirective")
        {
            self.uris(n);
            return;
        }
        self.visit_parent_if_node_or_before(n);
    }

    pub(super) fn v_super_invocation(&mut self, n: usize) {
        let Some(ctor) = self.parent(n).filter(|&c| self.kind(c) == "ConstructorDeclaration") else { return };
        self.local("ConstructorDeclaration_initializer");
        let Some(sup) = self.tok_after(self.start(n)) else { return };
        let list = self.child(n, "ArgumentList");
        if self.offset <= self.span_tok(sup).end && list.is_none() {
            self.constructor_initializer_kw(ctor, Some(n));
            return;
        }
        let dot = self.tok_after(self.span_tok(sup).end).filter(|&i| self.is_op(i, Op::Dot));
        if let Some(p) = dot
            && self.offset >= self.span_tok(p).end
            && self.offset <= list.map_or(self.synthetic(self.end(n)).start, |l| self.start(l))
        {
            let Some(container) = self.parent(ctor) else { return };
            if !matches!(self.kind(container), "ClassDeclaration" | "EnumDeclaration") {
                return;
            }
            let Some(class) = self.node_class(container) else { return };
            let supertype = self.query.outline.classes.get(class.0 as usize).and_then(|d| d.supertype);
            if let Some(st) = supertype {
                let constant = self.real_constructor(ctor).is_some_and(|k| k.const_);
                self.dh(Flags { constant, ..Flags::default() });
                self.dh_type_constructor_names(st, None);
            }
        }
    }

    pub(super) fn v_default(&mut self, n: usize) {
        let Some(k) = self.word_in("default", self.start(n), self.end(n)) else { return };
        if self.offset <= self.span_tok(k).start {
            self.local("SwitchMember_statement");
            self.kw("case");
            self.kw_and_text("default", ":");
        } else if self.offset <= self.span_tok(k).end {
            let colon = self.tok_after(self.span_tok(k).end).is_some_and(|i| self.is_op(i, Op::Colon));
            if !colon {
                self.kw_and_text("default", ":");
            } else {
                self.kw("default");
            }
        }
    }

    pub(super) fn v_switch_expression(&mut self, n: usize) {
        let opens = self.op_in(Op::LParen, self.start(n), self.end(n));
        let closes = opens.and_then(|a| self.pair(a));
        if let (Some(a), Some(f)) = (opens, closes)
            && self.offset >= self.span_tok(a).end
            && self.offset <= self.span_tok(f).start
        {
            self.local("SwitchExpression_expression");
            self.to_expression(n, false, false);
            return;
        }
        let after = closes.map_or(self.start(n), |f| self.span_tok(f).end);
        let key = self.op_in(Op::LBrace, after, self.end(n));
        if let Some(c) = key {
            let end = self.pair(c).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
            if self.offset >= self.span_tok(c).end && self.offset <= end {
                self.local("SwitchExpression_body");
                self.to_pattern(n, true);
            }
        }
    }

    pub(super) fn v_switch_expression_case(&mut self, n: usize) {
        let gp = self.child(n, "GuardedPattern");
        let arrow = self.op_in(Op::Arrow, gp.map_or(self.start(n), |g| self.end(g)), self.end(n).max(self.offset + 1));
        if arrow.is_none() {
            self.local("SwitchExpression_body");
            self.to_pattern(n, true);
            return;
        }
        let Some(expr) = self.children(n).iter().copied().find(|&k| Some(k) != gp) else { return };
        let one_token = self.tok_after(self.start(expr)).is_some_and(|i| self.span_tok(i).end == self.end(expr));
        if one_token {
            self.local("SwitchExpressionCase_expression");
            self.to_expression(expr, false, false);
        }
    }

    pub(super) fn v_pattern_case(&mut self, n: usize) {
        let covered = self.covered_node();
        let Some(case) = self.tok_after(self.start(n)) else { return };
        let gp = self.child(n, "GuardedPattern");
        let colon = self.op_in(Op::Colon, gp.map_or(self.span_tok(case).end, |g| self.end(g)), self.end(n).max(self.offset + 1));
        let dp = colon.map_or(self.synthetic(gp.map_or(self.span_tok(case).end, |g| self.end(g))).start, |i| self.span_tok(i).start);
        if self.offset <= self.span_tok(case).end {
            self.kw("case");
        } else if self.offset <= dp {
            self.local("SwitchPatternCase_pattern");
            if let Some(c) = covered
                && self.kind(c) == "NamedType"
                && self.parent(c).is_some_and(|p| self.kind(p) == "ObjectPattern")
            {
                self.local("ObjectPattern_type");
                self.visit(c);
                return;
            }
            let pattern = gp.and_then(|g| self.children(g).first().copied());
            if let Some(p) = pattern {
                if self.kind(p) == "ConstantPattern"
                    && let Some(&id) = self.children(p).first()
                    && self.kind(id) == "SimpleIdentifier"
                    && self.offset < self.start(id)
                {
                    self.dh(Flags { ty: true, ..Flags::default() });
                    self.lexical_dh(n);
                    return;
                }
                if self.kind(p) == "WildcardPattern" && self.offset < self.end(p).saturating_sub(1) {
                    self.dh(Flags { ty: true, ..Flags::default() });
                    self.lexical_dh(n);
                    return;
                }
                if self.kind(p) == "ConstantPattern"
                    && let Some(&e) = self.children(p).first()
                    && self.is_type_literal(e)
                    && self.end(e) < self.offset
                {
                    let name = self.source[self.start(e)..self.end(e)].rsplit('.').next().unwrap_or("").to_string();
                    self.ih(false);
                    self.type_name_ih(&name);
                    return;
                }
            }
            if let Some(c) = covered
                && self.kind(c) == "NamedType"
                && let Some(pp) = self.parent(c)
            {
                match self.kind(pp) {
                    "DeclaredVariablePattern" => {
                        self.visit(c);
                        return;
                    }
                    "ObjectPattern" => {
                        self.local("ObjectPattern_type");
                        self.visit(c);
                        return;
                    }
                    "WildcardPattern" => {
                        self.local("WildcardPattern_type");
                        self.visit(c);
                        return;
                    }
                    _ => {}
                }
            }
            let previous = colon.and_then(|d| d.checked_sub(1)).or_else(|| self.tok_before(dp));
            let Some(prev) = previous else { return };
            let word = self.is_keyword(prev).then(|| self.text_tok(prev));
            match word {
                None => {
                    if colon.is_none() || self.covers(self.span_tok(prev).start, self.span_tok(prev).end) {
                        self.kw("final");
                        self.kw("var");
                        match pattern {
                            Some(p) if self.kind(p) == "ConstantPattern" => {
                                let e = self.children(p).first().copied().unwrap_or(p);
                                self.to_expression(e, false, true);
                            }
                            Some(p) => self.to_expression(p, false, true),
                            None => self.to_expression(n, false, true),
                        }
                    } else {
                        self.kw("as");
                        self.kw("when");
                    }
                }
                Some("as") => self.kw("dynamic"),
                Some("when") => {}
                Some(_) => {
                    self.kw("as");
                    self.kw("when");
                }
            }
        } else {
            self.local("SwitchMember_statement");
            let commands: Vec<usize> = self.children(n).iter().copied().filter(|&k| super::isp::is_command(self.kind(k))).collect();
            if commands.first().is_none_or(|&c| self.offset <= self.start(c)) {
                self.kw("case");
                self.kw_and_text("default", ":");
            }
            self.to_command(n);
        }
    }

    pub(super) fn v_switch(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.local("Block_statement");
            self.to_command(n);
            return;
        }
        let opens = self.op_in(Op::LParen, self.span_tok(word).end, self.end(n));
        let closes = opens.and_then(|a| self.pair(a));
        if let (Some(a), Some(f)) = (opens, closes)
            && self.offset >= self.span_tok(a).end
            && self.offset <= self.span_tok(f).start
        {
            self.local("SwitchStatement_expression");
            self.to_expression(n, false, true);
            return;
        }
        let after = closes.map_or(self.span_tok(word).end, |f| self.span_tok(f).end);
        let Some(c) = self.op_in(Op::LBrace, after, self.end(n)) else { return };
        let end = self.pair(c).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if self.offset >= self.span_tok(c).end && self.offset <= end {
            self.local("SwitchMember_statement");
            let members = self.children_of(n, &["SwitchCase", "SwitchDefault", "SwitchPatternCase"]);
            self.kw("case");
            self.kw_and_text("default", ":");
            if !members.is_empty() {
                if !members.iter().any(|&m| self.kind(m) == "SwitchDefault") {
                    self.kw_and_text("default", ":");
                }
                if let Some(m) = self.element_before(&members) {
                    self.to_command(m);
                }
            }
        }
    }

    pub(super) fn v_top_variable(&mut self, n: usize) {
        if self.handled_recovery(n) {
            return;
        }
        if self.offset == self.start(n) {
            self.to_unit_member_before(n);
            return;
        }
        let Some(list_node) = self.child(n, "VariableDeclarationList") else { return };
        let Some(list) = self.real_list(list_node) else { return };
        let Some(first) = list.variables.first() else { return };
        if self.offset > first.name.span.end {
            if list.ty.is_none() {
                let text = self.real_name(first.name).to_string();
                self.ih(true);
                self.type_name_ih(&text);
            }
            return;
        }
        let external = self.word_in("external", self.start(n), first.name.span.start).is_some();
        if !external {
            self.kw("external");
        }
        if !list.late {
            self.kw("late");
        }
        if !list.const_ {
            self.kw("const");
        }
        if !list.final_ {
            self.kw("final");
        }
        if self.parent(n) == Some(0) {
            let (before, _) = self.member_neighbors(n);
            if before.is_none_or(|a| self.is_directive(a)) {
                self.local("CompilationUnit_directive");
            } else {
                self.local("CompilationUnit_declaration");
            }
        }
        self.dh(Flags { ty: true, ..Flags::default() });
        self.lexical_dh(n);
    }

    pub(super) fn v_try(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.local("Block_statement");
            self.to_command(n);
            return;
        }
        let body = self.child(n, "Block");
        if body.is_some_and(|b| self.offset >= self.end(b)) {
            let finally = self.word_in("finally", self.start(n), self.end(n));
            match finally {
                None => {
                    let last = self.children_of(n, &["CatchClause"]).last().copied();
                    match last {
                        None => self.try_clauses_kw(true),
                        Some(c) => self.try_clauses_kw(self.offset >= self.end(c)),
                    }
                }
                Some(f) if self.offset < self.span_tok(f).start => self.try_clauses_kw(false),
                _ => {}
            }
        }
    }

    pub(super) fn v_type_parameter(&mut self, n: usize) {
        if let Some(list) = self.parent(n)
            && self.kind(list) == "TypeParameterList"
        {
            // `Future<void^>` num corpo de classe: o fasta inventa `() {}`.
            let closes = self.tok_before(self.end(list));
            let next = closes.and_then(|f| self.toks.get(f + 1));
            let without_parens = next.is_none_or(|t| t.kind != dartforge_frontend::token::Kind::Op(Op::LParen));
            let ghost_method = self.parent(list).is_some_and(|m| self.kind(m) == "MethodDeclaration" && self.child(m, "FormalParameterList").is_none());
            if without_parens && ghost_method {
                self.local("TypeParameter_bound");
                self.to_type(n, TypeOptions { excluded: vec![list], ..TypeOptions::default() });
                return;
            }
        }
        let Some(name) = self.tok_after(self.start(n)) else { return };
        let name = if self.children(n).iter().any(|&k| self.kind(k) == "Annotation") {
            self.tok_after(self.children(n).iter().filter(|&&k| self.kind(k) == "Annotation").map(|&k| self.end(k)).max().unwrap_or(self.start(n))).unwrap_or(name)
        } else {
            name
        };
        if self.offset <= self.span_tok(name).end {
            return;
        }
        let extends = self.word_in("extends", self.span_tok(name).end, self.end(n));
        match extends {
            None => self.kw("extends"),
            Some(e) if self.offset <= self.span_tok(e).end => self.kw("extends"),
            Some(_) => {
                self.local("TypeParameter_bound");
                self.to_type(n, TypeOptions { non_void: true, ..TypeOptions::default() });
            }
        }
    }

    pub(super) fn v_variable_declaration(&mut self, n: usize) {
        let Some(list_node) = self.parent(n).filter(|&l| self.kind(l) == "VariableDeclarationList") else { return };
        let grandparent = self.parent(list_node);
        let grandparent_kind = grandparent.map(|a| self.kind(a));
        match grandparent_kind {
            Some("FieldDeclaration") => {
                let a = grandparent.unwrap_or(n);
                if self.previous_incomplete_member(a) && self.loose_field(a) {
                    return;
                }
            }
            Some("ForPartsWithDeclarations") => {
                let list = self.real_list(list_node);
                let has_equal = self.real_variable(n).is_some_and(|v| v.initializer.is_some());
                if !has_equal
                    && list.is_some_and(|l| l.variables.len() == 1)
                    && list.and_then(|l| l.ty).is_some_and(|t| matches!(self.a.ty(t).kind, ast::TypeKind::Record { .. }))
                {
                    self.kw("in");
                }
            }
            Some("TopLevelVariableDeclaration") => {
                if self.handled_recovery(grandparent.unwrap_or(n)) {
                    return;
                }
            }
            _ => {}
        }
        let Some(v) = self.real_variable(n) else { return };
        let Some(list) = self.real_list(list_node) else { return };
        if self.offset <= v.name.span.end {
            let container = grandparent.and_then(|a| self.parent(a));
            match list.ty {
                None => {
                    self.local("VariableDeclarationList_type");
                    let word = !(list.var_ || list.final_ || list.const_);
                    if word {
                        self.kw("const");
                        self.kw("final");
                        self.kw("var");
                    }
                    if !list.var_ {
                        self.to_type(n, TypeOptions::default());
                    }
                }
                Some(t) => {
                    let private = matches!(grandparent_kind, Some("FieldDeclaration" | "TopLevelVariableDeclaration"));
                    let node_t = self.children(list_node).iter().copied().find(|&k| self.link(k) == Ligacao::Tipo(t));
                    self.ih(private);
                    self.ih_variable(node_t);
                }
            }
            if grandparent_kind == Some("FieldDeclaration") {
                let a = grandparent.unwrap_or(n);
                self.local("FieldDeclaration_fields");
                let start_a = self.start(a);
                let before_name = |s: &Self, p: &str| s.word_in(p, start_a, v.name.span.start).is_some();
                if !before_name(self, "external") {
                    self.kw("external");
                }
                if !before_name(self, "static") {
                    self.kw("static");
                    if container.is_some_and(|c| matches!(self.kind(c), "ClassDeclaration" | "MixinDeclaration")) {
                        if !before_name(self, "abstract") {
                            self.kw("abstract");
                        }
                        if !before_name(self, "covariant") {
                            self.kw("covariant");
                        }
                    }
                    if !list.late && container.is_none_or(|c| self.kind(c) != "ExtensionDeclaration") {
                        self.kw("late");
                    }
                }
                if self.first_after_annotations(a).is_some_and(|i| self.span_tok(i).start == v.name.span.start) {
                    self.kw("const");
                    if container.is_some_and(|c| self.kind(c) == "ClassDeclaration") {
                        self.kw("factory");
                    }
                    self.kw("get");
                    self.kw("operator");
                    self.kw("set");
                }
                if self.loose_field(a) {
                    match container.map(|c| self.kind(c)) {
                        Some("ClassDeclaration") => self.local("ClassDeclaration_member"),
                        Some("EnumDeclaration") => self.local("EnumDeclaration_member"),
                        Some("ExtensionDeclaration") => self.local("ExtensionDeclaration_member"),
                        Some("MixinDeclaration") => self.local("MixinDeclaration_member"),
                        _ => {}
                    }
                    let class = container.filter(|&c| matches!(self.kind(c), "ClassDeclaration" | "MixinDeclaration")).and_then(|c| self.node_class(c));
                    self.suggest_overrides(class, false);
                }
            } else if grandparent_kind == Some("TopLevelVariableDeclaration") {
                let a = grandparent.unwrap_or(n);
                if self.word_in("external", self.start(a), v.name.span.start).is_none() {
                    self.kw("external");
                }
                if !list.late && container.is_none_or(|c| self.kind(c) != "ExtensionDeclaration") {
                    self.kw("late");
                }
            }
            return;
        }
        if let Some(init) = v.initializer {
            let start = self.a.expr(init).span.start;
            let equal = self.op_in(Op::Assign, v.name.span.end, start.max(v.name.span.end + 1));
            if equal.is_some_and(|i| self.offset >= self.span_tok(i).end) {
                self.local("VariableDeclaration_initializer");
                self.to_expression(n, false, true);
                if let Some(t) = self.local_type(v.name.span.start).or_else(|| list.ty.and_then(|t| {
                    let no = self.children(list_node).iter().copied().find(|&k| self.link(k) == Ligacao::Tipo(t))?;
                    self.type_node_type(no)
                })) && matches!(self.query.table.get(t), Type::Function { .. })
                {
                    let names: Vec<String> = match list.ty.map(|x| &self.a.ty(x).kind) {
                        Some(ast::TypeKind::Function { parameters, .. }) => parameters
                            .iter()
                            .filter(|x| x.kind != ast::ParameterKind::Named)
                            .map(|x| x.name.map(|n| self.real_name(n).to_string()).unwrap_or_default())
                            .collect(),
                        _ => Vec::new(),
                    };
                    self.suggest_closure_with_names(t, &names, false);
                }
            }
        } else if let Some(i) = self.tok_after(v.name.span.end)
            && self.is_op(i, Op::Assign)
            && self.offset >= self.span_tok(i).end
        {
            self.local("VariableDeclaration_initializer");
            self.to_expression(n, false, true);
        }
    }

    pub(super) fn v_variable_list(&mut self, n: usize) {
        let Some(l) = self.real_list(n) else { return };
        let Some(first) = l.variables.first() else { return };
        if self.offset <= first.name.span.end {
            let type_covers = l.ty.is_some_and(|t| {
                let s = self.a.ty(t).span;
                self.covers(s.start, s.end)
            });
            if (l.ty.is_none() || type_covers) && !l.var_ {
                self.local("VariableDeclarationList_type");
                self.to_type(n, TypeOptions::default());
            } else if l.ty.is_some_and(|t| matches!(self.a.ty(t).kind, ast::TypeKind::Record { .. })) {
                self.kw("in");
            }
        }
    }

    pub(super) fn v_variables_statement(&mut self, n: usize) {
        self.local("Block_statement");
        if self.previous_incomplete_command(n) {
            return;
        }
        if let Initial::Tok(i) = self.initial(n)
            && self.offset <= self.span_tok(i).end
        {
            self.to_command(n);
        } else if self.offset >= self.end(n)
            && let Some(p) = self.parent(n)
        {
            self.to_command(p);
        }
    }

    pub(super) fn v_while(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.local("Block_statement");
            self.to_command(n);
            return;
        }
        let Some(a) = self.op_in(Op::LParen, self.span_tok(word).end, self.end(n)) else { return };
        let closes = self.pair(a).map_or(self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if self.span_tok(a).end <= self.offset && self.offset <= closes {
            let condition = self.children(n).first().copied();
            let ok = match condition {
                None => true,
                Some(c) => self.offset <= self.start(c) || self.offset == self.end(c),
            };
            if ok {
                self.local("WhileStatement_condition");
                self.to_expression(n, false, true);
            }
        }
    }

    pub(super) fn v_with(&mut self, n: usize) {
        let parent = self.parent(n);
        let Some(w) = self.tok_after(self.start(n)) else { return };
        if self.offset <= self.span_tok(w).start && parent.is_some_and(|p| self.kind(p) == "ClassDeclaration") {
            if let Some(p) = parent {
                self.class_declaration_kw(p);
            }
        } else if self.offset <= self.span_tok(w).end {
            self.kw("with");
        } else {
            self.local("WithClause_mixinType");
            self.to_type(n, TypeOptions { mixable: true, ..TypeOptions::default() });
        }
    }

    pub(super) fn v_yield(&mut self, n: usize) {
        let Some(word) = self.tok_after(self.start(n)) else { return };
        if self.offset <= self.span_tok(word).end {
            self.local("Block_statement");
            self.kw("yield");
            return;
        }
        let pv = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::Semicolon));
        if pv.is_none_or(|p| self.offset <= self.span_tok(p).end) {
            self.local("YieldStatement_expression");
            self.to_expression(n, false, false);
        }
    }
}

/// Os lexemas de palavra-chave do analyzer (exportado para as visitas).
pub(super) const ANALYZER_PUB_WORDS: &[&str] = super::isp::ANALYZER_WORDS;
