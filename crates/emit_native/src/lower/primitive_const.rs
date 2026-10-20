//! The value of a `const` variable of type `int`, `double` or `bool` whose
//! initializer is a literal (or integer arithmetic over literals and
//! other such constants): the read becomes the constant, without the lazy
//! getter of the global (the flag, the value and the initialization call).
//!
//! It is what the CFE does with every constant (the read of `static const int X`
//! is a `ConstantExpression` in the kernel): the `switch (_state) { case
//! _State.START: … }` of `_HttpParser._doParse` compares against immediates, and
//! LLVM turns it into a jump table, instead of a chain of reads of
//! globals — before, each byte of the request went through a dozen of them.
//! `String` is left out (the canonical literal has its own identity).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_elements::model::{Element, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, BinaryOp, ExprKind, UnaryOp};
use dartforge_types::resolved::{MemberRef, Resolved};

#[derive(Clone, Copy)]
enum Primitive {
    Int(i64),
    Double(f64),
    Bool(bool),
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The constant in place of the read of the global `vid`, when it is a
    /// primitive `const` (see the module).
    pub(super) fn read_primitive_const(&mut self, vid: VariableId) -> Option<Operand> {
        let (c, ty) = match self.primitive_const_value(vid, 0)? {
            Primitive::Int(v) => (Constant::Int(v), Type::I64),
            Primitive::Double(v) => (Constant::Double(v), Type::F64),
            Primitive::Bool(v) => (Constant::Bool(v), Type::I1),
        };
        let repr = self.repr(super::members::variable_type(self.ctx, vid));
        let v = self.emit(Instruction::Const(c), ty);
        Some(self.coerce(v, repr))
    }

    /// The structural key (`constants.rs`) of the value of a primitive
    /// `const`: `const [top]` is the same constant as `const [7]`.
    pub(super) fn primitive_const_key(&self, vid: VariableId) -> Option<String> {
        Some(match self.primitive_const_value(vid, 0)? {
            Primitive::Int(v) => format!("i:{v}"),
            Primitive::Double(v) => format!("d:{}", v.to_bits()),
            Primitive::Bool(v) => format!("b:{v}"),
        })
    }

    fn primitive_const_value(&self, vid: VariableId, prof: u32) -> Option<Primitive> {
        let var = &self.ctx.program.variables[vid.0 as usize];
        if !var.const_ || var.late || var.external || prof > 16 {
            return None;
        }
        let unit = match var.node {
            VariableRef::Field { unit, .. } | VariableRef::TopLevel { unit, .. } => unit,
            _ => return None,
        };
        let init = self.variable_initializer_in(vid)?;
        let v = self.evaluate_primitive(unit, init, prof)?;
        // Only when the type of the variable is exactly that of the value: `const
        // double x = 1` (the integer literal becomes `double` from the context) and
        // `const num`/`Object` keep the getter.
        let t = super::members::variable_type(self.ctx, vid);
        let core = &self.ctx.core;
        match v {
            Primitive::Int(_) if t == core.int => Some(v),
            Primitive::Double(_) if t == core.double => Some(v),
            Primitive::Bool(_) if t == core.bool_ => Some(v),
            _ => None,
        }
    }

    fn evaluate_primitive(&self, unit: UnitId, e: ast::ExprId, prof: u32) -> Option<Primitive> {
        let u = self.ctx.program.unit(unit);
        let expr = u.ast.expr(e);
        match &expr.kind {
            ExprKind::Int(s) => {
                let t = u.source[s.start..s.end].replace('_', "");
                let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    u64::from_str_radix(h, 16).ok()? as i64
                } else {
                    t.parse::<u64>().ok()? as i64
                };
                Some(Primitive::Int(v))
            }
            ExprKind::Double(s) => {
                let t = u.source[s.start..s.end].replace('_', "");
                Some(Primitive::Double(t.parse().ok()?))
            }
            ExprKind::Bool(b) => Some(Primitive::Bool(*b)),
            ExprKind::Parenthesized(x) => self.evaluate_primitive(unit, *x, prof),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } => match self.evaluate_primitive(unit, *operand, prof)? {
                Primitive::Int(v) => Some(Primitive::Int(v.wrapping_neg())),
                Primitive::Double(v) => Some(Primitive::Double(-v)),
                Primitive::Bool(_) => None,
            },
            ExprKind::Binary { op, left, right } => {
                let (Primitive::Int(a), Primitive::Int(b)) = (
                    self.evaluate_primitive(unit, *left, prof)?,
                    self.evaluate_primitive(unit, *right, prof)?,
                ) else {
                    return None;
                };
                let r = match op {
                    BinaryOp::Add => a.wrapping_add(b),
                    BinaryOp::Sub => a.wrapping_sub(b),
                    BinaryOp::Mul => a.wrapping_mul(b),
                    BinaryOp::BitAnd => a & b,
                    BinaryOp::BitOr => a | b,
                    BinaryOp::BitXor => a ^ b,
                    BinaryOp::Shl if (0..64).contains(&b) => a.wrapping_shl(b as u32),
                    BinaryOp::Shr if (0..64).contains(&b) => a >> b,
                    _ => return None,
                };
                Some(Primitive::Int(r))
            }
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                let Resolved::Constructor(fid) = self.ctx.get_resolved(unit, e)? else {
                    return None;
                };
                let (kind, has) = self.environment_factory(*fid)?;
                let key = arguments.args.iter().find(|a| a.name.is_none())?.value;
                self.environment_key(unit, key, 0)?;
                match self.environment_value(unit, key, kind, has) {
                    Some(Constant::Bool(value)) => Some(Primitive::Bool(value)),
                    _ => {
                        if let Some(default) = arguments.args.iter().find(|a| {
                            a.name
                                .is_some_and(|n| self.ctx.symbol_name(n.sym) == "defaultValue")
                        }) {
                            self.evaluate_primitive(unit, default.value, prof + 1)
                        } else {
                            match kind {
                                "bool" => Some(Primitive::Bool(false)),
                                "int" => Some(Primitive::Int(0)),
                                _ => None,
                            }
                        }
                    }
                }
            }
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let target = match self.ctx.get_resolved(unit, e)? {
                    Resolved::Element(Element::Variable(v)) => *v,
                    Resolved::Member {
                        member: MemberRef::Variable(v),
                        ..
                    } => *v,
                    Resolved::Member {
                        member: MemberRef::Function(f),
                        ..
                    }
                    | Resolved::Element(Element::Function(f)) => {
                        self.ctx.program.functions[f.0 as usize].variable?
                    }
                    _ => return None,
                };
                let var = &self.ctx.program.variables[target.0 as usize];
                if !(var.static_ || var.class.is_none()) {
                    return None;
                }
                self.primitive_const_value(target, prof + 1)
            }
            _ => None,
        }
    }
}
