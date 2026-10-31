//! State of a body being inferred: locals, lexical scopes, visible type
//! parameters, function stack (for `return`/`yield`), cascade receivers and
//! the current flow model.

use super::BodyInferrer;
use super::flow::Flow;
use crate::resolved::LocalId;
use crate::table::{TypeId, TypeParamId};
use dartforge_elements::model::{
    ClassId, ExtensionId, FunctionElementId, LibraryId, UnitId, VariableId,
};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// A local variable (including parameters, pattern variables and local functions).
#[derive(Debug, Clone)]
pub(crate) struct Local {
    pub name: SymbolId,
    /// Declared type (written or inferred from the initializer).
    pub ty: TypeId,
    pub final_: bool,
    pub late: bool,
    pub const_: bool,
    /// Offset of the name in the declaration.
    pub offset: usize,
    /// Type not yet known (`var x;` without an initializer has `dynamic`, but a
    /// local function being inferred is not available).
    pub local_function: bool,
}

/// What a name resolves to in a block scope.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MemberName {
    Local(LocalId),
    TypeParam(TypeParamId),
    /// A local declared further ahead in the same block: the block's scope
    /// already contains it, and using it before the declaration is an error. The
    /// range of the name in the declaration.
    Forward(dartforge_diagnostics::Span),
}

/// Context of the function (or function expression) enclosing the current
/// point.
#[derive(Debug, Clone)]
pub(crate) struct FunctionCtx {
    pub modifier: ast::AsyncModifier,
    /// The `imposedType` of a function literal's `BodyInferenceContext`: the
    /// return of the context's function type (except `dynamic`/`_`), against
    /// which `yield` is checked (`function_expression_resolver.dart:37-48`).
    pub closure_imposed: Option<TypeId>,
    /// Declared return type (or imposed by the context); `None` when the return
    /// is being inferred.
    pub the_return: Option<TypeId>,
    /// Context schema for the `return`/`yield` expressions.
    pub return_context: TypeId,
    /// Types of the returned (or yielded) expressions, for inference.
    pub returned: Vec<TypeId>,
    /// There is a `return;` without a value.
    pub empty_return: bool,
    /// The `return` keywords of a closure's valueless `return;`s, for
    /// `_checkReturnWithoutValue` after its return type is inferred.
    pub valueless_returns: Vec<dartforge_diagnostics::Span>,
    /// The expressions of `return e;` (and the `=> e` body) with their types,
    /// for checking a closure whose return ends up being the context's.
    pub returned_expressions: Vec<(ast::ExprId, TypeId)>,
    /// The executable that declares the return, for `return_of_invalid_type`;
    /// `None` in closures and generative constructors (other rules).
    pub executable: Option<Executable>,
    /// `hasLegalReturnType`: false after `illegal_*_return_type`, which silences
    /// the check of the returned expressions.
    pub legal_return: bool,
}

