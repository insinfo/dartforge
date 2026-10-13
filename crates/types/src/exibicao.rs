//! A conversão dos argumentos de um diagnóstico em texto, como o
//! `ErrorReporter` do analyzer (`analyzer/lib/error/listener.dart:338-426`;
//! docs/ANALYZER-ESPECIFICACAO.md §G, T7): um argumento que é tipo sai pelo
//! `getDisplayString(preferTypeAlias: true)`, e dois argumentos-tipo do mesmo
//! relato com o **mesmo texto** e elementos diferentes de mesmo nome ganham
//! o sufixo `(where X is defined in <arquivo>)`.
//!
//! O texto de um tipo continua sendo o de [`TypeTable::format`] (com alias)
//! e [`TypeTable::format_sem_alias`]; este módulo é o ponto único por onde
//! os argumentos de um relato passam juntos.

use crate::table::{Type, TypeId, TypeTable};
use dartforge_elements::model::{ClassId, Program};
use dartforge_intern::{Interner, SymbolId};
use std::borrow::Cow;
use std::collections::HashMap;

/// Um argumento de molde antes da conversão.
#[derive(Debug, Clone)]
pub enum Arg<'a> {
    /// Um `DartType`: com alias, e desambiguado contra os outros tipos do
    /// mesmo relato.
    Tipo(TypeId),
    /// Um texto que o emissor oficial tirou de `getDisplayString()` antes de
    /// passar: sem alias e sem desambiguação.
    TypeWithoutAlias(TypeId),
    /// `String`, `int` ou nome de elemento: como veio.
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

/// O formatador dos argumentos de um relato.
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

    /// `_convertTypeNames`: o texto de cada argumento, na ordem.
    pub fn argumentos(&self, args: &[Arg<'_>]) -> Vec<String> {
        self.arguments_and_context(args).0
    }

    /// `_convertTypeNames` inteiro: os textos e as mensagens de contexto
    /// (`listener.dart:408-413`): uma por elemento de cada tipo dos grupos
    /// de dois ou mais argumentos com o mesmo texto — mesmo os elementos que
    /// não colidem —, `<nome> is defined in <caminho>`, no nome do elemento,
    /// no arquivo dele.
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
        // Os argumentos-tipo agrupados pelo texto, na ordem em que aparecem.
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
            // Os elementos de todos os tipos do grupo, pelo nome.
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
                    // A mensagem de contexto sai para todo elemento (fora do
                    // `if` no analyzer).
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

    /// O intervalo do nome da classe (`nameOffset`/`nameLength`); nenhum na
    /// sintética.
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

    /// `_TypeToConvert.allElements`: as classes (enums, mixins, tipos de
    /// extensão) alcançadas pelo retorno e pelos parâmetros de um tipo de
    /// função e pelos argumentos de tipo de uma interface, na ordem da
    /// visita e sem repetir; nunca por dentro de um record. Um elemento já
    /// visto não é percorrido de novo.
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
                // Os nomeados na ordem do analyzer (alfabética).
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
                // O elemento sem nome não conta, mas os argumentos dele sim.
                out.push(*class);
                for &a in args.iter() {
                    self.elementos(a, out, prof + 1);
                }
            }
            // `FutureOr<T>` é uma interface do `dart:async` para o analyzer:
            // o elemento é sempre o mesmo, só o argumento importa.
            Type::FutureOr { arg, .. } => self.elementos(*arg, out, prof + 1),
            _ => {}
        }
    }

    /// `element.source.fullName`: o caminho nativo do arquivo que declara a
    /// classe (a URI, para o que não tem arquivo).
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
