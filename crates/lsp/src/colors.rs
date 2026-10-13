//! `textDocument/documentColor` and `textDocument/colorPresentation`
//! (docs/LSP-ESPECIFICACAO.md §8.2): the 3.6.2 `ColorComputer`
//! (`AS:src/computer/computer_color.dart`) and the
//! `DocumentColorPresentationHandler`
//! (`handler_document_color_presentation.dart`).
//!
//! An expression whose static type is `dart:ui`'s `Color` (or a subtype) has
//! a color when its constant value (or a member of it, or a palette index)
//! yields the `a`, `r`, `g` and `b` fields, or when it is a known call
//! (`Color(0x…)`, `Color.from`, `Color.fromARGB`, `Color.fromRGBO`,
//! `ColorSwatch(0x…)`, `MaterialAccentColor(0x…)`) with literals. The
//! visitor's rules: at an index (`Colors.red[500]`), at a prefixed
//! identifier (`A.b`, whole, or the member `b` of the value of `A`) and at a
//! property access (the member of the target's value) a found color stops
//! the descent; in a creation, the arguments are still visited.
//!
//! Written without compiling or running (2026-10-05).

use crate::project::Project;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, LibraryId, UnitId};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_frontend::parents::{Parent, Parents};
use dartforge_types::constantes::avaliador::{Constante, Ctx, Motor};
use dartforge_types::constantes::valor::{Campo, Estado, Valor};
use dartforge_types::{Resolved, Type};
use std::collections::HashSet;

/// ARGB from 0 to 255.
pub(crate) type Color = [u8; 4];

