//! The conversion of a diagnostic's arguments into text, like the analyzer's
//! `ErrorReporter` (`analyzer/lib/error/listener.dart:338-426`;
//! docs/ANALYZER-ESPECIFICACAO.md §G, T7): an argument that is a type is
//! output through `getDisplayString(preferTypeAlias: true)`, and two type
//! arguments of the same report with the **same text** and different
//! elements of the same name get the suffix
//! `(where X is defined in <arquivo>)`.
//!
//! The text of a type is still that of [`TypeTable::format`] (with alias)
//! and [`TypeTable::format_sem_alias`]; this module is the single point
//! through which the arguments of a report pass together.

use crate::table::{Type, TypeId, TypeTable};
use dartforge_elements::model::{ClassId, Program};
use dartforge_intern::{Interner, SymbolId};
use std::borrow::Cow;
use std::collections::HashMap;

/// A template argument before conversion.
#[derive(Debug, Clone)]
pub enum Arg<'a> {
    /// A `DartType`: with alias, and disambiguated against the other types of
    /// the same report.
    Tipo(TypeId),
    /// A text that the official emitter took from `getDisplayString()` before
    /// passing it: without alias and without disambiguation.
    TypeWithoutAlias(TypeId),
    /// `String`, `int` or element name: as it came.
    Texto(Cow<'a, str>),
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(s: &'a str) -> Self {
        Arg::Texto(Cow::Borrowed(s))
    }
}

impl From<String> for Arg<'_> {
    fn from(s: String) -> Self {
        Arg::Texto(Cow::Owned(s))
    }
}

/// The formatter of a report's arguments.
pub struct Exibidor<'a> {
    pub table: &'a TypeTable,
    pub interner: &'a Interner,
    pub program: &'a Program,
}

