//! Padrões: condições JS com ligações por efeito colateral.

use crate::body::FnEmitter;
use crate::js::{self, Js, P_AND, P_OR, P_PRIMARY};
use crate::ty::Ty;
use dartforge_frontend::ast::{self, BinaryOp, PatternKind};
use dartforge_intern::SymbolId;

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// Condição JS que testa `value_js` (tipo `vty`) contra o padrão, ligando
    /// variáveis (`(x = v, true)`); `binds` recebe (símbolo, tipo) a declarar.
    pub fn pattern_cond(&mut self, p: ast::PatternId, value_js: &str, vty: &Ty, binds: &mut Vec<(SymbolId, Ty)>, irrefutable: bool) -> String {
        let pat = self.ast().pattern(p);
        let v = Js::prim(value_js.to_string());
        match &pat.kind {
            PatternKind::Wildcard { ty } => match ty {
                Some(t) => {
                    let t = self.resolve_type(*t);
                    if self.ctx.is_subtype(vty, &t) && !vty.is_dynamic() {
                        "true".into()
                    } else {
                        self.is_test(&v, &t).code
                    }
                }
                None => "true".into(),
            },
            PatternKind::Variable { ty, name, final_, var_ } if !irrefutable && ty.is_none() && !*final_ && !*var_ && !matches!(self.resolve_ident(name.sym), crate::expr::IdentTarget::Local(..) | crate::expr::IdentTarget::Unknown) => {
                // Identificador solto em contexto de casamento: padrão constante.
                let (c, cty) = self.emit_identifier_value(name.sym);
                self.emit_equals(&c, &cty, &v, vty).code
            }
            PatternKind::Variable { ty, name, .. } => {
                let (test, bty) = match ty {
                    Some(t) => {
                        let t = self.resolve_type(*t);
                        if self.ctx.is_subtype(vty, &t) && !vty.is_dynamic() {
                            (None, t)
                        } else {
                            (Some(self.is_test(&v, &t).code), t)
                        }
                    }
                    None => (None, vty.clone()),
                };
                push_bind(binds, name.sym, bty);
                let jsn = js::ident(self.name(name.sym));
                let assign = format!("({jsn} = {value_js}, true)");
                match test {
                    Some(t) => format!("{} && {assign}", Js::new(t, P_PRIMARY).at(P_AND)),
                    None => assign,
                }
            }
            PatternKind::Constant(e) => {
                let (c, cty) = self.emit_expr(*e, Some(vty));
                self.emit_equals(&c, &cty, &v, vty).code
            }
            PatternKind::Relational { op, value } => {
                let (c, cty) = self.emit_expr(*value, None);
                match op {
                    BinaryOp::Eq => self.emit_equals(&v, vty, &c, &cty).code,
                    BinaryOp::NotEq => format!("!{}", self.emit_equals(&v, vty, &c, &cty).paren().code),
                    _ => self.emit_binop_values(*op, v, vty, c, &cty).0.code,
                }
            }
            PatternKind::Or(a, b) => {
                let ca = self.pattern_cond(*a, value_js, vty, binds, irrefutable);
                let cb = self.pattern_cond(*b, value_js, vty, binds, irrefutable);
                format!("{} || {}", Js::new(ca, P_PRIMARY).at(P_OR), Js::new(cb, P_PRIMARY).at(P_OR + 1))
            }
            PatternKind::And(a, b) => {
                let ca = self.pattern_cond(*a, value_js, vty, binds, irrefutable);
                let cb = self.pattern_cond(*b, value_js, vty, binds, irrefutable);
                and(&ca, &cb)
            }
            PatternKind::NullCheck(inner) => {
                let c = self.pattern_cond(*inner, value_js, &vty.non_null(), binds, irrefutable);
                and(&format!("{value_js} != null"), &c)
            }
            PatternKind::NullAssert(inner) => {
                let c = self.pattern_cond(*inner, value_js, &vty.non_null(), binds, irrefutable);
                and(&format!("({value_js} != null || dart.nullCheck({value_js}))"), &c)
            }
            PatternKind::Cast { pattern, ty } => {
                let t = self.resolve_type(*ty);
                let cast = self.as_cast(&v, &t);
                let c = self.pattern_cond(*pattern, &cast.code, &t, binds, irrefutable);
                c
            }
            PatternKind::Parenthesized(inner) => self.pattern_cond(*inner, value_js, vty, binds, irrefutable),
            PatternKind::List { type_args, elements } => {
                let elem_ty = if let Some(t) = type_args.first() {
                    self.resolve_type(*t)
                } else {
                    self.ctx
                        .list_
                        .and_then(|l| self.ctx.as_super(&vty.non_null(), l))
                        .and_then(|t| t.args().first().cloned())
                        .unwrap_or_else(|| if vty.is_dynamic() { Ty::Dynamic } else { self.ctx.t_object_q() })
                };
                let list_ty = self.ctx.t_list(elem_ty.clone());
                let mut conds = Vec::new();
                if !(self.ctx.is_subtype(vty, &list_ty) && !vty.is_dynamic()) {
                    conds.push(self.is_test(&v, &list_ty).code);
                }
                let rest_idx = elements.iter().position(|e| matches!(e, ast::ListPatternElement::Rest(_)));
                let n = elements.len();
                let len_access = self.member_access(&list_ty, "length", false);
                let get = self.member_access(&list_ty, "[]", false);
                match rest_idx {
                    None => conds.push(format!("{value_js}{len_access} === {n}")),
                    Some(_) => {
                        if n - 1 > 0 {
                            conds.push(format!("{value_js}{len_access} >= {}", n - 1));
                        }
                    }
                }
                for (i, el) in elements.iter().enumerate() {
                    match el {
                        ast::ListPatternElement::Pattern(p) => {
                            let idx = match rest_idx {
                                Some(r) if i > r => format!("{value_js}{len_access} - {}", n - i),
                                _ => i.to_string(),
                            };
                            let sub = format!("{value_js}{get}({idx})");
                            let c = self.pattern_cond(*p, &sub, &elem_ty, binds, irrefutable);
                            if c != "true" {
                                conds.push(c);
                            }
                        }
                        ast::ListPatternElement::Rest(Some(p)) => {
                            let after = n - i - 1;
                            let sublist = self.member_access(&list_ty, "sublist", false);
                            let sub = if after == 0 {
                                format!("{value_js}{sublist}({i})")
                            } else {
                                format!("{value_js}{sublist}({i}, {value_js}{len_access} - {after})")
                            };
                            let c = self.pattern_cond(*p, &sub, &list_ty, binds, irrefutable);
                            if c != "true" {
                                conds.push(c);
                            }
                        }
                        ast::ListPatternElement::Rest(None) => {}
                    }
                }
                join_and(conds)
            }
            PatternKind::Map { type_args, entries, .. } => {
                let (kt, vt) = if type_args.len() == 2 {
                    (self.resolve_type(type_args[0]), self.resolve_type(type_args[1]))
                } else {
                    match self.ctx.map_.and_then(|m| self.ctx.as_super(&vty.non_null(), m)) {
                        Some(t) => (t.args().first().cloned().unwrap_or(Ty::Dynamic), t.args().get(1).cloned().unwrap_or(Ty::Dynamic)),
                        None => {
                            if vty.is_dynamic() {
                                (Ty::Dynamic, Ty::Dynamic)
                            } else {
                                (self.ctx.t_object_q(), self.ctx.t_object_q())
                            }
                        }
                    }
                };
                let map_ty = self.ctx.t_map(kt.clone(), vt.clone());
                let mut conds = Vec::new();
                if !(self.ctx.is_subtype(vty, &map_ty) && !vty.is_dynamic()) {
                    conds.push(self.is_test(&v, &map_ty).code);
                }
                let contains = self.member_access(&map_ty, "containsKey", false);
                let get = self.member_access(&map_ty, "[]", false);
                for e in entries.iter() {
                    let (k, _) = self.emit_expr(e.key, Some(&kt));
                    conds.push(format!("{value_js}{contains}({})", k.code));
                    let sub = format!("{value_js}{get}({})", k.code);
                    let c = self.pattern_cond(e.value, &sub, &vt, binds, irrefutable);
                    if c != "true" {
                        conds.push(c);
                    }
                }
                join_and(conds)
            }
            PatternKind::Record { fields } => {
                // Tipo do record: do valor, ou forma com Object?.
                let (pos_tys, named_tys): (Vec<Ty>, Vec<(String, Ty)>) = match &vty.non_null() {
                    Ty::Record { pos, named, .. } => (pos.clone(), named.clone()),
                    _ => {
                        let npos = fields.iter().filter(|f| f.name.is_none() && !is_inferred_named(self, f)).count();
                        let mut names: Vec<String> = fields.iter().filter_map(|f| self.pattern_field_name(f)).collect();
                        names.sort();
                        (vec![self.ctx.t_object_q(); npos], names.into_iter().map(|n| (n, self.ctx.t_object_q())).collect())
                    }
                };
                let rec_ty = Ty::Record { pos: pos_tys.clone(), named: named_tys.clone(), nullable: false };
                let mut conds = Vec::new();
                if !matches!(vty, Ty::Record { nullable: false, .. }) {
                    conds.push(self.is_test(&v, &rec_ty).code);
                }
                let mut pi = 0;
                for f in fields.iter() {
                    match self.pattern_field_name(f) {
                        Some(n) => {
                            let sub = format!("{value_js}{}", js::prop_access(&n));
                            let fty = named_tys.iter().find(|(x, _)| *x == n).map(|(_, t)| t.clone()).unwrap_or(Ty::Dynamic);
                            let c = self.pattern_cond(f.pattern, &sub, &fty, binds, irrefutable);
                            if c != "true" {
                                conds.push(c);
                            }
                        }
                        None => {
                            pi += 1;
                            let sub = format!("{value_js}.${pi}");
                            let fty = pos_tys.get(pi - 1).cloned().unwrap_or(Ty::Dynamic);
                            let c = self.pattern_cond(f.pattern, &sub, &fty, binds, irrefutable);
                            if c != "true" {
                                conds.push(c);
                            }
                        }
                    }
                }
                join_and(conds)
            }
            PatternKind::Object { ty, fields } => {
                let t = self.resolve_type(*ty);
                let mut conds = Vec::new();
                if !(self.ctx.is_subtype(vty, &t) && !vty.is_dynamic()) {
                    conds.push(self.is_test(&v, &t).code);
                }
                for f in fields.iter() {
                    let Some(n) = self.pattern_field_name(f) else { continue };
                    let (g, gty) = self.emit_member_get(&v, &t, &n, None);
                    let c = self.pattern_cond(f.pattern, &g.code, &gty, binds, irrefutable);
                    if c != "true" {
                        conds.push(c);
                    }
                }
                join_and(conds)
            }
        }
    }

    /// Nome do campo de um `PatternField` (`nome:` explícito ou `:x` inferido).
    pub fn pattern_field_name(&self, f: &ast::PatternField) -> Option<String> {
        if let Some(n) = f.name {
            return Some(self.name(n.sym).to_string());
        }
        None
    }
}

fn is_inferred_named(_s: &FnEmitter, _f: &ast::PatternField) -> bool {
    false
}

fn push_bind(binds: &mut Vec<(SymbolId, Ty)>, sym: SymbolId, ty: Ty) {
    if !binds.iter().any(|(s, _)| *s == sym) {
        binds.push((sym, ty));
    }
}

fn and(a: &str, b: &str) -> String {
    if a == "true" {
        return b.to_string();
    }
    if b == "true" {
        return a.to_string();
    }
    format!("{} && {}", Js::new(a.to_string(), P_PRIMARY).at(P_AND), Js::new(b.to_string(), P_PRIMARY).at(P_AND + 1))
}

fn join_and(conds: Vec<String>) -> String {
    let conds: Vec<String> = conds.into_iter().filter(|c| c != "true").collect();
    if conds.is_empty() {
        return "true".into();
    }
    conds.iter().map(|c| paren_cond(c)).collect::<Vec<_>>().join(" && ")
}

fn paren_cond(c: &str) -> String {
    // Parênteses quando a condição tem operadores de baixa precedência.
    if c.contains("||") || c.contains(" ? ") || c.contains(", ") {
        format!("({c})")
    } else {
        c.to_string()
    }
}
