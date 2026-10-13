//! Collection literals with control elements (P3): `...e`, `...?e`,
//! `?e`, `if (c) e else f`, `for (…) e`, and the set literal.
//!
//! The specification (§17.9 "Collection literals") evaluates the elements in
//! order and appends each produced value; `...e` appends the elements of
//! `e` (`...?e` ignores null); `if` and `for` produce zero or more values. The
//! list literal with only expressions stays on the direct allocation
//! (`AllocList`, `llvm/ir_lists.rs`); the others start empty (the
//! `_GrowableList` of the literal, the source `_Map`/`_Set`) and append.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, CollectionElement, ExprId, ForInit};

/// From how many elements a constant literal goes in chunks
/// (`FnBuilder::fill_in_chunks`).
const CHUNK_THRESHOLD: usize = 256;
/// How many elements each auxiliary function of a chunked literal evaluates.
const LITERAL_CHUNK: usize = 128;

/// Which collection the literal builds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Collection {
    List,
    Set,
    Map,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Is the `{…}` literal a set (and not a map)? By the static type, or
    /// by the shape of the elements (an empty `{}` is a map).
    pub fn literal_is_set(&self, expr_id: ExprId, elements: &[CollectionElement]) -> bool {
        if let Some(t) = self.ctx.get_type(self.unit_id, expr_id)
            && let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(t)
        {
            match self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name) {
                "Set" | "LinkedHashSet" => return true,
                "Map" | "LinkedHashMap" => return false,
                _ => {}
            }
        }
        fn leaf(el: &CollectionElement) -> Option<bool> {
            match el {
                CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => Some(true),
                CollectionElement::MapEntry { .. } => Some(false),
                CollectionElement::If { then, .. } => leaf(then),
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => leaf(body),
                CollectionElement::Spread { .. } => None,
            }
        }
        elements.iter().find_map(leaf).unwrap_or(false)
    }

    /// An empty collection of the requested type: the `_GrowableList` of the literal (`AllocList`
    /// without elements) or the source `_Map`/`_Set`.
    fn empty_collection(&mut self, typ: Collection) -> Operand {
        match typ {
            Collection::List => self.emit(Instruction::AllocList { elements: Vec::new() }, Type::Ref),
            _ => self.source_empty_collection(typ == Collection::Map),
        }
    }

    /// Appends a value to the list or to the set. The list is the `_GrowableList`
    /// that the literal just created (still without the reified `E`, written at the end
    /// by whoever lowers the literal): the runtime's direct `add`
    /// (`dartforge_lista_acrescentar`, the value in a `Ref` position), without the
    /// dispatch nor the covariance check, which the static type of the
    /// element already guarantees. The set uses the `add` of `_Set`.
    fn append(&mut self, alvo: Operand, typ: Collection, v: Operand) {
        if typ == Collection::Set {
            self.call_by_name(alvo, super::sdk_source::AccessKind::Call, "add", &[(None, v)]);
            return;
        }
        let v = self.coerce(v, Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_lista_acrescentar".to_string(),
                args: vec![(alvo, Type::Ref), (v, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
    }

    /// Appends an entry to the map (the `[]=` of `_Map`).
    fn append_entry(&mut self, alvo: Operand, k: Operand, v: Operand) {
        self.call_by_name(alvo, super::sdk_source::AccessKind::Call, "[]=", &[(None, k), (None, v)]);
    }

    /// A collection literal with control elements (or a set literal).
    pub fn lower_collection_literal(
        &mut self,
        ast: &ast::Ast,
        typ: Collection,
        elements: &[CollectionElement],
        span: Span,
    ) -> Operand {
        let alvo = self.empty_collection(typ);
        if self.fill_from_table(ast, alvo.clone(), typ, elements) {
            return alvo;
        }
        if self.fill_in_chunks(ast, alvo.clone(), typ, elements, span) {
            return alvo;
        }
        if self.fill_map_at_once(ast, alvo.clone(), typ, elements) {
            return alvo;
        }
        for el in elements {
            self.collection_element(ast, alvo.clone(), typ, el, span);
        }
        alvo
    }

    /// A large literal of only strings, bools and null (the name and entity
    /// tables of real packages) becomes DATA: the elements in a byte
    /// table that the runtime converts into a list
    /// (`dartforge_lista_de_tabela`), and a Dart support function that copies it into the
    /// collection — instead of one `add`/`[]=` call per element, which
    /// made the getter of a 2-thousand-entry constant a function of 50
    /// thousand instructions. Numbers come in through the static type of the literal
    /// (`int` as `i`, `double` as `d`); a number of another static type
    /// (`num`, `Object`: the value is the `int`, but with no guarantee here) keeps the
    /// literal out of the table.
    ///
    /// In a constant context (the getter of a constant), an element that is
    /// another constant (`C(…)`, another literal, `E.value`) comes in as `g` and the
    /// index of its canonical getter (`dfc.<hash>.get`) in a vector of
    /// addresses (`FunctionTable`), which the runtime calls
    /// (`dartforge_lista_de_tabela_g`): large constant maps of
    /// constant values (`{'a': C(1), …}`) did one `[]=` by selector per
    /// entry — 30 MB of IR in 234 getters in new_sali/backend
    /// (docs/NATIVO-PRODUCAO-GRANDE.md §6.5).
    fn fill_from_table(&mut self, ast: &ast::Ast, alvo: Operand, typ: Collection, elements: &[CollectionElement]) -> bool {
        const MINIMUM: usize = 8;
        if elements.len() < MINIMUM {
            return false;
        }
        let mut table: Vec<u8> = Vec::new();
        let mut by_getter: Vec<ast::ExprId> = Vec::new();
        let in_const = self.in_const_context;
        let scalar = |e: ast::ExprId, table: &mut Vec<u8>, by_getter: &mut Vec<ast::ExprId>| -> bool {
            // Number literal (or `-literal`): by the STATIC TYPE of the expression,
            // which already says whether the context made an integer literal a `double`
            // (the `Map<int, int>` tables of `enough_convert`, 30 thousand
            // entries).
            let number = matches!(ast.expr(e).kind, ast::ExprKind::Int(_) | ast::ExprKind::Double(_))
                || matches!(&ast.expr(e).kind, ast::ExprKind::Unary { op: ast::UnaryOp::Neg, operand }
                    if matches!(ast.expr(*operand).kind, ast::ExprKind::Int(_) | ast::ExprKind::Double(_)));
            if number {
                let Some(map_key) = self.constant_key(ast, e, true) else { return false };
                let typ = self.ctx.get_type(self.unit_id, e);
                let (int, double) = (Some(self.ctx.core.int), Some(self.ctx.core.double));
                let val_ref = match map_key.as_str() {
                    k if k.starts_with("i:") => k[2..].parse::<i64>().ok().map(|v| (v, false)),
                    k if k.starts_with("d:") => k[2..].parse::<u64>().ok().map(|b| (b as i64, true)),
                    k if k.starts_with("neg(i:") && k.ends_with(')') => {
                        k[6..k.len() - 1].parse::<i64>().ok().map(|v| (v.wrapping_neg(), false))
                    }
                    k if k.starts_with("neg(d:") && k.ends_with(')') => {
                        k[6..k.len() - 1].parse::<u64>().ok().map(|b| ((-f64::from_bits(b)).to_bits() as i64, true))
                    }
                    _ => None,
                };
                let Some((v, is_double)) = val_ref else { return false };
                if typ == int && !is_double {
                    table.push(b'i');
                    table.extend_from_slice(&v.to_le_bytes());
                } else if typ == double {
                    let bits = if is_double { v as u64 } else { (v as f64).to_bits() };
                    table.push(b'd');
                    table.extend_from_slice(&bits.to_le_bytes());
                } else {
                    return false;
                }
                return true;
            }
            let other_constant = |table: &mut Vec<u8>, by_getter: &mut Vec<ast::ExprId>| -> bool {
                if !in_const || self.constant_key(ast, e, true).is_none() {
                    return false;
                }
                table.push(b'g');
                table.extend_from_slice(&(by_getter.len() as u32).to_le_bytes());
                by_getter.push(e);
                true
            };
            match &ast.expr(e).kind {
                ast::ExprKind::Null => table.push(b'n'),
                ast::ExprKind::Bool(b) => table.push(if *b { b't' } else { b'f' }),
                ast::ExprKind::String(s) => {
                    let Some(text) = s.constant_value() else { return false };
                    let bytes = text.as_bytes();
                    table.push(b's');
                    table.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    table.extend_from_slice(bytes);
                }
                _ => return other_constant(table, by_getter),
            }
            true
        };
        for el in elements {
            let ok = match (el, typ) {
                (CollectionElement::Expression(e), Collection::List | Collection::Set) => scalar(*e, &mut table, &mut by_getter),
                (
                    CollectionElement::MapEntry { key, value, null_aware_key: false, null_aware_value: false },
                    Collection::Map,
                ) => scalar(*key, &mut table, &mut by_getter) && scalar(*value, &mut table, &mut by_getter),
                _ => false,
            };
            if !ok {
                return false;
            }
        }
        let mut getters: Vec<String> = Vec::with_capacity(by_getter.len());
        for e in by_getter {
            let Some(g) = self.constant_getter(ast, e) else { return false };
            getters.push(g);
        }
        let support = match typ {
            Collection::List => "_dartforgePreencherLista",
            Collection::Set => "_dartforgePreencherConjunto",
            Collection::Map => "_dartforgePreencherMapa",
        };
        let Some(fid) = self.top_level_function("dart:_compact_hash", support) else { return false };
        let words: Vec<i64> = table
            .chunks(8)
            .map(|c| {
                let mut w = [0u8; 8];
                w[..c.len()].copy_from_slice(c);
                i64::from_le_bytes(w)
            })
            .collect();
        let data = self.emit(Instruction::ConstArray(words), Type::Ptr);
        let size = (Operand::Constant(Constant::Int(table.len() as i64)), Type::I64);
        let values = if getters.is_empty() {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_lista_de_tabela".to_string(),
                    args: vec![(data, Type::Ptr), size],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else {
            let fns = self.emit(Instruction::FunctionTable(getters), Type::Ptr);
            self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_lista_de_tabela_g".to_string(),
                    args: vec![(data, Type::Ptr), size, (fns, Type::Ptr)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        };
        let evaluated: Vec<super::members::Evaluated> = vec![(None, alvo), (None, values)];
        let args = self.match_args(fid, &evaluated);
        self.call_direct(fid, None, args);
        true
    }

    /// The map literal with only entries (`{k: v, …}`, without control
    /// elements nor `?`), as the VM builds it: evaluates the keys and the values in
    /// order, keeps them in a list `[k0, v0, k1, v1, …]` (`AllocList`, a single
    /// allocation) and inserts everything at the end (`_dartforgePreencherMapa`). The VM
    /// does the same (`Map._fromLiteral`,
    /// runtime/vm/compiler/frontend/kernel_binary_flowgraph.cc): the `hashCode`
    /// and the `==` of the keys only run after all values are evaluated.
    /// Before it was one `[]=` by selector per entry — the argument vector, the
    /// selector lookup, the indirect call and the exception check for each
    /// one: the `toMap`/`toJson` of a real backend are made of that. Outside
    /// ARC, which builds the list through the Owned factory (`ir_lists.rs`).
    fn fill_map_at_once(&mut self, ast: &ast::Ast, alvo: Operand, typ: Collection, elements: &[CollectionElement]) -> bool {
        // Up to `LITERAL_CHUNK` entries: the values stay alive (and
        // rooted) until the list, in the frame.
        if typ != Collection::Map || elements.len() < 2 || elements.len() > LITERAL_CHUNK || self.ctx.arc_memory {
            return false;
        }
        let only_entries = elements.iter().all(|el| {
            matches!(el, CollectionElement::MapEntry { null_aware_key: false, null_aware_value: false, .. })
        });
        if !only_entries {
            return false;
        }
        let Some(fid) = self.top_level_function("dart:_compact_hash", "_dartforgePreencherMapa") else { return false };
        let mut values: Vec<(Operand, u8)> = Vec::with_capacity(elements.len() * 2);
        for el in elements {
            let CollectionElement::MapEntry { key, value, .. } = el else { unreachable!("conferido acima") };
            for e in [*key, *value] {
                let x = self.lower_expr(ast, e);
                if self.is_terminated() {
                    return true;
                }
                let tag = self.operand_tag(&x);
                values.push((x, tag));
            }
        }
        let list = self.emit(Instruction::AllocList { elements: values }, Type::Ref);
        let evaluated: Vec<super::members::Evaluated> = vec![(None, alvo), (None, list)];
        let args = self.match_args(fid, &evaluated);
        self.call_direct(fid, None, args);
        true
    }

    /// A large literal of constant elements that the table does not cover
    /// (integers, enum values, `const`s of other declarations — the
    /// Unicode tables of `bidi`, 16 thousand entries `65: CharacterCategory.lu`).
    ///
    /// In a single function, each element left its argument buffer in the frame
    /// (`[]=` by selector) and, at `-O0`, a spill word per value that crosses
    /// the exception check: the
    /// frame of the getter went past 1 MiB and overflowed the stack of the main
    /// isolate (the main thread of the process, 1 MiB on Windows — the
    /// same reservation as the VM, `OSThread::GetMaxStackSize`,
    /// runtime/vm/os_thread_win.cc:101-104). The VM does not have the problem because
    /// the literal is a vector filled by `StoreIndexed` and a single
    /// `Map._fromLiteral`/`_GrowableList._literal`
    /// (runtime/vm/compiler/frontend/kernel_binary_flowgraph.cc:4051-4140).
    ///
    /// Here the elements go, in order, through auxiliary functions of
    /// [`LITERAL_CHUNK`] elements each (`<function>$lit<pos>_<i>(target)`), which
    /// append to `target` like the common path. Only with constant
    /// elements (the canonical key exists): a constant does not read locals,
    /// `this` nor type parameters, so the evaluation outside the function that
    /// contains the literal is the same, in the same order — and the local `const`s go
    /// along (`local_const_keys`), reevaluated by the initializer
    /// as in the constant getter.
    fn fill_in_chunks(
        &mut self,
        ast: &ast::Ast,
        alvo: Operand,
        typ: Collection,
        elements: &[CollectionElement],
        span: Span,
    ) -> bool {
        if elements.len() < CHUNK_THRESHOLD {
            return false;
        }
        let constants = elements.iter().all(|el| match (el, typ) {
            (CollectionElement::Expression(e), Collection::List | Collection::Set) => {
                self.constant_key(ast, *e, true).is_some()
            }
            (
                CollectionElement::MapEntry { key, value, null_aware_key: false, null_aware_value: false },
                Collection::Map,
            ) => self.constant_key(ast, *key, true).is_some() && self.constant_key(ast, *value, true).is_some(),
            _ => false,
        });
        if !constants {
            return false;
        }
        let alvo = self.coerce(alvo, Type::Ref);
        for (i, chunk) in elements.chunks(LITERAL_CHUNK).enumerate() {
            let symbol = format!("{}$lit{}_{i}", self.func.symbol, span.start);
            let mut g = FnBuilder::new(self.ctx, self.unit_id, symbol.clone(), "literal".to_string(), Type::Ref);
            g.in_const_context = self.in_const_context;
            g.local_const_keys = self.local_const_keys.clone();
            let p = Operand::Val(g.add_param("alvo".to_string(), Type::Ref));
            for el in chunk {
                if g.is_terminated() {
                    break;
                }
                g.collection_element(ast, p.clone(), typ, el, span);
            }
            if !g.is_terminated() {
                g.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
            }
            // Called as a Dart function (`CallStatic`): the Ref return is Owned.
            if self.ctx.arc_memory {
                g.dart_ref_returns.insert(symbol.clone());
            }
            self.absorb(g);
            self.emit_call_with_check(
                Instruction::CallStatic { symbol: symbol, args: vec![alvo.clone()], ret_ty: Type::Ref },
                Type::Ref,
            );
        }
        true
    }

    /// Does the list literal with only expressions go through the chunked path?
    /// (large, and every element constant: `fill_in_chunks`).
    pub fn large_literal_is_constant(&self, ast: &ast::Ast, elements: &[CollectionElement]) -> bool {
        elements.len() >= CHUNK_THRESHOLD
            && elements.iter().all(|el| match el {
                CollectionElement::Expression(e) => self.constant_key(ast, *e, true).is_some(),
                _ => false,
            })
    }

    /// `e` names a `const` (top-level or static, of this or of another
    /// unit) whose initializer is a list, set or map literal:
    /// the unit and the literal.
    fn constant_literal(&self, ast: &ast::Ast, e: ExprId) -> Option<(dartforge_elements::model::UnitId, ExprId)> {
        use dartforge_elements::model::{Element, VariableRef};
        use dartforge_types::resolved::{MemberRef, Resolved};
        if !matches!(ast.expr(e).kind, ast::ExprKind::Identifier(_) | ast::ExprKind::Property { .. }) {
            return None;
        }
        let vid = match self.ctx.get_resolved(self.unit_id, e)? {
            Resolved::Element(Element::Variable(v)) => *v,
            Resolved::Element(Element::Function(f)) => self.ctx.program.functions[f.0 as usize].variable?,
            Resolved::Member { member: MemberRef::Variable(v), .. } => *v,
            Resolved::Member { member: MemberRef::Function(f), .. } => self.ctx.program.functions[f.0 as usize].variable?,
            _ => return None,
        };
        let var = &self.ctx.program.variables[vid.0 as usize];
        if !var.const_ {
            return None;
        }
        let unit_ref = match var.node {
            VariableRef::Field { unit, .. } | VariableRef::TopLevel { unit, .. } => unit,
            _ => return None,
        };
        let mut init = self.variable_initializer_in(vid)?;
        let ast_c = &self.ctx.program.unit(unit_ref).ast;
        while let ast::ExprKind::Parenthesized(x) = &ast_c.expr(init).kind {
            init = *x;
        }
        matches!(ast_c.expr(init).kind, ast::ExprKind::List { .. } | ast::ExprKind::SetOrMap { .. }).then_some((unit_ref, init))
    }

    /// The top-level function `name` of library `uri`.
    pub fn top_level_function(&self, uri: &str, name: &str) -> Option<usize> {
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == uri)?;
        let sym = self.ctx.interner.lookup(name)?;
        let b = self.ctx.program.lookup(dartforge_elements::model::LibraryId(lib as u32), sym)?;
        match b.getter? {
            dartforge_elements::model::Element::Function(f) => Some(f.0 as usize),
            _ => None,
        }
    }

    fn collection_element(
        &mut self,
        ast: &ast::Ast,
        alvo: Operand,
        typ: Collection,
        el: &CollectionElement,
        span: Span,
    ) {
        match el {
            CollectionElement::Expression(e) => {
                let v = self.lower_expr(ast, *e);
                if typ == Collection::Map {
                    self.unsupported("expressão num literal de mapa", span);
                    return;
                }
                self.append(alvo, typ, v);
            }
            CollectionElement::NullAwareExpression(e) => {
                let v = self.lower_expr(ast, *e);
                self.if_non_null(v, |s, v| s.append(alvo.clone(), typ, v));
            }
            CollectionElement::MapEntry {
                key,
                value,
                null_aware_key,
                null_aware_value,
            } => {
                if *null_aware_key || *null_aware_value {
                    self.unsupported("entrada de mapa null-aware", span);
                    return;
                }
                let k = self.lower_expr(ast, *key);
                let v = self.lower_expr(ast, *value);
                self.append_entry(alvo, k, v);
            }
            CollectionElement::Spread { value, null_aware } => {
                // In a constant context, `...c` of a `const` whose
                // initializer is a literal appends the elements OF THE
                // LITERAL (§17.9.1: the result is the same), instead of reading the
                // global `c`. Reading the global created a cycle when the
                // constant being built has the same canonical key as that
                // of `c` (`const [...c]` with `const c = [1, 2]`): the canonical
                // getter evaluated `c`, whose initializer called the same
                // getter still in progress — the `StackOverflowError` of the reentrant
                // global (corpus/native/120).
                if self.in_const_context
                    && let Some((unit_ref, init)) = self.constant_literal(ast, *value)
                {
                    let ctx = self.ctx;
                    let ast_c = &ctx.program.unit(unit_ref).ast;
                    let element_list = match &ast_c.expr(init).kind {
                        ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } => elements,
                        _ => unreachable!("literal_da_constante devolve um literal"),
                    };
                    let saved = std::mem::replace(&mut self.unit_id, unit_ref);
                    for el in element_list.iter() {
                        self.collection_element(ast_c, alvo.clone(), typ, el, span);
                    }
                    self.unit_id = saved;
                    return;
                }
                let source = self.lower_expr(ast, *value);
                let source = self.coerce(source, Type::Ref);
                // `...e` is the `addAll` of the collection (§17.9.1).
                let spread = |s: &mut Self, f: Operand| {
                    s.call_by_name(alvo.clone(), super::sdk_source::AccessKind::Call, "addAll", &[(None, f)]);
                };
                if *null_aware {
                    self.if_non_null(source, spread);
                } else {
                    spread(self, source);
                }
            }
            CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                let c = self.lower_expr(ast, *condition);
                let b_else = self.new_block();
                let end = self.new_block();
                self.open_scope();
                if let Some(p) = case_pattern {
                    let mut linked = std::collections::HashSet::new();
                    let saved = std::mem::replace(&mut self.refutable_pattern, true);
                    self.match_pattern(ast, *p, c, b_else, super::patterns::Linkage::Declare, &mut linked, *condition);
                    self.refutable_pattern = saved;
                    if let Some(g) = guard {
                        let ok = self.lower_expr(ast, *g);
                        let ok = self.to_bool(ok);
                        let follows = self.new_block();
                        self.terminate(Terminator::CondBranch {
                            cond: ok,
                            then_block: follows,
                            else_block: b_else,
                        });
                        self.set_block(follows);
                    }
                } else {
                    let c = self.to_bool(c);
                    let b_then = self.new_block();
                    self.terminate(Terminator::CondBranch {
                        cond: c,
                        then_block: b_then,
                        else_block: b_else,
                    });
                    self.set_block(b_then);
                }
                self.collection_element(ast, alvo.clone(), typ, then, span);
                self.close_scope();
                self.terminate(Terminator::Branch(end));
                self.set_block(b_else);
                if let Some(e) = else_ {
                    self.collection_element(ast, alvo, typ, e, span);
                }
                self.terminate(Terminator::Branch(end));
                self.set_block(end);
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.open_scope();
                match init {
                    Some(ForInit::Variables(list)) => {
                        for var in list.variables.iter() {
                            let ty = self.local_repr_of(var.name.span.start as usize);
                            let v = match var.initializer {
                                Some(i) => self.lower_expr(ast, i),
                                None => Self::zero_value(ty),
                            };
                            self.declare_variable(var.name.sym, var.name.span.start as usize, ty, v);
                        }
                    }
                    Some(ForInit::Expression(e)) => {
                        self.lower_expr(ast, *e);
                    }
                    Some(ForInit::Pattern { pattern, value, .. }) => {
                        let v = self.lower_expr(ast, *value);
                        self.match_irrefutable(ast, *pattern, v, super::patterns::Linkage::Declare, *value);
                    }
                    None => {}
                }
                let head = self.new_block();
                let fn_body = self.new_block();
                let end = self.new_block();
                self.terminate(Terminator::Branch(head));
                self.set_block(head);
                match condition {
                    Some(c) => {
                        let c = self.lower_expr(ast, *c);
                        let c = self.to_bool(c);
                        self.terminate(Terminator::CondBranch {
                            cond: c,
                            then_block: fn_body,
                            else_block: end,
                        });
                    }
                    None => self.terminate(Terminator::Branch(fn_body)),
                }
                self.set_block(fn_body);
                self.emit_safepoint();
                self.collection_element(ast, alvo, typ, body, span);
                if let Some(ForInit::Variables(list)) = init {
                    for var in list.variables.iter() {
                        self.renew_cell(var.name.sym);
                    }
                }
                for &u in updates.iter() {
                    self.lower_expr(ast, u);
                }
                self.terminate(Terminator::Branch(head));
                self.set_block(end);
                self.close_scope();
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                let source = self.lower_expr(ast, *iterable);
                let source = self.coerce(source, Type::Ref);
                let fn_body: &CollectionElement = body;
                self.iterate_source(source, &mut |s: &mut Self, x: Operand| {
                    s.open_scope();
                    s.bind_for_in_target(ast, target, x, *iterable, span);
                    s.collection_element(ast, alvo.clone(), typ, fn_body, span);
                    s.close_scope();
                });
            }
        }
    }

    /// `if (v != null) f(v)`.
    fn if_non_null(&mut self, v: Operand, f: impl FnOnce(&mut Self, Operand)) {
        if self.operand_type(&v) != Type::Ref {
            f(self, v);
            return;
        }
        let nn = self.emit(
            Instruction::ICmp(ICmpOp::Ne, v.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: nn,
            then_block: b,
            else_block: end,
        });
        self.set_block(b);
        f(self, v);
        self.terminate(Terminator::Branch(end));
        self.set_block(end);
    }

    /// `for (i = 0; i < n; i++) f(i)` with the index in an `alloca` (R6).
    pub fn loop_index(&mut self, n: Operand, f: impl FnOnce(&mut Self, Operand)) {
        let p = self.entry_alloca(Type::I64);
        self.emit(
            Instruction::Store {
                ptr: p.clone(),
                val: Operand::Constant(Constant::Int(0)),
            },
            Type::Void,
        );
        let head = self.new_block();
        let fn_body = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::Branch(head));
        self.set_block(head);
        let i = self.emit(
            Instruction::Load {
                ptr: p.clone(),
                ty: Type::I64,
            },
            Type::I64,
        );
        let c = self.emit(Instruction::ICmp(ICmpOp::Slt, i.clone(), n), Type::I1);
        self.terminate(Terminator::CondBranch {
            cond: c,
            then_block: fn_body,
            else_block: end,
        });
        self.set_block(fn_body);
        f(self, i.clone());
        let next = self.emit(Instruction::Add(i, Operand::Constant(Constant::Int(1))), Type::I64);
        self.emit(Instruction::Store { ptr: p, val: next }, Type::Void);
        self.terminate(Terminator::Branch(head));
        self.set_block(end);
    }

    /// Element `i` of a list or set (`for-in`, spread) in
    /// representation `repr` (R5).
    pub fn read_iterable_element(&mut self, source: Operand, i: Operand, repr: Type) -> Operand {
        if repr == Type::Ref {
            return self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_iteravel_get_ref".to_string(),
                    args: vec![(source, Type::Ref), (i, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        let bits = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_iteravel_get_bits".to_string(),
                args: vec![(source, Type::Ref), (i, Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.bits_to(bits, repr)
    }
}