impl Exibidor<'_> {
    /// `getDisplayString(preferTypeAlias: alias)`.
    pub fn tipo(&self, t: TypeId, alias: bool) -> String {
        if alias {
            self.table.format(t, self.interner, self.program)
        } else {
            self.table.format_sem_alias(t, self.interner, self.program)
        }
    }

    /// `_convertTypeNames`: the text of each argument, in order.
    pub fn argumentos(&self, args: &[Arg<'_>]) -> Vec<String> {
        self.arguments_and_context(args).0
    }

    /// The whole `_convertTypeNames`: the texts and the context messages
    /// (`listener.dart:408-413`): one per element of each type of the groups of
    /// two or more arguments with the same text — even the elements that do not
    /// collide —, `<name> is defined in <caminho>`, at the element name, in its
    /// file.
    pub fn arguments_and_context(&self, args: &[Arg<'_>]) -> (Vec<String>, Vec<dartforge_diagnostics::Contexto>) {
        let mut context: Vec<dartforge_diagnostics::Contexto> = Vec::new();
        let mut output: Vec<String> = args
            .iter()
            .map(|a| match a {
                Arg::Tipo(t) => self.tipo(*t, true),
                Arg::TypeWithoutAlias(t) => self.tipo(*t, false),
                Arg::Texto(s) => s.to_string(),
            })
            .collect();
        // The type arguments grouped by text, in the order they appear.
        let mut groups: Vec<(String, Vec<(usize, TypeId)>)> = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let Arg::Tipo(t) = a else { continue };
            let position = groups.iter().position(|(name, _)| *name == output[i]);
            match position {
                Some(k) => groups[k].1.push((i, *t)),
                None => groups.push((output[i].clone(), vec![(i, *t)])),
            }
        }
        for (name, members) in groups {
            if members.len() < 2 {
                continue;
            }
            // The elements of all types of the group, by name.
            let mut by_name: HashMap<SymbolId, Vec<ClassId>> = HashMap::new();
            let mut elements_of: Vec<Vec<ClassId>> = Vec::with_capacity(members.len());
            for &(_, t) in &members {
                let mut els = Vec::new();
                self.elementos(t, &mut els, 0);
                for &c in &els {
                    let list = by_name.entry(self.program.class(c).name).or_default();
                    if !list.contains(&c) {
                        list.push(c);
                    }
                }
                elements_of.push(els);
            }
            for (k, &(i, _)) in members.iter().enumerate() {
                let mut suffix: Option<String> = None;
                for &c in &elements_of[k] {
                    let element_name = self.program.class(c).name;
                    let snippet = format!("{} is defined in {}", self.interner.resolve(element_name), self.file_of(c));
                    // The context message is output for every element (outside the `if` in
                    // the analyzer).
                    if let Some(span) = self.nome_da_classe(c) {
                        context.push(dartforge_diagnostics::Contexto { arquivo: Some(self.file_of(c).into()), span, mensagem: snippet.clone().into() });
                    }
                    if by_name.get(&element_name).is_none_or(|l| l.len() < 2) {
                        continue;
                    }
                    suffix = Some(match suffix {
                        None => format!("where {snippet}"),
                        Some(s) => format!("{s}, {snippet}"),
                    });
                }
                if let Some(s) = suffix {
                    output[i] = format!("{name} ({s})");
                }
            }
        }
        (output, context)
    }

    /// The range of the class name (`nameOffset`/`nameLength`); none for a
    /// synthetic one.
    fn nome_da_classe(&self, c: ClassId) -> Option<dartforge_diagnostics::Span> {
        use dartforge_frontend::ast::DeclKind;
        let d = self.program.class(c).decl?;
        let n = match &self.program.unit(d.unit).ast.decl(d.decl).kind {
            DeclKind::Class(x) => x.name,
            DeclKind::Mixin(x) => x.name,
            DeclKind::Enum(x) => x.name,
            DeclKind::ExtensionType(x) => x.name,
            _ => return None,
        };
        Some(n.span)
    }

    /// `_TypeToConvert.allElements`: the classes (enums, mixins, extension
    /// types) reached through the return type and the parameters of a function
    /// type and through the type arguments of an interface, in visiting order
    /// and without repetition; never inside a record. An element already seen
    /// is not walked again.
    fn elementos(&self, t: TypeId, out: &mut Vec<ClassId>, prof: u32) {
        if prof > 32 {
            return;
        }
        match self.table.get(t) {
            Type::Function { ret, positional, optional, named, .. } => {
                self.elementos(*ret, out, prof + 1);
                for &p in positional.iter().chain(optional.iter()) {
                    self.elementos(p, out, prof + 1);
                }
                // The named ones in the analyzer's order (alphabetical).
                let mut order: Vec<&(SymbolId, TypeId, bool)> = named.iter().collect();
                order.sort_by(|a, b| self.interner.resolve(a.0).cmp(self.interner.resolve(b.0)));
                for (_, p, _) in order {
                    self.elementos(*p, out, prof + 1);
                }
            }
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => {
                if out.contains(class) {
                    return;
                }
                // An unnamed element does not count, but its arguments do.
                out.push(*class);
                for &a in args.iter() {
                    self.elementos(a, out, prof + 1);
                }
            }
            // `FutureOr<T>` is a `dart:async` interface for the analyzer: the element
            // is always the same, only the argument matters.
            Type::FutureOr { arg, .. } => self.elementos(*arg, out, prof + 1),
            _ => {}
        }
    }

    /// `element.source.fullName`: the native path of the file that declares the
    /// class (the URI, for what has no file).
    fn file_of(&self, c: ClassId) -> String {
        let cls = self.program.class(c);
        let unit_id = match cls.decl {
            Some(d) => Some(d.unit),
            None => self.program.library(cls.library).units.first().copied(),
        };
        match unit_id {
            Some(u) => {
                let unit = self.program.unit(u);
                match &unit.path {
                    Some(p) => p.display().to_string(),
                    None => unit.uri.clone(),
                }
            }
            None => String::new(),
        }
    }
}
