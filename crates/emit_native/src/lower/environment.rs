//! Native compilation environment. This is independent of the process environment
//! and of analyzer constant evaluation, which intentionally keeps queries unknown.

use super::fn_builder::FnBuilder;
use crate::hir::Constant;
use dartforge_elements::model::{Element, FunctionElementId, UnitId, VariableRef};
use dartforge_frontend::ast::{self, ExprKind};
use dartforge_types::resolved::{MemberRef, Resolved};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The public core environment factory, including its SDK patch implementation.
    pub(super) fn environment_factory(&self, fid: FunctionElementId) -> Option<(&'static str, bool)> {
        let f = self.ctx.program.function(self.ctx.program.public(fid));
        if !f.const_ || Some(f.library) != self.ctx.core.core_library { return None; }
        let class = f.class?;
        let name = self.ctx.symbol_name(f.name);
        let kind = if Some(class) == self.ctx.core.bool_class { "bool" }
            else if Some(class) == self.ctx.core.int_class { "int" }
            else if Some(class) == self.ctx.core.string_class { "String" }
            else { return None; };
        match name {
            "fromEnvironment" => Some((kind, false)),
            "hasEnvironment" if kind == "bool" => Some((kind, true)),
            _ => None,
        }
    }

    /// The native definitions: `dartforge.compiler` (true) and the VM's profile
    /// keys — `dart.vm.product` (`Context::product`: true in
    /// `DartFlags::PRODUCTION`, as in `dart compile exe`; false in development,
    /// as in `dart run`) and `dart.vm.profile` (false). `hasEnvironment` is
    /// true for the three in both profiles, as in the VM. An integer query
    /// cannot parse `true`/`false` and therefore uses its default; other keys
    /// remain undefined.
    pub(super) fn environment_value(&self, unit: UnitId, key: ast::ExprId, kind: &str, has: bool) -> Option<Constant> {
        let value = match self.environment_key(unit, key, 0)?.as_str() {
            "dartforge.compiler" => true,
            "dart.vm.product" => self.ctx.product,
            "dart.vm.profile" => false,
            _ => return if has { Some(Constant::Bool(false)) } else { None },
        };
        if has { return Some(Constant::Bool(true)); }
        match kind {
            "bool" => Some(Constant::Bool(value)),
            "String" => Some(Constant::String(if value { "true" } else { "false" }.into())),
            _ => None,
        }
    }

    pub(super) fn environment_key(&self, unit: UnitId, e: ast::ExprId, depth: u32) -> Option<String> {
        if depth > 32 { return None; }
        let ast = &self.ctx.program.unit(unit).ast;
        match &ast.expr(e).kind {
            ExprKind::String(s) => {
                if let Some(value) = s.constant_value() { return Some(value.as_str()?.to_string()); }
                let mut text = String::new();
                for part in &s.parts {
                    match part {
                        ast::StringPart::Text(value) => text.push_str(value.as_str()?),
                        ast::StringPart::Interpolation(value) => {
                            if let Some(value) = self.environment_key(unit, *value, depth + 1) { text.push_str(&value); }
                            else {
                                let value = self.environment_boolean(unit, *value, depth + 1)?; text.push_str(if value { "true" } else { "false" }); }
                        }
                    }
                }
                Some(text)
            }
            ExprKind::Conditional { condition, then, else_ } => {
                let branch = if self.environment_boolean(unit, *condition, depth + 1)? { then } else { else_ };
                self.environment_key(unit, *branch, depth + 1)
            },
            ExprKind::Parenthesized(x) => self.environment_key(unit, *x, depth + 1),
            ExprKind::Binary { op: ast::BinaryOp::Add, left, right } => {
                Some(self.environment_key(unit, *left, depth + 1)? + &self.environment_key(unit, *right, depth + 1)?)
            }
            ExprKind::Identifier(n) if unit == self.unit_id && self.local_const_keys.contains_key(&n.sym) => {
                let (_, init) = self.local_const_keys.get(&n.sym)?;
                self.environment_key(unit, *init, depth + 1)
            }
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let vid = match self.ctx.get_resolved(unit, e)? {
                    Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => *v,
                    Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => self.ctx.program.function(*f).variable?,
                    _ => return None,
                };
                let var = self.ctx.program.variable(vid);
                if !var.const_ { return None; }
                let other = match var.node { VariableRef::Field { unit, .. } | VariableRef::TopLevel { unit, .. } => unit, _ => return None };
                self.environment_key(other, self.variable_initializer_in(vid)?, depth + 1)
            }
            _ => None,
        }
    }

    fn environment_boolean(&self, unit: UnitId, e: ast::ExprId, depth: u32) -> Option<bool> {
        if depth > 32 { return None; }
        let ast = &self.ctx.program.unit(unit).ast;
        match &ast.expr(e).kind {
            ExprKind::Bool(value) => Some(*value),
            ExprKind::Parenthesized(inner) => self.environment_boolean(unit, *inner, depth + 1),
            ExprKind::Unary { op: ast::UnaryOp::Not, operand } => self.environment_boolean(unit, *operand, depth + 1).map(|value| !value),
            ExprKind::Identifier(n) if unit == self.unit_id && self.local_const_keys.contains_key(&n.sym) => {
                let (_, init) = self.local_const_keys.get(&n.sym)?;
                self.environment_boolean(unit, *init, depth + 1)
            }
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                let Resolved::Constructor(fid) = self.ctx.get_resolved(unit, e)? else { return None; };
                let (kind, has) = self.environment_factory(*fid)?;
                if kind != "bool" { return None; }
                let key = arguments.args.iter().find(|a| a.name.is_none())?.value;
                self.environment_key(unit, key, depth + 1)?;
                match self.environment_value(unit, key, kind, has) {
                    Some(Constant::Bool(value)) => Some(value),
                    _ => arguments.args.iter().find(|a| a.name.is_some_and(|n| self.ctx.symbol_name(n.sym) == "defaultValue"))
                        .map_or(Some(false), |a| self.environment_boolean(unit, a.value, depth + 1)),
                }
            }
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let vid = match self.ctx.get_resolved(unit, e)? {
                    Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => *v,
                    Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => self.ctx.program.function(*f).variable?,
                    _ => return None,
                };
                let var = self.ctx.program.variable(vid);
                if !var.const_ { return None; }
                let other = match var.node { VariableRef::Field { unit, .. } | VariableRef::TopLevel { unit, .. } => unit, _ => return None };
                self.environment_boolean(other, self.variable_initializer_in(vid)?, depth + 1)
            }
            _ => None,
        }
    }

}
