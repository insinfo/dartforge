//! Proven interpolation values, preserving Dart UTF-16 units and unknowns.

use super::Context;
use dartforge_elements::model::{LibraryId, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_frontend::text::DartStrBuilder;
use dartforge_types::const_eval::evaluation::{Constant, Ctx, Engine};
use dartforge_types::const_eval::value::InstanceState;
use std::collections::{HashMap, HashSet};

impl Context<'_> {
    /// Evaluates only language constants; getters, unknown environment values and
    /// double interpolation keep their ordinary runtime lowering.
    pub(crate) fn constant_string(&self, unit: UnitId, expression: ExprId) -> Option<&[u8]> {
        self.constant_strings
            .get_or_init(|| {
                let inferred: HashSet<_> = self
                    .compiled
                    .iter()
                    .enumerate()
                    .filter_map(|(i, compiled)| compiled.then_some(LibraryId(i as u32)))
                    .collect();
                let mut table = self.table.clone();
                let mut engine = Engine::new(
                    self.program,
                    self.interner,
                    &mut table,
                    self.core,
                    self.outline,
                    self.bodies,
                    &inferred,
                );
                engine.conservative_string_interpolation = true;
                let mut values = HashMap::new();
                for (i, source) in self.program.units.iter().enumerate() {
                    if !self.library_in_module(source.library) {
                        continue;
                    }
                    let unit = UnitId(i as u32);
                    let context = Ctx::simple(unit, source.library);
                    for (i, expression) in source.ast.exprs.iter().enumerate() {
                        let ExprKind::String(literal) = &expression.kind else {
                            continue;
                        };
                        if literal.constant_value().is_some() {
                            continue;
                        }
                        let id = ExprId(i as u32);
                        let Constant::Value(value) = engine.evaluate(&context, id, false) else {
                            continue;
                        };
                        let InstanceState::Str(Some(units)) = value.state else {
                            continue;
                        };
                        let mut text = DartStrBuilder::with_capacity(units.len());
                        for &unit in units.iter() {
                            text.push_unit(unit);
                        }
                        values.insert((unit, id), text.finish().as_bytes().to_vec());
                    }
                }
                values
            })
            .get(&(unit, expression))
            .map(Vec::as_slice)
    }
}
