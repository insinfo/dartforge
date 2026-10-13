//! O valor de uma variável `const` de tipo `int`, `double` ou `bool` cujo
//! inicializador é um literal (ou aritmética de inteiros sobre literais e
//! outras dessas constantes): a leitura vira a constante, sem o getter
//! preguiçoso do global (a bandeira, o valor e a chamada de inicialização).
//!
//! É o que o CFE faz com toda constante (a leitura de `static const int X`
//! é um `ConstantExpression` no kernel): o `switch (_state) { case
//! _State.START: … }` do `_HttpParser._doParse` compara com imediatos, e o
//! LLVM o transforma numa tabela de saltos, em vez de uma cadeia de leituras
//! de globais — antes, cada byte da requisição passava por uma dezena delas.
//! O `String` fica de fora (o literal canônico tem identidade própria).

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
    /// A constante no lugar da leitura do global `vid`, quando ele é uma
    /// `const` primitiva (ver o módulo).
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

    /// A chave estrutural (`constants.rs`) do valor de uma `const`
    /// primitiva: `const [top]` é a mesma constante que `const [7]`.
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
        // Só quando o tipo da variável é exatamente o do valor: `const
        // double x = 1` (o literal integer vira `double` pelo contexto) e
        // `const num`/`Object` ficam com o getter.
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
                let t = u.source[s.start as usize..s.end as usize].replace('_', "");
                let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    u64::from_str_radix(h, 16).ok()? as i64
                } else {
                    t.parse::<u64>().ok()? as i64
                };
                Some(Primitive::Int(v))
            }
            ExprKind::Double(s) => {
                let t = u.source[s.start as usize..s.end as usize].replace('_', "");
                Some(Primitive::Double(t.parse().ok()?))
            }
            ExprKind::Bool(b) => Some(Primitive::Bool(*b)),
            ExprKind::Parenthesized(x) => self.evaluate_primitive(unit, *x, prof),
            ExprKind::Unary { op: UnaryOp::Neg, operand } => match self.evaluate_primitive(unit, *operand, prof)? {
                Primitive::Int(v) => Some(Primitive::Int(v.wrapping_neg())),
                Primitive::Double(v) => Some(Primitive::Double(-v)),
                Primitive::Bool(_) => None,
            },
            ExprKind::Binary { op, left, right } => {
                let (Primitive::Int(a), Primitive::Int(b)) =
                    (self.evaluate_primitive(unit, *left, prof)?, self.evaluate_primitive(unit, *right, prof)?)
                else {
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
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let alvo = match self.ctx.get_resolved(unit, e)? {
                    Resolved::Element(Element::Variable(v)) => *v,
                    Resolved::Member { member: MemberRef::Variable(v), .. } => *v,
                    Resolved::Member { member: MemberRef::Function(f), .. }
                    | Resolved::Element(Element::Function(f)) => self.ctx.program.functions[f.0 as usize].variable?,
                    _ => return None,
                };
                let var = &self.ctx.program.variables[alvo.0 as usize];
                if !(var.static_ || var.class.is_none()) {
                    return None;
                }
                self.primitive_const_value(alvo, prof + 1)
            }
            _ => None,
        }
    }
}