/// Kind and display name of the executable (the analyzer's
/// `EnclosingExecutableContext`): `return_of_invalid_type` says "function",
/// "method" or "constructor" and the name.
#[derive(Debug, Clone)]
pub(crate) struct Executable {
    pub kind: ExecutableKind,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutableKind {
    /// Named top-level or local function, and getters/setters (top-level or
    /// class ones): a `PropertyAccessorElement` counts as a function.
    Function,
    /// Method or operator of a class or extension.
    Method,
    /// Factory constructor.
    Constructor,
}

/// The scrutinee of the `switch` in progress: the expression, the value's
/// promotion key, the write version of the scrutinized variable and the
/// promotion target the first case resolved.
pub(crate) type SwitchScrutinee = (ast::ExprId, LocalId, Option<u32>, Option<LocalId>);

/// Inference state of a body.
pub(crate) struct Body {
    pub unit: UnitId,
    pub lib: LibraryId,
    pub class: Option<ClassId>,
    pub extension: Option<ExtensionId>,
    /// Without `this` (static member, top level, factory constructor...).
    pub the_static: bool,
    pub locals: Vec<Local>,
    /// Declarations of local functions (distinct from final variables with a function type).
    pub local_functions: HashSet<LocalId>,
    pub scopes: Vec<Vec<(SymbolId, MemberName)>>,
    pub flow: Flow,
    pub functions: Vec<FunctionCtx>,
    pub cascades: Vec<TypeId>,
    /// The target of each open cascade (the sections' `realTarget`).
    pub cascade_targets: Vec<ast::ExprId>,
    /// The promotion base of each cascade target and its version at the start
    /// of the cascade: the target lives in an implicit temporary with the
    /// expression's SSA node (`cascadeExpression_afterTarget`), which a write to
    /// the variable during the sections does not change.
    pub cascade_bases: Vec<Option<(Base, u32)>>,
    /// In the initializer of a non-`late` instance field or of a static variable of a
    /// class or extension (`_isInInstanceNotLateVariableDeclaration`,
    /// `_isInStaticVariableDeclaration`): an instance member without `this` is
    /// `IMPLICIT_THIS_REFERENCE_IN_INITIALIZER`.
    pub variable_initializer: bool,
    /// The forced version of a base while a property is read through the
    /// cascade target ([`Body::base_version`]).
    pub forced_version: Option<(Base, u32)>,
    /// Typing the pattern of a `case`/`if-case` (refutable): a bare identifier
    /// is a constant (`case _padrao:`), not a new variable.
    pub refutable_pattern: bool,
    /// A refutable pattern already reported in an irrefutable context
    /// (`refutablePatternInIrrefutableContext`): what is below it is analyzed as
    /// refutable (`context.makeRefutable()`), without reporting again or
    /// checking the required type.
    pub forced_refutable: bool,
    /// The integer literal operand of a unary `-` being analyzed
    /// (`IntegerLiteral.immediatelyNegated`).
    pub negated_literal: Option<ast::ExprId>,
    /// The variables of the guarded pattern whose `when` clause is being
    /// analyzed (`isVisitingWhenClause`): writing to them is
    /// `PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD`.
    pub variables_in_guard: Vec<LocalId>,
    /// The variables of the current guarded pattern (name and declaration), which
    /// stay hidden during the whole pattern (`HiddenElements.forGuardedPattern`,
    /// `error_verifier.dart:6570-6575`): reading them inside the pattern is
    /// `REFERENCED_BEFORE_DECLARATION`.
    pub pattern_hidden: Vec<(SymbolId, dartforge_diagnostics::Span)>,
    /// The locals declared by the pattern of the last `case` (without those of
    /// the guard), for the join variables of cases that share the body.
    pub last_pattern_locals: std::ops::Range<usize>,
    /// Inconsistent join variables of cases that share a body, with the code that
    /// each reference reports (`finishJoinedPatternVariable`).
    pub inconsistent_joins: HashMap<LocalId, dartforge_diagnostics::Code>,
    /// The pattern match in progress (T6: the matched value's reference and the
    /// "did not match" state); `None` outside a pattern.
    pub matching: Option<super::patterns::Matching>,
    /// Inside a `switch` (statement or expression): the scrutinee already
    /// matched by a case (the expression, the value's promotion key, the
    /// write version of the scrutinized variable and the promotion target the
    /// first case resolved), which the following cases reuse. `None` outside a
    /// `switch`; `Some(None)` before the first case.
    pub switch_scrutinee: Option<Option<SwitchScrutinee>>,
    /// Body of a static method or static field initializer: the class's type
    /// parameters do not apply there (`TYPE_PARAMETER_REFERENCED_BY_STATIC`).
    pub static_member: bool,
    /// Stack of `break`/`continue` targets (labels and loops): flow models
    /// accumulated at the jumps.
    pub jumps: Vec<JumpTarget>,
    /// Type of `this` (class, mixin, enum, extension type or the extension's `on`).
    pub this_type: Option<TypeId>,
    /// Names written inside closures (write capture, which prevents promotion).
    pub written_in_closure: Vec<super::statements::Write>,
    /// Labels of the labeled statement whose body is the next loop/`switch`.
    pub pending_labels: Vec<SymbolId>,
    /// The symbol `_` when the library has wildcards (Dart 3.7): declaring `_`
    /// creates the local (it has a `LocalId` and a type) but does not bind the
    /// name.
    pub wildcard: Option<SymbolId>,
    /// Dot shorthands (3.10): the context of the selector chain, by the root's
    /// `DotShorthand` expression (see `atalhos`).
    pub shorthand_context: HashMap<u32, TypeId>,
    /// `DotShorthand` roots of an invocation whose error was already reported
    /// (`shorthands::construction`): the target's [`super::shorthands::value_`] does not report again.
    pub reported_shorthands: std::collections::HashSet<u32>,
    /// Names written anywhere in the top-level body: inside a closure they do
    /// not stay promoted (`functionExpression_begin` does the conservative join
    /// with `assignedVariables.anywhere`).
    /// Computed at the first closure (most bodies have none).
    /// `(written_ones outside de literals, written_ones dentro de literals)` of the member.
    pub written_in_body: Option<(Vec<super::statements::Write>, Vec<super::statements::Write>)>,
    /// The top-level body, for computing `written_in_body` on demand.
    pub root: Root,
    /// Promotable properties (Dart 3.2) already referenced:
    /// `(base, write version of the base, name) -> synthetic local` that carries
    /// the property's flow model (the `_promotableProperties` of the base's SSA
    /// node).
    pub fields: HashMap<(Base, u32, SymbolId), LocalId>,
    /// Non-promotable properties: the generations (one per read that became a
    /// promotion target), from oldest to newest (the `_nonPromotableProperties`
    /// with the `previousSsaNode` of each node).
    pub generations: HashMap<(Base, u32, SymbolId), Vec<LocalId>>,
    /// The generation of each read of a non-promotable property.
    pub read_generation: HashMap<ast::ExprId, LocalId>,
    /// The synthetic local of `this` (the `thisPromotionKey`), which keeps the
    /// promotions that `this` never uses (only why-not-promoted sees them).
    pub local_this: Option<LocalId>,
    /// Flows from before each `?.` of the chains in progress.
    pub chains: Vec<super::flow::Flow>,
    /// Condition variables (§7.10): `(true_, false_)` of the value written to
    /// the variable, by the write's version, restored when it is read as a
    /// condition with that version. Every write stays: a write on a branch
    /// that does not complete must not hide the one the join keeps.
    pub conditions: HashMap<(LocalId, u32), (Flow, Flow)>,
    /// Explicit extension overrides (`E(x)`, R-EXT-02): the call `E(x)` → the
    /// extension and its type arguments.
    pub overlays: HashMap<ast::ExprId, (ExtensionId, Vec<TypeId>)>,
    /// Ends of the basic blocks in progress (the `flowEnd` of
    /// `NullSafetyDeadCodeVerifier`: function body, `if` branch, loop body,
    /// `try`/`catch`), already trimmed at the block's last statement.
    pub flow_ends: Vec<usize>,
    /// The end of the last statement of each block in progress: without an open
    /// basic block, it is where the dead fragment of an expression ends (the
    /// body's `flowEnd`, trimmed at the last statement).
    pub block_ends: Vec<usize>,
    /// The parameters of the primary constructor as seen in its initializers and
    /// in those of non-`late` fields (the `constructorInitializers` and
    /// `instanceFieldDeclaration` `_thisContext` of the 3.13 `ErrorVerifier`):
    /// writing to one of them there is
    /// `ASSIGNMENT_TO_PRIMARY_CONSTRUCTOR_PARAMETER`.
    pub primary_parameters: HashSet<LocalId>,
    /// `this` without the declaration's type: the header of a primary
    /// constructor (3.13, outside the class body).
    pub untyped_this: bool,
    /// Depth in `flow_ends` where the dead fragment in progress started
    /// (`_firstDeadNode`): while there is one, another unreachable node is part
    /// of the same fragment.
    pub dead_code: Option<usize>,
    /// Where the dead fragment in progress started: the statement itself
    /// (unreachable branch) or the block that contains it. A `for` reports the
    /// updates when it is its body (`_reportForUpdaters`).
    pub dead_origin: Option<ast::StmtId>,
    /// An annotation of a class or extension member: the error of an
    /// instance member read without `this` there (the `ErrorVerifier` visits
    /// the metadata with the member's own context: static method, factory,
    /// field that is static or not `late`).
    pub annotation_access: Option<dartforge_diagnostics::Code>,
}

/// Base of a reference to a promotable field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Base {
    This,
    /// `super` (the `_superSsaNode`: its properties are different from those of
    /// `this`).
    Super,
    Local(LocalId),
}

