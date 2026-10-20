//! Reachability filter for the production profile (`crates/emit_js_production`,
//! `docs/JS-PRODUCAO.md` §1.7).
//!
//! The emitter asks the filter whether a class, function or variable is live, and
//! does not emit what is dead. **Without a filter (`Ctx::filter == None`), nothing
//! changes**: that is the development profile and `dartforge serve`, byte for byte as
//! before — the `identidade` test of `emit_js_production` compiles the same programs
//! through both paths and compares them.

use dartforge_elements::model::{ClassId, FunctionElementId, VariableId};

/// Reachability level of a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Not emitted.
    Dead,
    /// Identity only: shell, `addRtiResources`, rti rules and live statics.
    Type,
    /// Constructed: everything the filter says is live.
    Instantiated,
}

/// State of a function or variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Live,
    Dead,
    /// Emitted as a *stub* that reports the call (`dart_podado("…")`): the
    /// pruning checker mode.
    Stub,
}

pub trait Liveness {
    fn class_level(&self, c: ClassId) -> Level;
    fn function(&self, f: FunctionElementId) -> State;
    fn variable(&self, v: VariableId) -> State;
    /// Name invoked somewhere (`noSuchMethod` forwarders).
    fn selector(&self, name: &str) -> bool;
    /// Name written somewhere (`x.name = v`): the `noSuchMethod` forwarders for
    /// abstract setters. Without a filter, always.
    fn write_selector(&self, base_name: &str) -> bool;
    /// The constructor's static tear-off `_#name#tearOff` is referenced.
    fn tearoff_ctor(&self, f: FunctionElementId) -> bool;
    /// The signature entry (`setMethodSignature` and its siblings) of member
    /// `name` is emitted. Only dynamic dispatch (`dsend`/`dload`/`dput`) and the
    /// *tear-off* (`bind`) read signatures (`docs/JS-PRODUCAO-SDK-PROPRIO.md`
    /// §6); the default is to emit all of them.
    fn signature(&self, _name: &str) -> bool {
        true
    }
    /// The program's `assert`s are emitted (the production profile turns them
    /// off by default, like `dart2js`).
    fn keep_asserts(&self) -> bool {
        true
    }
    /// Constant boolean value proven for expression `x` of unit `u`
    /// (`dartforge_world::Constants`): the emitter writes the literal, and the
    /// dead branch of an `if` disappears.
    fn const_bool(
        &self,
        _u: dartforge_elements::model::UnitId,
        _x: dartforge_frontend::ast::ExprId,
    ) -> Option<bool> {
        None
    }
    /// The JS key of a named parameter (production profile:
    /// `docs/JS-PRODUCAO-TAMANHO.md` §8.2); `None` means the name itself.
    fn named_key_of(&self, _name: &str) -> Option<&str> {
        None
    }
    /// The map of named parameters as JSON (name → key), when the runtime needs
    /// to translate names at run time (a live `Function.apply`).
    fn named_table(&self) -> Option<String> {
        None
    }
    /// `--omit-checks` (the `omitImplicitChecks` of `dart2js -O3/-O4`,
    /// `options.dart:1102-1126`): implicit casts from `dynamic` and covariance
    /// checks are not emitted. **Unsafe**: a program with a type error keeps
    /// running instead of throwing `TypeError`.
    fn omit_implicit_checks(&self) -> bool {
        false
    }
    /// Whether the emission makes the production profile's choices that are
    /// not pruning (eager static literals, short signatures, chained `null`
    /// initializations). [`AllLive`] says no: with it the text is the one
    /// without a filter.
    fn production(&self) -> bool {
        true
    }
}

/// Everything live: must emit exactly what the unfiltered path emits.
pub struct AllLive;

impl Liveness for AllLive {
    fn class_level(&self, _: ClassId) -> Level {
        Level::Instantiated
    }
    fn function(&self, _: FunctionElementId) -> State {
        State::Live
    }
    fn variable(&self, _: VariableId) -> State {
        State::Live
    }
    fn selector(&self, _: &str) -> bool {
        true
    }
    fn production(&self) -> bool {
        false
    }
    fn write_selector(&self, _: &str) -> bool {
        true
    }
    fn tearoff_ctor(&self, _: FunctionElementId) -> bool {
        true
    }
}