/// `getColorForInt`.
fn int_color(v: i64) -> Color {
    [((v >> 24) & 0xff) as u8, ((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8]
}

/// `getColorForDoubles`: `(x * 255.0).round() & 0xff`.
fn doubles_color(a: f64, r: f64, g: f64, b: f64) -> Color {
    let c = |x: f64| ((x * 255.0).round() as i64 & 0xff) as u8;
    [c(a), c(r), c(g), c(b)]
}

/// The value of an integer literal (decimal or hexadecimal, with `_`).
fn integer(source: &str, s: Span) -> Option<i64> {
    let t: String = source.get(s.start..s.end)?.chars().filter(|c| *c != '_').collect();
    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(h, 16).ok().map(|v| v as i64),
        None => t.parse().ok(),
    }
}

fn double(source: &str, s: Span) -> Option<f64> {
    source.get(s.start..s.end)?.replace('_', "").parse().ok()
}

impl Project {
    /// The class is `dart:ui`'s `Color` or a subtype of it (`isColor`).
    fn is_color(&self, c: ClassId) -> bool {
        let p = self.program();
        let mut stack = vec![c];
        let mut seen = HashSet::new();
        while let Some(x) = stack.pop() {
            if !seen.insert(x) {
                continue;
            }
            let cl = p.class(x);
            if self.name(cl.name) == "Color" && p.library(cl.library).uri == "dart:ui" {
                return true;
            }
            stack.extend(cl.supertype_class);
            stack.extend(cl.interface_classes.iter().copied());
            stack.extend(cl.mixin_classes.iter().copied());
        }
        false
    }

    /// The static type of `e` is a color.
    fn color_type(&self, u: UnitId, e: ExprId) -> bool {
        let Some(t) = self.query.bodies.units[u.0 as usize].get_type(e) else { return false };
        match self.query.table.get(t) {
            Type::Interface { class, .. } => self.is_color(*class),
            _ => false,
        }
    }

    /// The libraries that evaluating the colors of `u` needs inferred, besides
    /// those already inferred: those of the constant variables that the color
    /// expressions cite and those of the `Color`s of `dart:ui` and Flutter.
    pub(crate) fn color_libraries(&self, u: UnitId) -> Vec<LibraryId> {
        let p = self.program();
        let mut v: Vec<LibraryId> = Vec::new();
        let bodies = &self.query.bodies.units[u.0 as usize];
        for i in 0..p.unit(u).ast.exprs.len() {
            let e = ExprId(i as u32);
            if !self.color_type(u, e) {
                continue;
            }
            let var = match bodies.get_resolved(e) {
                Some(Resolved::Element(dartforge_elements::model::Element::Variable(x))) => Some(*x),
                Some(Resolved::Member { member: dartforge_types::MemberRef::Variable(x), .. }) => Some(*x),
                Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) | Some(Resolved::Member { member: dartforge_types::MemberRef::Function(f), .. }) => p.function(*f).variable,
                _ => None,
            };
            if let Some(x) = var {
                v.push(p.variable(x).library);
            }
        }
        for (i, l) in p.libraries.iter().enumerate() {
            if matches!(
                l.uri.as_str(),
                "dart:ui" | "package:flutter/src/painting/colors.dart" | "package:flutter/src/material/colors.dart" | "package:flutter/src/cupertino/colors.dart"
            ) {
                v.push(LibraryId(i as u32));
            }
        }
        v.sort();
        v.dedup();
        v.retain(|l| !self.libraries.contains(l));
        v
    }

    /// The colors of `u` (`ColorComputer.compute`), in text order.
    pub(crate) fn colors(&mut self, u: UnitId) -> Vec<(Span, Color)> {
        let lib = self.program().unit(u).library;
        let before_3 = self.program().library(lib).features.version().major < 3;
        let parents = {
            let un = self.program().unit(u);
            Parents::new(&un.ast, &un.unit, &un.source, before_3)
        };
        let n = self.program().unit(u).ast.exprs.len();
        // The attempt for each node (without the ancestors), and whether it stops
        // the descent.
        let mut found: Vec<(ExprId, Span, Color, bool)> = Vec::new();
        let inferred: HashSet<LibraryId> = self.libraries.iter().copied().collect();
        for i in 0..n {
            let e = ExprId(i as u32);
            if !self.color_type(u, e) {
                continue;
            }
            let (span, kind) = {
                let a = &self.program().unit(u).ast;
                let x = a.expr(e);
                let bodies = &self.query.bodies.units[u.0 as usize];
                (x.span, superficial_shape(a, e, matches!(bodies.get_resolved(e), Some(Resolved::Constructor(_)))))
            };
            let in_const = parents.in_constant_context(&self.program().unit(u).ast, e);
            let color = match kind {
                Superficial::Index { target, index } => {
                    let source = &self.program().unit(u).source;
                    let index_ = match &self.program().unit(u).ast.expr(index).kind {
                        ExprKind::Int(s) => integer(source, *s),
                        _ => None,
                    };
                    index_.and_then(|k| self.color_by_value(u, &inferred, target, in_const, Member::Index(k))).map(|c| (c, true))
                }
                Superficial::Creation => {
                    let c = self.color_by_value(u, &inferred, e, in_const, Member::None).or_else(|| self.known_constructor_color(u, e));
                    c.map(|c| (c, false))
                }
                Superficial::Prefixed { target, name } => self
                    .color_by_value(u, &inferred, e, in_const, Member::None)
                    .or_else(|| self.color_by_value(u, &inferred, target, in_const, Member::Name(name)))
                    .map(|c| (c, true)),
                Superficial::Property { target, name } => {
                    self.color_by_value(u, &inferred, target, in_const, Member::Name(name)).map(|c| (c, true))
                }
                Superficial::Identifier => self.color_by_value(u, &inferred, e, in_const, Member::None).map(|c| (c, false)),
                Superficial::Other => None,
            };
            if let Some((c, closes)) = color {
                found.push((e, span, c, closes));
            }
        }
        // What lies under a node that found a color and stopped the descent was not
        // visited.
        let closed: HashSet<ExprId> = found.iter().filter(|x| x.3).map(|x| x.0).collect();
        let under_closed = |e: ExprId| {
            let mut current = parents.parent(e);
            for _ in 0..n + 1 {
                match current {
                    Parent::Expr(p) => {
                        if closed.contains(&p) {
                            return true;
                        }
                        current = parents.parent(p);
                    }
                    _ => return false,
                }
            }
            false
        };
        let mut output: Vec<(Span, Color)> = found.iter().filter(|x| !under_closed(x.0)).map(|x| (x.1, x.2)).collect();
        // The visit order: the parent before the child.
        output.sort_by_key(|(s, _)| (s.start, std::cmp::Reverse(s.end)));
        output.dedup_by_key(|(s, _)| s.start);
        output
    }

    /// `tryAddColor`: the constant value of `target` (or the member, or the
    /// palette index) as a color.
    fn color_by_value(&mut self, u: UnitId, inferred: &HashSet<LibraryId>, target: ExprId, in_const: bool, member: Member) -> Option<Color> {
        let lib = self.program().unit(u).library;
        let value = {
            let crate::query::Query { program, names, table, core, outline, bodies, .. } = &mut self.query;
            let mut engine = Motor::novo(program, names, table, core, outline, bodies, inferred);
            match engine.avaliar(&Ctx::simples(u, lib), target, in_const) {
                Constante::Valor(v) if engine.relatos.is_empty() => v,
                _ => return None,
            }
        };
        let value = match member {
            Member::None => value,
            Member::Name(name) => {
                let text = self.name(name).to_string();
                match field(&value, name) {
                    Some(v) => v,
                    None => {
                        let k = text.strip_prefix("shade")?.parse::<i64>().ok()?;
                        self.from_palette(&value, k)?
                    }
                }
            }
            Member::Index(k) => self.from_palette(&value, k)?,
        };
        self.value_color(&value)
    }

    /// `_getSwatchColor`: the entry with key `k` of the `_swatch` field.
    fn from_palette(&self, v: &Valor, k: i64) -> Option<Valor> {
        let symbol = self.query.names.lookup("_swatch")?;
        let palette = field(v, symbol)?;
        let Estado::Mapa { entradas: entries, .. } = &palette.estado else { return None };
        entries.iter().find(|(c, _)| matches!(c.estado, Estado::Int(Some(x)) if x == k)).map(|(_, v)| v.clone())
    }

    /// `getColorForObject`: the `color` field (if any) and `a`, `r`, `g`, `b`.
    fn value_color(&self, v: &Valor) -> Option<Color> {
        match self.query.table.get(v.tipo) {
            Type::Interface { class, .. } if self.is_color(*class) => {}
            _ => return None,
        }
        let names = &self.query.names;
        let v = names.lookup("color").and_then(|s| field(v, s)).unwrap_or_else(|| v.clone());
        let d = |n: &str| -> Option<f64> {
            match field(&v, names.lookup(n)?)?.estado {
                Estado::Double(Some(x)) => Some(x),
                Estado::Int(Some(x)) => Some(x as f64),
                _ => None,
            }
        };
        Some(doubles_color(d("a")?, d("r")?, d("g")?, d("b")?))
    }

    /// `tryAddKnownColorConstructor`: the known calls with literals.
    fn known_constructor_color(&self, u: UnitId, e: ExprId) -> Option<Color> {
        let p = self.program();
        let un = p.unit(u);
        let f = match self.query.bodies.units[u.0 as usize].get_resolved(e) {
            Some(Resolved::Constructor(f)) => *f,
            _ => return None,
        };
        let class = p.function(f).class?;
        let class_name = self.name(p.class(class).name);
        let uri = &p.library(p.class(class).library).uri;
        let constructor = self.name(p.function(f).name);
        let args: &[ast::Argument] = match &un.ast.expr(e).kind {
            ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &arguments.args,
            _ => return None,
        };
        let int = |a: &ast::Argument| match &un.ast.expr(a.value).kind {
            ExprKind::Int(s) => integer(&un.source, *s),
            _ => None,
        };
        let num = |a: &ast::Argument| match &un.ast.expr(a.value).kind {
            ExprKind::Int(s) => integer(&un.source, *s).map(|x| x as f64),
            ExprKind::Double(s) => double(&un.source, *s),
            _ => None,
        };
        if uri == "dart:ui" && class_name == "Color" {
            match constructor {
                "" if args.len() == 1 => int(&args[0]).map(int_color),
                "from" => {
                    let mut x = [None; 4];
                    for a in args.iter().filter(|a| a.name.is_some()) {
                        let i = match self.name(a.name?.sym) {
                            "alpha" => 0,
                            "red" => 1,
                            "green" => 2,
                            "blue" => 3,
                            _ => continue,
                        };
                        x[i] = num(a);
                    }
                    Some(doubles_color(x[0]?, x[1]?, x[2]?, x[3]?))
                }
                "fromARGB" if args.len() == 4 => {
                    let v: Vec<i64> = args.iter().map(int).collect::<Option<_>>()?;
                    Some([v[0] as u8, v[1] as u8, v[2] as u8, v[3] as u8])
                }
                "fromRGBO" if args.len() == 4 => {
                    let rgb: Vec<i64> = args[..3].iter().map(int).collect::<Option<_>>()?;
                    let alpha = (num(&args[3])? * 255.0) as i64;
                    Some([alpha as u8, rgb[0] as u8, rgb[1] as u8, rgb[2] as u8])
                }
                _ => None,
            }
        } else if (uri == "package:flutter/src/painting/colors.dart" && class_name == "ColorSwatch")
            || (uri == "package:flutter/src/material/colors.dart" && class_name == "MaterialAccentColor")
        {
            (constructor.is_empty() && !args.is_empty()).then(|| int(&args[0]).map(int_color)).flatten()
        } else {
            None
        }
    }

    /// `_willRequireConstKeyword`: the expression at `offset` is constant and is
    /// not in a constant context.
    pub(crate) fn requires_const(&self, u: UnitId, offset: usize) -> bool {
        let p = self.program();
        let un = p.unit(u);
        let lib = un.library;
        let parents = Parents::new(&un.ast, &un.unit, &un.source, p.library(lib).features.version().major < 3);
        let Some((i, x)) = un.ast.exprs.iter().enumerate().filter(|(_, x)| x.span.start <= offset && offset <= x.span.end).min_by_key(|(_, x)| x.span.end - x.span.start) else {
            return false;
        };
        let e = ExprId(i as u32);
        if parents.in_constant_context(&un.ast, e) {
            return false;
        }
        match &x.kind {
            ExprKind::InstanceCreation { keyword, .. } => *keyword == Some(ast::CreationKeyword::Const),
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let bodies = &self.query.bodies.units[u.0 as usize];
                let r = match parents.parent(e) {
                    Parent::Expr(parent) if matches!(un.ast.expr(parent).kind, ExprKind::Property { target, .. } if target == e) => bodies.get_resolved(parent),
                    _ => bodies.get_resolved(e),
                };
                let var = match r {
                    Some(Resolved::Element(dartforge_elements::model::Element::Variable(v))) => Some(*v),
                    Some(Resolved::Member { member: dartforge_types::MemberRef::Variable(v), .. }) => Some(*v),
                    Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) | Some(Resolved::Member { member: dartforge_types::MemberRef::Function(f), .. }) => p.function(*f).variable,
                    _ => None,
                };
                var.is_some_and(|v| p.variable(v).const_)
            }
            _ => false,
        }
    }

    /// Flutter's `Color` class (`getFlutterClass('Color')`: the one from
    /// `dart:ui`, which `painting` exports), if the program has it.
    pub(crate) fn has_flutter_color(&self) -> bool {
        let p = self.program();
        p.classes.iter().any(|c| self.name(c.name) == "Color" && p.library(c.library).uri == "dart:ui")
    }
}