/// Top-level body being inferred.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Root {
    Nothing,
    Function(ast::FunctionId),
    Constructor(ast::MemberId),
}

/// Jump destination.
pub(crate) struct JumpTarget {
    pub labels: Vec<SymbolId>,
    /// It is a loop or a `switch` (target of an unlabeled `break`/`continue`).
    pub the_loop: bool,
    pub is_switch: bool,
    pub breaks: Vec<Flow>,
    pub continues: Vec<Flow>,
}

impl Body {
    pub fn new(
        inf: &mut BodyInferrer<'_>,
        unit: UnitId,
        class: Option<ClassId>,
        extension: Option<ExtensionId>,
        static_: bool,
    ) -> Self {
        let lib = inf.program.unit(unit).library;
        inf.current_unit = Some(unit);
        let mut cx = Body {
            unit,
            lib,
            class,
            extension,
            the_static: static_,
            locals: Vec::new(),
            local_functions: HashSet::new(),
            scopes: vec![Vec::new()],
            flow: Flow::reachable(),
            functions: Vec::new(),
            cascades: Vec::new(),
            cascade_targets: Vec::new(),
            cascade_bases: Vec::new(),
            variable_initializer: false,
            forced_version: None,
            refutable_pattern: false,
            forced_refutable: false,
            negated_literal: None,
            variables_in_guard: Vec::new(),
            pattern_hidden: Vec::new(),
            last_pattern_locals: 0..0,
            inconsistent_joins: HashMap::new(),
            matching: None,
            switch_scrutinee: None,
            static_member: false,
            jumps: Vec::new(),
            this_type: None,
            written_in_closure: Vec::new(),
            pending_labels: Vec::new(),
            wildcard: None,
            shorthand_context: HashMap::new(),
            reported_shorthands: std::collections::HashSet::new(),
            written_in_body: None,
            root: Root::Nothing,
            fields: HashMap::new(),
            generations: HashMap::new(),
            read_generation: HashMap::new(),
            local_this: None,
            chains: Vec::new(),
            conditions: HashMap::new(),
            overlays: HashMap::new(),
            flow_ends: Vec::new(),
            block_ends: Vec::new(),
            primary_parameters: HashSet::new(),
            untyped_this: false,
            dead_code: None,
            dead_origin: None,
            annotation_access: None,
        };
        if inf
            .program
            .library(lib)
            .features
            .has(dartforge_frontend::Feature::WildcardVariables)
        {
            cx.wildcard = inf.interner.lookup("_");
        }
        // The class/extension's type parameters are always in scope (even in
        // static members, where using them is an error).
        if let Some(c) = class {
            let ps = inf.outline.classes[c.0 as usize].type_params.clone();
            for p in ps.iter() {
                let name = inf.table.param(*p).name;
                cx.scopes[0].push((name, MemberName::TypeParam(*p)));
            }
            if !static_ {
                cx.this_type = Some(inf.class_this_type(c));
            }
        } else if let Some(e) = extension {
            let ps = inf.outline.extensions[e.0 as usize].type_params.clone();
            for p in ps.iter() {
                let name = inf.table.param(*p).name;
                cx.scopes[0].push((name, MemberName::TypeParam(*p)));
            }
            if !static_ {
                cx.this_type = Some(inf.outline.extensions[e.0 as usize].on);
            }
        }
        cx.scopes.push(Vec::new());
        cx
    }

