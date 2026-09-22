//! Avaliador de expressões e valores constantes (`const`).
//!
//! Avalia expressões em tempo de compilação para:
//! - Anotações e metadados.
//! - Valores padrão de parâmetros opcionais.
//! - Casos de `switch` clássicos.
//! - Expressões `const` de aritmética, strings (UTF-16 via `DartStr`), coleções e construtores.
//! - Consultas de ambiente (`fromEnvironment`).

use crate::table::{CoreTypes, TypeId, TypeTable};
use dartforge_elements::model::{ClassId, Program, UnitId};
use dartforge_frontend::ast::{self, BinaryOp, UnaryOp};
use dartforge_frontend::text::{DartStr, DartStrBuilder};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// Valor avaliado de uma expressão constante em tempo de compilação.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    String(DartStr),
    List(Vec<ConstValue>),
    Map(Vec<(ConstValue, ConstValue)>),
    Set(Vec<ConstValue>),
    Record {
        positional: Vec<ConstValue>,
        named: Vec<(SymbolId, ConstValue)>,
    },
    Instance {
        class: ClassId,
        constructor: Option<SymbolId>,
        fields: Vec<(SymbolId, ConstValue)>,
    },
    Type(TypeId),
    Symbol(Vec<SymbolId>),
}

/// Avaliador de expressões constantes em uma unidade sintática.
pub struct ConstantEvaluator<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub env_variables: HashMap<String, String>,
    pub error_thrown: Option<String>,
}