/// What to read from the value.
enum Member {
    None,
    Name(dartforge_intern::SymbolId),
    Index(i64),
}

/// The node's shape for the visitor.
enum Superficial {
    Index { target: ExprId, index: ExprId },
    Creation,
    Prefixed { target: ExprId, name: dartforge_intern::SymbolId },
    Property { target: ExprId, name: dartforge_intern::SymbolId },
    Identifier,
    Other,
}

/// The shape of node `e` for the visitor. `constructor`: the call resolves
/// to a constructor (a creation without `new`, which the analyzer rewrites
/// as an `InstanceCreationExpression`).
fn superficial_shape(a: &ast::Ast, e: ExprId, constructor: bool) -> Superficial {
    match &a.expr(e).kind {
        ExprKind::Index { target, index, .. } => Superficial::Index { target: *target, index: *index },
        ExprKind::InstanceCreation { .. } => Superficial::Creation,
        ExprKind::Call { .. } if constructor => Superficial::Creation,
        // `A.b` with `A` a simple identifier is the `PrefixedIdentifier`; the rest,
        // `PropertyAccess`.
        ExprKind::Property { target, name, null_aware: false } if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) => {
            Superficial::Prefixed { target: *target, name: name.sym }
        }
        ExprKind::Property { target, name, .. } => Superficial::Property { target: *target, name: name.sym },
        ExprKind::Identifier(_) => Superficial::Identifier,
        _ => Superficial::Other,
    }
}

