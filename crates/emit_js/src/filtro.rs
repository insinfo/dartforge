//! Reachability filter for the production profile (`crates/emit_js_production`,
//! `docs/JS-PRODUCAO.md` §1.7).
//!
//! The emitter asks the filter whether a class, function or variable is live, and
//! does not emit what is dead. **Without a filter (`Ctx::filtro == None`), nothing
//! changes**: that is the development profile and `dartforge serve`, byte for byte as
//! before — the `identidade` test of `emit_js_production` compiles the same programs
//! through both paths and compares them.

use dartforge_elements::model::{ClassId, FunctionElementId, VariableId};

/// Reachability level of a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Nivel {
    /// Not emitted.
    Morta,
    /// Identity only: shell, `addRtiResources`, rti rules and live statics.
    Tipo,
    /// Constructed: everything the filter says is live.
    Instanciada,
}

/// State of a function or variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    Viva,
    Morta,
    /// Emitted as a *stub* that reports the call (`dart_podado("…")`): the
    /// pruning checker mode.
    Stub,
}

pub trait Vivos {
    fn classe(&self, c: ClassId) -> Nivel;
    fn funcao(&self, f: FunctionElementId) -> Estado;
    fn variavel(&self, v: VariableId) -> Estado;
    /// Name invoked somewhere (`noSuchMethod` forwarders).
    fn seletor(&self, name: &str) -> bool;
    /// Name written somewhere (`x.name = v`): the `noSuchMethod` forwarders for
    /// abstract setters. Without a filter, always.
    fn seletor_escrita(&self, base_name: &str) -> bool;
    /// The constructor's static tear-off `_#name#tearOff` is referenced.
    fn tearoff_ctor(&self, f: FunctionElementId) -> bool;
    /// The signature entry (`setMethodSignature` and its siblings) of member
    /// `name` is emitted. Only dynamic dispatch (`dsend`/`dload`/`dput`) and the
    /// *tear-off* (`bind`) read signatures (`docs/JS-PRODUCAO-SDK-PROPRIO.md`
    /// §6); the default is to emit all of them.
    fn assinatura(&self, _name: &str) -> bool {
        true
    }
    /// The program's `assert`s are emitted (the production profile turns them
    /// off by default, like `dart2js`).
    fn manter_asserts(&self) -> bool {
        true
    }
    /// Constant boolean value proven for expression `x` of unit `u`
    /// (`dartforge_world::Constantes`): the emitter writes the literal, and the
    /// dead branch of an `if` disappears.
    fn constante_bool(&self, _u: dartforge_elements::model::UnitId, _x: dartforge_frontend::ast::ExprId) -> Option<bool> {
        None
    }
    /// The JS key of a named parameter (production profile:
    /// `docs/JS-PRODUCAO-TAMANHO.md` §8.2); `None` means the name itself.
    fn nome_nomeado(&self, _name: &str) -> Option<&str> {
        None
    }
    /// The map of named parameters as JSON (name → key), when the runtime needs
    /// to translate names at run time (a live `Function.apply`).
    fn tabela_de_nomeados(&self) -> Option<String> {
        None
    }
    /// `--omitir-checagens` (the `omitImplicitChecks` of `dart2js -O3/-O4`,
    /// `options.dart:1102-1126`): implicit casts from `dynamic` and covariance
    /// checks are not emitted. **Unsafe**: a program with a type error keeps
    /// running instead of throwing `TypeError`.
    fn omitir_checagens(&self) -> bool {
        false
    }
}

/// Everything live: must emit exactly what the unfiltered path emits.
pub struct TudoVivo;

impl Vivos for TudoVivo {
    fn classe(&self, _: ClassId) -> Nivel {
        Nivel::Instanciada
    }
    fn funcao(&self, _: FunctionElementId) -> Estado {
        Estado::Viva
    }
    fn variavel(&self, _: VariableId) -> Estado {
        Estado::Viva
    }
    fn seletor(&self, _: &str) -> bool {
        true
    }
    fn seletor_escrita(&self, _: &str) -> bool {
        true
    }
    fn tearoff_ctor(&self, _: FunctionElementId) -> bool {
        true
    }
}