    /// Body for the initializer of a top-level variable or a field.
    pub fn to_variable(inf: &mut BodyInferrer<'_>, vid: VariableId, unit: UnitId) -> Self {
        let v = inf.program.variable(vid);
        let (class, extension) = (v.class, v.extension);
        // Instance field initializers do not see `this` (only the `late` ones do).
        let static_ = v.static_ || (class.is_none() && extension.is_none()) || !v.late;
        let mut cx = Body::new(inf, unit, class, extension, static_);
        cx.static_member = v.static_ && (class.is_some() || extension.is_some());
        cx.variable_initializer =
            (class.is_some() || extension.is_some()) && (v.static_ || !v.late);
        if !v.static_ && v.late {
            cx.the_static = false;
            if let Some(c) = class {
                cx.this_type = Some(inf.class_this_type(c));
            }
        }
        // A non-`late` instance field sees the primary constructor's
        // parameters.
        if !v.static_
            && !v.late
            && let Some(c) = class
        {
            inf.declare_primary_parameters(&mut cx, c);
        }
        cx
    }

    /// Body of a declared function (method, top-level function, constructor).
    pub fn to_function(inf: &mut BodyInferrer<'_>, f: FunctionElementId, unit: UnitId) -> Self {
        let fe = inf.program.function(f);
        let (class, extension) = (fe.class, fe.extension);
        let static_ = fe.static_
            && !matches!(
                fe.kind,
                dartforge_elements::model::FunctionKind::Constructor
            )
            || fe.factory;
        let static_ = static_ || (class.is_none() && extension.is_none());
        let static_member = fe.static_
            && !fe.factory
            && !matches!(
                fe.kind,
                dartforge_elements::model::FunctionKind::Constructor
            )
            && (class.is_some() || extension.is_some());
        let mut cx = Body::new(inf, unit, class, extension, static_);
        cx.static_member = static_member;
        cx
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Declares a local in the innermost scope.
    pub fn declare(&mut self, local: Local) -> LocalId {
        let id = LocalId(self.locals.len() as u32);
        let name = local.name;
        self.locals.push(local);
        if self.wildcard == Some(name) {
            self.flow.declare(id);
            return id;
        }
        if let Some(e) = self.scopes.last_mut() {
            e.push((name, MemberName::Local(id)));
        }
        self.flow.declare(id);
        id
    }

    /// Records a name declared further ahead in the current block.
    pub fn declare_forward(&mut self, name: SymbolId, span: dartforge_diagnostics::Span) {
        if self.wildcard == Some(name) {
            return;
        }
        if let Some(e) = self.scopes.last_mut() {
            e.push((name, MemberName::Forward(span)));
        }
    }

    /// Synthetic local (without a name in scope): the promotion target of a
    /// field.
    pub fn declare_synthetic(&mut self, local: Local) -> LocalId {
        let id = LocalId(self.locals.len() as u32);
        self.locals.push(local);
        self.flow.declare(id);
        self.flow.initialize(id);
        id
    }

    /// Forgets the promoted fields of a reassigned local.
    pub fn forget_fields_of(&mut self, base: LocalId) {
        // In a cascade on this local, the target's temporary keeps the old SSA
        // node: the fields of the previous version are kept for the sections.
        if self
            .cascade_bases
            .iter()
            .any(|x| matches!(x, Some((Base::Local(b), _)) if *b == base))
        {
            let current = self.base_version(Base::Local(base));
            let stored: Vec<u32> = self
                .cascade_bases
                .iter()
                .filter_map(|x| match x {
                    Some((Base::Local(b), v)) if *b == base => Some(*v),
                    _ => None,
                })
                .collect();
            self.fields.retain(|(b, v, _), _| {
                *b != Base::Local(base) || (stored.contains(v) && *v != current)
            });
            return;
        }
        self.fields.retain(|(b, _, _), _| *b != Base::Local(base));
    }

    /// The local is the synthetic one of a property (stable or a generation).
    pub fn is_property(&self, id: LocalId) -> bool {
        self.fields.values().any(|&v| v == id) || self.read_generation.values().any(|&v| v == id)
    }

    /// `infoFor`: a synthetic local whose model was lost at a join (created in
    /// only one of the branches) comes back as new.
    pub fn ensure_model(&mut self, id: LocalId) {
        if self.flow.model(id).is_none() {
            self.flow.declare(id);
            self.flow.initialize(id);
        }
    }

    /// The write version of a property's base: the local's (its SSA node);
    /// `this`, `super` and stable properties do not change.
    pub fn base_version(&self, base: Base) -> u32 {
        if let Some((b, v)) = self.forced_version
            && b == base
        {
            return v;
        }
        match base {
            Base::Local(id) if !self.is_property(id) => self.flow.version(id).unwrap_or(0),
            _ => 0,
        }
    }

    pub fn declare_type_param(&mut self, name: SymbolId, p: TypeParamId) {
        if let Some(e) = self.scopes.last_mut() {
            e.push((name, MemberName::TypeParam(p)));
        }
    }

    /// A name in the block scopes (from the innermost outwards).
    pub fn lookup(&self, name: SymbolId) -> Option<MemberName> {
        for e in self.scopes.iter().rev() {
            for (n, r) in e.iter().rev() {
                if *n == name {
                    return Some(*r);
                }
            }
        }
        None
    }

    pub fn local(&self, id: LocalId) -> &Local {
        &self.locals[id.0 as usize]
    }

    /// The visible locals (declared or ahead in the block) by name, with the
    /// declaration's name and whether it has not been declared yet (hidden): a written type that names one of them is
    /// `REFERENCED_BEFORE_DECLARATION` (`NamedTypeResolver`,
    /// `named_type_resolver.dart:611-621`).
    pub fn visible_locals(
        &self,
        interner: &dartforge_intern::Interner,
    ) -> HashMap<SymbolId, (dartforge_diagnostics::Span, bool)> {
        let mut m = HashMap::new();
        for e in self.scopes.iter() {
            for (n, r) in e.iter() {
                match r {
                    MemberName::TypeParam(_) => {
                        m.remove(n);
                    }
                    MemberName::Local(id) => {
                        let l = &self.locals[id.0 as usize];
                        let start = l.offset;
                        m.insert(
                            *n,
                            (
                                dartforge_diagnostics::Span {
                                    start,
                                    end: start + interner.resolve(*n).len(),
                                },
                                false,
                            ),
                        );
                    }
                    MemberName::Forward(s) => {
                        m.insert(*n, (*s, true));
                    }
                }
            }
        }
        m
    }

    /// Type parameters visible by name (for resolving annotations).
    pub fn visible_type_parameters(&self) -> HashMap<SymbolId, TypeParamId> {
        let mut m = HashMap::new();
        for e in self.scopes.iter() {
            for (n, r) in e.iter() {
                match r {
                    MemberName::TypeParam(p) => {
                        m.insert(*n, *p);
                    }
                    MemberName::Local(_) => {
                        m.remove(n);
                    }
                    MemberName::Forward(_) => {}
                }
            }
        }
        m
    }

    /// The lexical scope at this point, for the LSP probe: each name once, by
    /// the innermost declaration (the same order as [`Body::lookup`]).
    pub fn visible_scope(&self) -> crate::resolved::ProbedScope {
        let mut seen: HashSet<SymbolId> = HashSet::new();
        let mut output = crate::resolved::ProbedScope {
            class: self.class,
            extension: self.extension,
            the_static: self.the_static,
            this_type: self.this_type,
            library: Some(self.lib),
            ..Default::default()
        };
        for e in self.scopes.iter().rev() {
            for (n, r) in e.iter().rev() {
                if !seen.insert(*n) {
                    continue;
                }
                match r {
                    MemberName::Local(id) => {
                        let l = self.local(*id);
                        output.locals.push(crate::resolved::VisibleLocal {
                            name: l.name,
                            ty: l.ty,
                            offset: l.offset,
                            function: self.local_functions.contains(id),
                        });
                    }
                    MemberName::TypeParam(p) => output.type_parameters.push((*n, *p)),
                    // Declared ahead: it cannot be used here yet.
                    MemberName::Forward(_) => {}
                }
            }
        }
        output
    }
}