/// `getFieldFromHierarchy`: the field by name, walking up through `super`.
fn field(v: &Valor, name: dartforge_intern::SymbolId) -> Option<Valor> {
    let Estado::Generico { campos: fields, .. } = &v.estado else { return None };
    if let Some((_, x)) = fields.iter().find(|(c, _)| *c == Campo::Nome(name)) {
        return Some(x.clone());
    }
    let (_, sup) = fields.iter().find(|(c, _)| *c == Campo::Super)?;
    field(sup, name)
}

/// The four presentations of a color (`_getPresentations`): the label and
/// the text that replaces the range (with `const ` when required).
pub(crate) fn presentations(alpha: f64, red: f64, green: f64, blue: f64, with_const: bool) -> Vec<(String, String)> {
    // `toStringAsFixed(3)` without the trailing zeros and the dot.
    let round = |x: f64| {
        let t = format!("{x:.3}");
        let t = t.trim_end_matches('0');
        t.trim_end_matches('.').to_string()
    };
    let (a, r, g, b) = ((alpha * 255.0) as i64, (red * 255.0) as i64, (green * 255.0) as i64, (blue * 255.0) as i64);
    let value = (a << 24) | (r << 16) | (g << 8) | b;
    let hex = format!("0x{:08X}", value as u32);
    // Dart's `num.toString` for the opacity: `1.0`, `0.5`.
    let opacity = if alpha.fract() == 0.0 { format!("{alpha:.1}") } else { format!("{alpha}") };
    let invocations = [
        format!(".fromARGB({a}, {r}, {g}, {b})"),
        format!(".fromRGBO({r}, {g}, {b}, {opacity})"),
        format!(".from(alpha: {}, red: {}, green: {}, blue: {})", round(alpha), round(red), round(green), round(blue)),
        format!("({hex})"),
    ];
    invocations
        .into_iter()
        .map(|i| (format!("Color{i}"), format!("{}Color{i}", if with_const { "const " } else { "" })))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_presentations() {
        assert_eq!(int_color(0xFF336699), [0xFF, 0x33, 0x66, 0x99]);
        assert_eq!(doubles_color(1.0, 0.5, 0.0, 1.0), [255, 128, 0, 255]);
        let a = presentations(1.0, 0.2, 0.4, 0.6, true);
        assert_eq!(a[0].0, "Color.fromARGB(255, 51, 102, 153)");
        assert_eq!(a[1].0, "Color.fromRGBO(51, 102, 153, 1.0)");
        assert_eq!(a[2].0, "Color.from(alpha: 1, red: 0.2, green: 0.4, blue: 0.6)");
        assert_eq!(a[3].0, "Color(0xFF336699)");
        assert!(a[3].1.starts_with("const Color("));
    }
}