impl<'a> ConstantEvaluator<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
    ) -> Self {
        Self {
            program,
            interner,
            table,
            core,
            env_variables: HashMap::new(),
            error_thrown: None,
        }
    }

    /// Avalia recursivamente uma expressão para um [`ConstValue`].
    pub fn evaluate_expr(&mut self, unit_id: UnitId, expr_id: ast::ExprId) -> Option<ConstValue> {
        let expr = &self.program.unit(unit_id).ast.exprs[expr_id.0 as usize];

        match &expr.kind {
            ast::ExprKind::Int(span) => {
                let text = &self.program.unit(unit_id).source[span.start as usize..span.end as usize];
                text.trim().parse::<i64>().ok().map(ConstValue::Int)
            }
            ast::ExprKind::Double(span) => {
                let text = &self.program.unit(unit_id).source[span.start as usize..span.end as usize];
                text.trim().parse::<f64>().ok().map(ConstValue::Double)
            }
            ast::ExprKind::Bool(b) => Some(ConstValue::Bool(*b)),
            ast::ExprKind::Null => Some(ConstValue::Null),
            ast::ExprKind::String(lit) => {
                let mut builder = DartStrBuilder::default();
                for part in &lit.parts {
                    match part {
                        ast::StringPart::Text(t) => {
                            for u in t.code_units() {
                                builder.push_unit(u);
                            }
                        }
                        ast::StringPart::Interpolation(interp_expr) => {
                            let val = self.evaluate_expr(unit_id, *interp_expr)?;
                            match val {
                                ConstValue::String(s) => {
                                    for u in s.code_units() {
                                        builder.push_unit(u);
                                    }
                                }
                                ConstValue::Int(i) => {
                                    let s = i.to_string();
                                    for c in s.encode_utf16() {
                                        builder.push_unit(c);
                                    }
                                }
                                ConstValue::Double(d) => {
                                    let s = d.to_string();
                                    for c in s.encode_utf16() {
                                        builder.push_unit(c);
                                    }
                                }
                                ConstValue::Bool(b) => {
                                    let s = if b { "true" } else { "false" };
                                    for c in s.encode_utf16() {
                                        builder.push_unit(c);
                                    }
                                }
                                ConstValue::Null => {
                                    for c in "null".encode_utf16() {
                                        builder.push_unit(c);
                                    }
                                }
                                _ => return None,
                            }
                        }
                    }
                }
                Some(ConstValue::String(builder.finish()))
            }
            ast::ExprKind::Symbol(names) => {
                let syms = names.iter().map(|n| n.sym).collect();
                Some(ConstValue::Symbol(syms))
            }
            ast::ExprKind::Parenthesized(inner) => self.evaluate_expr(unit_id, *inner),
            ast::ExprKind::Unary { op, operand } => {
                let val = self.evaluate_expr(unit_id, *operand)?;
                self.evaluate_unary(*op, val)
            }
            ast::ExprKind::Binary { op, left, right } => {
                let l_val = self.evaluate_expr(unit_id, *left)?;
                let r_val = self.evaluate_expr(unit_id, *right)?;
                self.evaluate_binary(*op, l_val, r_val)
            }
            ast::ExprKind::Conditional { condition, then, else_ } => {
                let cond_val = self.evaluate_expr(unit_id, *condition)?;
                match cond_val {
                    ConstValue::Bool(true) => self.evaluate_expr(unit_id, *then),
                    ConstValue::Bool(false) => self.evaluate_expr(unit_id, *else_),
                    _ => None,
                }
            }
            ast::ExprKind::List { elements, .. } => {
                let mut list = Vec::with_capacity(elements.len());
                for el in elements {
                    match el {
                        ast::CollectionElement::Expression(e) => {
                            list.push(self.evaluate_expr(unit_id, *e)?);
                        }
                        ast::CollectionElement::Spread { value, .. } => {
                            let spread_val = self.evaluate_expr(unit_id, *value)?;
                            match spread_val {
                                ConstValue::List(items) | ConstValue::Set(items) => {
                                    list.extend(items);
                                }
                                _ => return None,
                            }
                        }
                        _ => return None,
                    }
                }
                Some(ConstValue::List(list))
            }
            ast::ExprKind::SetOrMap { elements, .. } => {
                // Avalia se é mapa ou conjunto
                let is_map = elements.iter().any(|el| matches!(el, ast::CollectionElement::MapEntry { .. }));
                if is_map {
                    let mut map = Vec::new();
                    for el in elements {
                        if let ast::CollectionElement::MapEntry { key, value, .. } = el {
                            let k = self.evaluate_expr(unit_id, *key)?;
                            let v = self.evaluate_expr(unit_id, *value)?;
                            map.push((k, v));
                        }
                    }
                    Some(ConstValue::Map(map))
                } else {
                    let mut set = Vec::new();
                    for el in elements {
                        if let ast::CollectionElement::Expression(e) = el {
                            set.push(self.evaluate_expr(unit_id, *e)?);
                        }
                    }
                    Some(ConstValue::Set(set))
                }
            }
            ast::ExprKind::Record { positional, named, .. } => {
                let mut pos = Vec::with_capacity(positional.len());
                for &e in positional {
                    pos.push(self.evaluate_expr(unit_id, e)?);
                }
                let mut nmd = Vec::with_capacity(named.len());
                for (name, e) in named {
                    nmd.push((name.sym, self.evaluate_expr(unit_id, *e)?));
                }
                Some(ConstValue::Record {
                    positional: pos,
                    named: nmd,
                })
            }
            ast::ExprKind::Call { target, arguments } => {
                // Trata métodos estáticos / construtores especiais como identical e fromEnvironment
                self.evaluate_special_call(unit_id, *target, arguments)
            }
            _ => None,
        }
    }

    fn evaluate_unary(&self, op: UnaryOp, val: ConstValue) -> Option<ConstValue> {
        match (op, val) {
            (UnaryOp::Neg, ConstValue::Int(i)) => Some(ConstValue::Int(-i)),
            (UnaryOp::Neg, ConstValue::Double(d)) => Some(ConstValue::Double(-d)),
            (UnaryOp::Not, ConstValue::Bool(b)) => Some(ConstValue::Bool(!b)),
            (UnaryOp::BitNot, ConstValue::Int(i)) => Some(ConstValue::Int(!i)),
            _ => None,
        }
    }

    fn evaluate_binary(&mut self, op: BinaryOp, left: ConstValue, right: ConstValue) -> Option<ConstValue> {
        match (op, left, right) {
            // Aritmética inteira
            (BinaryOp::Add, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a.wrapping_add(b))),
            (BinaryOp::Sub, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a.wrapping_sub(b))),
            (BinaryOp::Mul, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a.wrapping_mul(b))),
            (BinaryOp::Div, ConstValue::Int(a), ConstValue::Int(b)) => {
                if b == 0 {
                    self.error_thrown = Some("IntegerDivisionByZeroException".to_string());
                    None
                } else {
                    Some(ConstValue::Double(a as f64 / b as f64))
                }
            }
            (BinaryOp::TruncDiv, ConstValue::Int(a), ConstValue::Int(b)) => {
                if b == 0 {
                    self.error_thrown = Some("IntegerDivisionByZeroException".to_string());
                    None
                } else {
                    Some(ConstValue::Int(a / b))
                }
            }
            (BinaryOp::Rem, ConstValue::Int(a), ConstValue::Int(b)) => {
                if b == 0 {
                    self.error_thrown = Some("IntegerDivisionByZeroException".to_string());
                    None
                } else {
                    Some(ConstValue::Int(a % b))
                }
            }
            // Operadores bitwise com semântica de 32/64 bits
            (BinaryOp::BitAnd, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a & b)),
            (BinaryOp::BitOr, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a | b)),
            (BinaryOp::BitXor, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a ^ b)),
            (BinaryOp::Shl, ConstValue::Int(a), ConstValue::Int(b)) => {
                if b < 0 { None } else { Some(ConstValue::Int(a << (b as u32 % 64))) }
            }
            (BinaryOp::Shr, ConstValue::Int(a), ConstValue::Int(b)) => {
                if b < 0 { None } else { Some(ConstValue::Int(a >> (b as u32 % 64))) }
            }
            (BinaryOp::UShr, ConstValue::Int(a), ConstValue::Int(b)) => {
                if b < 0 { None } else { Some(ConstValue::Int(((a as u64) >> (b as u32 % 64)) as i64)) }
            }
            // Comparações
            (BinaryOp::Lt, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a < b)),
            (BinaryOp::LtEq, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a <= b)),
            (BinaryOp::Gt, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a > b)),
            (BinaryOp::GtEq, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a >= b)),
            (BinaryOp::Eq, l, r) => Some(ConstValue::Bool(l == r)),
            (BinaryOp::NotEq, l, r) => Some(ConstValue::Bool(l != r)),
            // Lógica booleana
            (BinaryOp::And, ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(a && b)),
            (BinaryOp::Or, ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(a || b)),
            _ => None,
        }
    }

    fn evaluate_special_call(
        &mut self,
        unit_id: UnitId,
        target: ast::ExprId,
        args: &ast::Arguments,
    ) -> Option<ConstValue> {
        let expr = &self.program.unit(unit_id).ast.exprs[target.0 as usize];

        // identical(a, b)
        if let ast::ExprKind::Identifier(name) = &expr.kind {
            if self.interner.resolve(name.sym) == "identical" && args.args.len() == 2 {
                let v1 = self.evaluate_expr(unit_id, args.args[0].value)?;
                let v2 = self.evaluate_expr(unit_id, args.args[1].value)?;
                return Some(ConstValue::Bool(v1 == v2));
            }
        }

        // String.fromEnvironment / int.fromEnvironment / bool.fromEnvironment
        if let ast::ExprKind::Property { target: recv, name, .. } = &expr.kind {
            let recv_expr = &self.program.unit(dartforge_elements::model::UnitId(unit_id.0)).ast.exprs[recv.0 as usize];
            if let ast::ExprKind::Identifier(recv_name) = &recv_expr.kind {
                let recv_str = self.interner.resolve(recv_name.sym);
                let method_str = self.interner.resolve(name.sym);

                if method_str == "fromEnvironment" && !args.args.is_empty() {
                    let key_val = self.evaluate_expr(unit_id, args.args[0].value)?;
                    let key_str = match key_val {
                        ConstValue::String(s) => s.as_str().unwrap_or("").to_string(),
                        _ => return None,
                    };

                    let default_arg = args.args.iter().find(|a| {
                        a.name.as_ref().is_some_and(|n| self.interner.resolve(n.sym) == "defaultValue")
                    });

                    match recv_str {
                        "String" => {
                            if let Some(val) = self.env_variables.get(&key_str) {
                                let mut b = DartStrBuilder::default();
                                for u in val.encode_utf16() {
                                    b.push_unit(u);
                                }
                                return Some(ConstValue::String(b.finish()));
                            }
                            if let Some(def) = default_arg {
                                return self.evaluate_expr(unit_id, def.value);
                            }
                            return Some(ConstValue::Null);
                        }
                        "int" => {
                            if let Some(val) = self.env_variables.get(&key_str).and_then(|s| s.parse::<i64>().ok()) {
                                return Some(ConstValue::Int(val));
                            }
                            if let Some(def) = default_arg {
                                return self.evaluate_expr(unit_id, def.value);
                            }
                            return Some(ConstValue::Int(0));
                        }
                        "bool" => {
                            if let Some(val) = self.env_variables.get(&key_str) {
                                return Some(ConstValue::Bool(val == "true"));
                            }
                            if let Some(def) = default_arg {
                                return self.evaluate_expr(unit_id, def.value);
                            }
                            return Some(ConstValue::Bool(false));
                        }
                        _ => {}
                    }
                }
            }
        }

        None
    }
}
