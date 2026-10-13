//! Patterns (P3): matching as a decision tree over the tests the
//! lowering already has (`test_type`, Dart's `==`, comparisons).
//!
//! The semantics is that of the patterns specification (`patterns-feature-
//! specification.md`, "Pattern matching"): each subpattern is tested from
//! left to right; the first one that fails branches to the failure
//! block; the variables are bound as matching proceeds; `||`
//! tries the second side only if the first fails; a constant pattern matches if
//! `constant == value` (the constant's `==`); a relational one is `value op
//! operand`; a list one checks the exact length (or the minimum, with `...`);
//! a map one checks `containsKey` before reading; an object one tests the type and
//! reads the getters by name.
//!
//! The statements and expressions with patterns (`switch`, `if-case`, declaration and
//! assignment by pattern, `for-in` with a pattern) are at the end.

use super::fn_builder::FnBuilder;
use super::locals::Mode;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind};
use dartforge_frontend::ast::{
    self, BinaryOp, ExprId, ExprKind, PatternId, PatternKind, StmtId,
};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::MemberRef;
use std::collections::HashSet;

/// How a pattern binds the names.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Linkage {
    /// Declaration pattern (`var (a, b) = …`, `case`): declares.
    Declare,
    /// Assignment pattern (`(a, b) = …`): writes into existing variables.
    Assign,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Continues ahead if `cond` (an `i1`), otherwise branches to `failure`.
    fn require(&mut self, cond: Operand, failure: BlockId) {
        let cond = self.to_bool(cond);
        let ok = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond,
            then_block: ok,
            else_block: failure,
        });
        self.set_block(ok);
    }

    /// Binds the name of a variable pattern to the value.
    fn link_default(
        &mut self,
        name_str: ast::Name,
        val_ref: Operand,
        linkage: Linkage,
        linked: &mut HashSet<SymbolId>,
        span: Span,
    ) {
        if linkage == Linkage::Assign || linked.contains(&name_str.sym) {
            if self.store_local(name_str.sym, val_ref.clone()).is_none() {
                // Assignment by pattern to a name that is not a local: through the
                // lexical scope (field, global).
                match self.resolve_by_name(name_str.sym) {
                    Some(dartforge_types::resolved::Resolved::Element(Element::Variable(vid))) => {
                        self.store_global(vid, val_ref, span);
                    }
                    Some(dartforge_types::resolved::Resolved::Member {
                        member: dartforge_types::resolved::MemberRef::Variable(vid),
                        ..
                    }) if !super::is_global(self.ctx, vid) => {
                        if let Some(this) = self.this_param.clone() {
                            self.store_field(this, vid, val_ref, span);
                        }
                    }
                    _ => {
                        let n = self.ctx.symbol_name(name_str.sym).to_string();
                        self.unsupported(&format!("atribuição por padrão a `{n}`"), span);
                    }
                }
            }
            return;
        }
        let ty = self.local_repr_of(name_str.span.start as usize);
        self.declare_variable(name_str.sym, name_str.span.start as usize, ty, val_ref);
        linked.insert(name_str.sym);
    }

    /// The program class named by an annotated type, if it is one.
    fn type_class(&self, ty: &ast::TypeAnnotation) -> Option<ClassId> {
        let ast::TypeKind::Named { name, .. } = &ty.kind else {
            return None;
        };
        let last_one = name.last()?;
        let lib = self.ctx.program.unit(self.unit_id).library;
        let binding = match &name[..] {
            [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
            _ => self.ctx.program.lookup(lib, last_one.sym),
        };
        match binding?.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        }
    }

    /// Reads member `name` of `value` for an object pattern: through the class
    /// of the pattern when it belongs to the program, otherwise through dynamic dispatch.
    fn read_pattern_member(
        &mut self,
        val_ref: Operand,
        class: Option<ClassId>,
        name_str: SymbolId,
        origin: ExprId,
        span: Span,
    ) -> Operand {
        let text = self.ctx.symbol_name(name_str).to_string();
        // Extension type: its member, statically (`extension_types.rs`);
        // what it does not declare is the representation's, through dispatch.
        if let Some(c) = class.filter(|&c| self.ctx.is_extension_type(c))
            && let Some(r) = self.member_te_in_class(c, name_str)
            && let Some(m) = self.resolved_member_te(Some(&r))
        {
            // The getter of a generic type receives the tuple of the annotation.
            if let (MemberRef::Function(f), Some(tuple)) = (m, self.pattern_tuple_te.clone())
                && !self.is_representation(m)
                && self.ctx.program.functions[f.0 as usize].kind == FunctionKind::Getter
            {
                let fid = f.0 as usize;
                let r = self.static_this_repr(fid).unwrap_or(Type::Ref);
                let recv = self.coerce(val_ref, r);
                let saved = self.armed_tuple.replace(tuple);
                let x = self.call_direct(fid, Some(recv), Vec::new());
                self.armed_tuple = saved;
                return x;
            }
            return self.read_member_te(val_ref, m, None, span);
        }
        if let Some(c) = class
            && self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
            && !self.ctx.is_extension_type(c)
        {
            // `index`/`name` of an enum.
            if let Some(op) = self.enum_member(c, &text, val_ref.clone()) {
                return op;
            }
            for k in crate::lower::members::linearization(self.ctx, c) {
                let cl = &self.ctx.program.classes[k.0 as usize];
                if !self.ctx.compiled_library(cl.library) {
                    break;
                }
                if let Some(&f) = cl.instance_members.get(&name_str) {
                    let fid = f.0 as usize;
                    if let Some(vid) = self.ctx.program.functions[fid].variable {
                        return self.read_field_with_late(val_ref, vid, span);
                    }
                    if self.ctx.program.functions[fid].kind == FunctionKind::Getter {
                        return self.call_member(val_ref, fid, &[], span);
                    }
                    return self.method_tearoff(val_ref, fid, span);
                }
                if let Some(&v) = cl.fields.iter().find(|&&v| {
                    let var = &self.ctx.program.variables[v.0 as usize];
                    !var.static_ && var.name == name_str
                }) {
                    return self.read_field_with_late(val_ref, v, span);
                }
            }
        }
        let r = self.property_by_selector(val_ref, &text, origin, span);
        self.coerce(r, Type::Ref)
    }

    /// Matches `value` against pattern `p`; on failure branches to `failure`. `origin`
    /// is the expression of the matched value (for SDK members by name).
    pub fn match_pattern(
        &mut self,
        ast: &ast::Ast,
        p: PatternId,
        val_ref: Operand,
        failure: BlockId,
        linkage: Linkage,
        linked: &mut HashSet<SymbolId>,
        origin: ExprId,
    ) {
        let pat = ast.pattern(p);
        let span = pat.span;
        // `<int>[…]`/`<K, V>{…}`: the value must be a `List<int>`/`Map<K, V>`
        // (RTI); after that, the matching of the elements is the usual one.
        if let PatternKind::List { type_args, .. } | PatternKind::Map { type_args, .. } = &pat.kind
            && !type_args.is_empty()
        {
            let class = match &pat.kind {
                PatternKind::List { .. } => self.ctx.core.list_class,
                _ => self.ctx.core.map_class,
            };
            let mut r = super::rti::Recipe { text: String::new(), variables: false };
            let mut ok = class.is_some();
            if let Some(c) = class {
                r.text.push_str(&format!("C{}<", self.ctx.id_rti(c)));
                for (i, a) in type_args.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    match self.annotation_recipe(ast.ty(*a)) {
                        Some(x) => {
                            r.text.push_str(&x.text);
                            r.variables |= x.variables;
                        }
                        None => ok = false,
                    }
                }
                r.text.push('>');
            }
            if !ok {
                self.unsupported("padrão de coleção com argumento de tipo não resolvido", span);
                return;
            }
            let t = self.recipe_rti(&r);
            let v = self.coerce(val_ref.clone(), Type::Ref);
            let e = self.test_rti(v, t);
            self.require(e, failure);
        }
        match &pat.kind {
            PatternKind::Wildcard { ty } => {
                if let Some(t) = ty {
                    let v = self.coerce(val_ref, Type::Ref);
                    let ok = self.test_type(ast.ty(*t), v);
                    self.require(ok, failure);
                }
            }
            PatternKind::Variable {
                ty: None,
                name,
                var_: false,
                final_: false,
            } if self.refutable_pattern && linkage == Linkage::Declare && self.ctx.symbol_name(name.sym) != "_" => {
                // In a matching pattern, a bare name is a constant
                // pattern (patterns specification, "Constant patterns"):
                // it matches if `constant == value`.
                let c = match self.read_local_by_name(name.sym) {
                    Some(v) => v,
                    None => match self.read_unresolved_name(name.sym, span) {
                        Some(v) => v,
                        None => {
                            let n = self.ctx.symbol_name(name.sym).to_string();
                            self.unsupported(&format!("padrão constante `{n}`"), span)
                        }
                    },
                };
                let ok = self.operate(BinaryOp::Eq, c, val_ref, false, span);
                self.require(ok, failure);
            }
            PatternKind::Variable { ty, name, .. } => {
                if let Some(t) = ty {
                    let v = self.coerce(val_ref.clone(), Type::Ref);
                    let ok = self.test_type(ast.ty(*t), v);
                    self.require(ok, failure);
                }
                self.link_default(*name, val_ref, linkage, linked, span);
            }
            PatternKind::Constant(e) => {
                // `const (e)`: a parenthesized constant expression (the
                // parser reads it as a one-field record; without the trailing comma
                // it is just `e`).
                let e = match &ast.expr(*e).kind {
                    ExprKind::Record { positional, named, .. } if positional.len() == 1 && named.is_empty() => {
                        let s = ast.expr(*e).span;
                        let text = &self.source()[s.start as usize..s.end as usize];
                        if text.trim_end().trim_end_matches(')').trim_end().ends_with(',') {
                            e
                        } else {
                            &positional[0]
                        }
                    }
                    _ => e,
                };
                let c = self.lower_expr(ast, *e);
                let ok = self.operate(BinaryOp::Eq, c, val_ref, false, span);
                self.require(ok, failure);
            }
            PatternKind::Relational { op, value } => {
                let c = self.lower_expr(ast, *value);
                let ok = match op {
                    BinaryOp::Eq | BinaryOp::NotEq => self.operate(*op, val_ref, c, false, span),
                    _ => {
                        let v = self.coerce(val_ref, Type::Ref);
                        let c = self.coerce(c, Type::Ref);
                        self.operate_dynamic(*op, v, c, span)
                    }
                };
                self.require(ok, failure);
            }
            PatternKind::Or(a, b) => {
                let try_b = self.new_block();
                let end = self.new_block();
                self.match_pattern(ast, *a, val_ref.clone(), try_b, linkage, linked, origin);
                self.terminate(Terminator::Branch(end));
                self.set_block(try_b);
                self.match_pattern(ast, *b, val_ref, failure, linkage, linked, origin);
                self.terminate(Terminator::Branch(end));
                self.set_block(end);
            }
            PatternKind::And(a, b) => {
                self.match_pattern(ast, *a, val_ref.clone(), failure, linkage, linked, origin);
                self.match_pattern(ast, *b, val_ref, failure, linkage, linked, origin);
            }
            PatternKind::Parenthesized(x) => self.match_pattern(ast, *x, val_ref, failure, linkage, linked, origin),
            PatternKind::NullCheck(x) => {
                if self.operand_type(&val_ref) == Type::Ref {
                    let nn = self.emit(
                        Instruction::ICmp(ICmpOp::Ne, val_ref.clone(), Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    );
                    self.require(nn, failure);
                }
                self.match_pattern(ast, *x, val_ref, failure, linkage, linked, origin);
            }
            PatternKind::NullAssert(x) => {
                if self.operand_type(&val_ref) == Type::Ref {
                    let null = self.emit(
                        Instruction::ICmp(ICmpOp::Eq, val_ref.clone(), Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    );
                    let b_error = self.new_block();
                    let b_ok = self.new_block();
                    self.terminate(Terminator::CondBranch {
                        cond: null,
                        then_block: b_error,
                        else_block: b_ok,
                    });
                    self.set_block(b_error);
                    let e = self.new_runtime_error("dartforge_type_error_new", Vec::new());
                    self.emit_throw_op(e);
                    self.set_block(b_ok);
                }
                self.match_pattern(ast, *x, val_ref, failure, linkage, linked, origin);
            }
            PatternKind::Cast { pattern, ty } => {
                let v = self.coerce(val_ref.clone(), Type::Ref);
                self.check_type_or_throw(ast.ty(*ty), v, super::rti::CastContext::How);
                self.match_pattern(ast, *pattern, val_ref, failure, linkage, linked, origin);
            }
            // Lists and maps are the classes of the SDK source: through the members
            // (`length`, `[]`, `containsKey`…) of the dynamic class.
            PatternKind::List { elements, .. } => {
                self.match_source_list(ast, elements, val_ref, failure, linkage, linked, origin);
            }
            PatternKind::Map { entries, .. } => {
                self.match_source_map(ast, entries, val_ref, failure, linkage, linked, origin);
            }
            PatternKind::Record { fields } if fields.iter().any(|f| f.name.is_some() || self.implicit_name(ast, f).is_some()) => {
                // A shape with a named field (`records.rs`): the class of the
                // value must be that of the pattern's shape.
                let mut npos = 0usize;
                let mut names = Vec::new();
                let mut field_list = Vec::new();
                for f in fields.iter() {
                    match f.name.map(|n| n.sym).or_else(|| self.implicit_name(ast, f)) {
                        Some(s) => {
                            let n = self.ctx.symbol_name(s).to_string();
                            names.push(n.clone());
                            field_list.push((Some(n), f.pattern, f.name.is_none()));
                        }
                        None => {
                            npos += 1;
                            field_list.push((None, f.pattern, false));
                        }
                    }
                }
                let mut sorted = names.clone();
                sorted.sort();
                let Some(id) = self.ctx.shape_id(npos, &sorted) else {
                    self.unsupported("padrão de record com forma desconhecida", span);
                    return;
                };
                let v = self.coerce(val_ref, Type::Ref);
                let cls = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_value_class".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let ok = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, cls, Operand::Constant(Constant::Int(i64::from(id)))),
                    Type::I1,
                );
                self.require(ok, failure);
                let mut k_pos = 0usize;
                for (name_str, sp, implicit) in field_list {
                    let idx = match name_str {
                        None => {
                            k_pos += 1;
                            k_pos - 1
                        }
                        Some(n) => npos + sorted.iter().position(|x| *x == n).expect("nome"),
                    };
                    let x = self.shape_field(v.clone(), idx);
                    // `:x` declares `x` even in a matching pattern.
                    let saved = self.refutable_pattern;
                    if implicit {
                        self.refutable_pattern = false;
                    }
                    self.match_pattern(ast, sp, x, failure, linkage, linked, origin);
                    self.refutable_pattern = saved;
                }
            }
            PatternKind::Record { fields } => {
                let v = self.coerce(val_ref, Type::Ref);
                let cls = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_value_class".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                // Positional-only record: the `_Record` class (fixed cid,
                // `REFS`); a record with names is of another class (that of its shape).
                let record_cid = i64::from(dartforge_runtime::layout::cid::RECORD);
                let is_rec = self.emit(Instruction::ICmp(ICmpOp::Eq, cls, Operand::Constant(Constant::Int(record_cid))), Type::I1);
                self.require(is_rec, failure);
                let n = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_native_DartForge_record_numFields".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let ok = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, n, Operand::Constant(Constant::Int(fields.len() as i64))),
                    Type::I1,
                );
                self.require(ok, failure);
                for (i, f) in fields.iter().enumerate() {
                    let x = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_native_DartForge_record_fieldAt".to_string(),
                            args: vec![
                                (v.clone(), Type::Ref),
                                (Operand::Constant(Constant::Int(i as i64)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                    self.match_pattern(ast, f.pattern, x, failure, linkage, linked, origin);
                }
            }
            PatternKind::Object { ty, fields } => {
                let t = ast.ty(*ty);
                let v = self.coerce(val_ref, Type::Ref);
                let ok = self.test_type(t, v.clone());
                self.require(ok, failure);
                let class = self.type_class(t);
                // The written type arguments of a generic extension type:
                // the tuple of its getters.
                let tuple_te = match class {
                    Some(c) if self.ctx.is_extension_type(c) && self.generic_class(c) => self.annotation_tuple(t),
                    _ => None,
                };
                let saved_tuple = std::mem::replace(&mut self.pattern_tuple_te, tuple_te);
                for f in fields.iter() {
                    let name_str = match f.name {
                        Some(n) => Some(n.sym),
                        None => self.implicit_name(ast, f),
                    };
                    let Some(name_str) = name_str else {
                        self.unsupported("campo de padrão de objeto sem nome", f.span);
                        continue;
                    };
                    let x = self.read_pattern_member(v.clone(), class, name_str, origin, f.span);
                    // `:x` declares `x` even in a matching pattern.
                    let saved = self.refutable_pattern;
                    if f.name.is_none() {
                        self.refutable_pattern = false;
                    }
                    self.match_pattern(ast, f.pattern, x, failure, linkage, linked, origin);
                    self.refutable_pattern = saved;
                }
                self.pattern_tuple_te = saved_tuple;
            }
        }
    }

    /// The name of a `:x` field (that of the variable pattern inside it).
    fn implicit_name(&self, ast: &ast::Ast, f: &ast::PatternField) -> Option<SymbolId> {
        let mut p = f.pattern;
        loop {
            match &ast.pattern(p).kind {
                PatternKind::Variable { name, .. } => {
                    // `:x` has an empty field name in the text: the `name` of the
                    // field is `None` only in that case when the pattern is a variable
                    // and the text starts with `:`.
                    let source = self.source();
                    let ini = f.span.start as usize;
                    return source[ini..].trim_start().starts_with(':').then_some(name.sym);
                }
                PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Cast { pattern: x, .. } => {
                    p = *x;
                }
                _ => return None,
            }
        }
    }

    // ------------------------------------------------------------------
    // Constructs with patterns

    /// `switch` as a statement: cases in order, `when`, an empty body that falls into the
    /// next one (several `case`s labeling the same body), `default`, `break`
    /// and `continue label` to the label of a `case`.
    pub fn lower_switch_statement(&mut self, ast: &ast::Ast, value: ExprId, cases: &[ast::SwitchCase]) {
        let v = self.lower_expr(ast, value);
        let exit = self.new_block();
        let external_labels = std::mem::take(&mut self.pending_labels);
        for &l in &external_labels {
            self.labeled_break_targets.insert(l, exit);
        }
        // The body block of each case (that of the empty cases is the one of the next
        // non-empty case: they share the body).
        let mut bodies: Vec<BlockId> = cases.iter().map(|_| self.new_block()).collect();
        for i in (0..cases.len()).rev() {
            if cases[i].body.is_empty() && i + 1 < cases.len() {
                bodies[i] = bodies[i + 1];
            }
        }
        // `continue label` to the label of a case goes to its body.
        let mut case_labels = Vec::new();
        for (i, c) in cases.iter().enumerate() {
            for l in c.labels.iter() {
                self.labeled_continue_targets.insert(l.sym, bodies[i]);
                case_labels.push(l.sym);
            }
        }
        self.break_targets.push(exit);
        // Tests, in order; the variables of each case live in its scope.
        let mut case_scopes = Vec::with_capacity(cases.len());
        let mut successes = Vec::with_capacity(cases.len());
        for c in cases.iter() {
            let next_item = self.new_block();
            self.open_scope();
            let mut linked = HashSet::new();
            if let Some(p) = c.pattern {
                let saved = std::mem::replace(&mut self.refutable_pattern, true);
                self.match_pattern(ast, p, v.clone(), next_item, Linkage::Declare, &mut linked, value);
                self.refutable_pattern = saved;
            }
            if let Some(g) = c.guard {
                let ok = self.lower_expr(ast, g);
                self.require(ok, next_item);
            }
            let body_i = bodies[case_scopes.len()];
            let passing = if c.body.is_empty() && case_scopes.len() + 1 < cases.len() {
                Some(self.new_block())
            } else {
                None
            };
            self.terminate(Terminator::Branch(passing.unwrap_or(body_i)));
            successes.push(passing);
            case_scopes.push(self.scopes.pop().expect("case scope"));
            self.set_block(next_item);
        }
        // Nenhum caso casou: sai.
        self.terminate(Terminator::Branch(exit));
        // `case A(:final x): case B(:final x): body` has a single `x`
        // observable in the body. Each test above created its own local; before
        // jumping to the body, the previous case copies its values into the
        // locals of the last case that labels it. Without that, the body reads an
        // uninitialized `alloca` when the first pattern matches.
        for i in 0..cases.len() {
            let Some(passing) = successes[i] else { continue };
            let Some(j) = (i + 1..cases.len()).find(|&j| bodies[j] == bodies[i] && !cases[j].body.is_empty()) else {
                self.set_block(passing);
                self.terminate(Terminator::Branch(bodies[i]));
                continue;
            };
            self.set_block(passing);
            let mut names: Vec<_> = case_scopes[i].keys().copied().collect();
            names.sort_by_key(|&sym| self.ctx.symbol_name(sym).to_string());
            for name_str in names {
                let Some(dest) = case_scopes[j].get(&name_str) else { continue };
                let val_ref = self.read_local(&case_scopes[i][&name_str]);
                let val_ref = self.coerce(val_ref, dest.ty);
                match &dest.mode {
                    Mode::Memory(ptr) => {
                        self.emit(Instruction::Store { ptr: ptr.clone(), val: val_ref }, Type::Void);
                    }
                    Mode::Cell(ptr) => {
                        // The last case did not run its declarator on this
                        // path: create the cell that the captured body uses.
                        let cell = self.emit(Instruction::AllocCell { value: val_ref }, Type::Ref);
                        self.emit(Instruction::Store { ptr: ptr.clone(), val: cell }, Type::Void);
                    }
                    Mode::Value(_) | Mode::Environment { .. } => unreachable!("variable declared by case"),
                }
            }
            self.terminate(Terminator::Branch(bodies[i]));
        }
        // The bodies. The scope of a shared body is that of the last case
        // that labels it; the previous cases wrote into these same locals.
        let mut done_set = HashSet::new();
        for (i, c) in cases.iter().enumerate() {
            if c.body.is_empty() || !done_set.insert(bodies[i]) {
                continue;
            }
            self.set_block(bodies[i]);
            self.scopes.push(case_scopes[i].clone());
            for &s in c.body.iter() {
                self.lower_stmt(ast, s);
            }
            self.close_scope();
            // Dart 3: a non-empty case does not fall into the next one.
            self.terminate(Terminator::Branch(exit));
        }
        // An empty case without a following case (the last one): its body is empty.
        for (i, c) in cases.iter().enumerate() {
            if c.body.is_empty() && !done_set.contains(&bodies[i]) {
                done_set.insert(bodies[i]);
                self.set_block(bodies[i]);
                self.terminate(Terminator::Branch(exit));
            }
        }
        self.break_targets.pop();
        for l in case_labels {
            self.labeled_continue_targets.remove(&l);
        }
        for l in external_labels {
            self.labeled_break_targets.remove(&l);
        }
        self.set_block(exit);
    }

    /// `switch` as an expression: the first case that matches gives the value; none
    /// matching is a run-time error (exhaustiveness belongs to the
    /// front end; here the unreachable path throws `StateError`).
    pub fn lower_switch_expression(
        &mut self,
        ast: &ast::Ast,
        expr_id: ExprId,
        value: ExprId,
        cases: &[ast::SwitchExprCase],
    ) -> Operand {
        let v = self.lower_expr(ast, value);
        let ty = self.expression_repr(expr_id).unwrap_or(Type::Ref);
        let join = self.new_block();
        let mut entries = Vec::new();
        for c in cases.iter() {
            let next_item = self.new_block();
            self.open_scope();
            let mut linked = HashSet::new();
            let saved = std::mem::replace(&mut self.refutable_pattern, true);
            self.match_pattern(ast, c.pattern, v.clone(), next_item, Linkage::Declare, &mut linked, value);
            self.refutable_pattern = saved;
            if let Some(g) = c.guard {
                let ok = self.lower_expr(ast, g);
                self.require(ok, next_item);
            }
            let r = self.lower_expr(ast, c.body);
            if !self.is_terminated() {
                let r = if matches!(self.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    self.coerce(r, ty)
                };
                if !self.is_terminated() {
                    entries.push((self.current_block, r));
                    self.terminate(Terminator::Branch(join));
                }
            }
            self.close_scope();
            self.set_block(next_item);
        }
        let msg = self.emit(
            Instruction::Const(Constant::String("switch sem caso correspondente".to_string())),
            Type::Ref,
        );
        let e = self.new_runtime_error("dartforge_state_error_new", vec![(msg, Type::Ref)]);
        self.emit_throw_op(e);
        self.terminate(Terminator::Unreachable);
        self.set_block(join);
        if entries.is_empty() {
            return Self::zero_value(ty);
        }
        self.emit(Instruction::Phi { incoming: entries, ty }, ty)
    }

    /// `if (e case p when g) S else T`.
    pub fn lower_if_case(
        &mut self,
        ast: &ast::Ast,
        condition: ExprId,
        pattern: PatternId,
        guard: Option<ExprId>,
        then: StmtId,
        else_: Option<StmtId>,
    ) {
        let v = self.lower_expr(ast, condition);
        let b_else = self.new_block();
        let end = self.new_block();
        self.open_scope();
        let mut linked = HashSet::new();
        let saved = std::mem::replace(&mut self.refutable_pattern, true);
        self.match_pattern(ast, pattern, v, b_else, Linkage::Declare, &mut linked, condition);
        self.refutable_pattern = saved;
        if let Some(g) = guard {
            let ok = self.lower_expr(ast, g);
            self.require(ok, b_else);
        }
        self.lower_stmt(ast, then);
        self.close_scope();
        self.terminate(Terminator::Branch(end));
        self.set_block(b_else);
        if let Some(e) = else_ {
            self.lower_stmt(ast, e);
        }
        self.terminate(Terminator::Branch(end));
        self.set_block(end);
    }

    /// An irrefutable pattern (declaration, assignment, `for-in`): failure is
    /// a run-time error (only `as` and `!` may fail, and those
    /// throw by themselves; a refutable pattern here is rejected by the front end).
    pub fn match_irrefutable(
        &mut self,
        ast: &ast::Ast,
        p: PatternId,
        val_ref: Operand,
        linkage: Linkage,
        origin: ExprId,
    ) {
        let failure = self.new_block();
        let mut linked = HashSet::new();
        let saved = std::mem::replace(&mut self.refutable_pattern, false);
        self.match_pattern(ast, p, val_ref, failure, linkage, &mut linked, origin);
        self.refutable_pattern = saved;
        let follows = self.current_block;
        self.set_block(failure);
        let msg = self.emit(
            Instruction::Const(Constant::String("padrão irrefutável não casou".to_string())),
            Type::Ref,
        );
        let e = self.new_runtime_error("dartforge_state_error_new", vec![(msg, Type::Ref)]);
        self.emit_throw_op(e);
        self.terminate(Terminator::Unreachable);
        self.set_block(follows);
    }
}
