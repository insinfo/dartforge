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
    /// The signature entry of member `name` under its `dartx` key is read on
    /// an object whose constructor is a Dart class (not a native peer): only
    /// a *tear-off* through the extension symbol (`dart.bind(o, $name)`)
    /// does that — dynamic dispatch reaches a Dart object by the string. The
    /// classes in a native peer's prototype chain keep the key regardless.
    /// The default is to emit all of them.
    fn symbol_signature(&self, _name: &str) -> bool {
        true
    }
    /// The short recipe id of each class (index: `ClassId`), when the
    /// caller ranks them ([`crate::recipe_ids_ranked`]); `None`: the
    /// program's own order ([`crate::recipe_ids`]).
    fn recipe_ids(&self) -> Option<&[String]> {
        None
    }
    /// The class's `addRtiResources` (its recipe on the constructor and its
    /// type tags on the prototype) is emitted. Only objects read them, so a
    /// class that no instance has in its prototype chain may go without; the
    /// default is to emit it.
    fn rti_resources(&self, _c: ClassId) -> bool {
        true
    }
    /// The rti may run: some function of `dart:_rti`, or a runtime function
    /// that reads a class's rti resources, is live. Without it no recipe is
    /// ever evaluated and no type tag is read, so the rti rules, the warm-up
    /// of the type universe and every `addRtiResources` are not emitted.
    /// Without a filter, always.
    fn rti_live(&self) -> bool {
        true
    }
    /// `identityHashCode` of `dart:core` may run: the primitives get the
    /// property it reads (`dart.definePrimitiveHashCode`). Without a filter,
    /// always.
    fn identity_hash_code(&self) -> bool {
        true
    }
    /// Legacy (`T*`) types may exist at run time. Without a filter, always.
    fn legacy_types(&self) -> bool {
        true
    }
    /// The `case` with pattern `p` (of unit `u`, without a guard) can never
    /// match and is not written (`dartforge_world::Constants::dead_case`).
    fn dead_case(
        &self,
        _u: dartforge_elements::model::UnitId,
        _p: dartforge_frontend::ast::PatternId,
    ) -> bool {
        false
    }
    /// `dart.equals` of the runtime may run: `Object` installs the identity
    /// `==` it compares against (`dart._installIdentityEquals`). Without a
    /// filter, always.
    fn runtime_equals(&self) -> bool {
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
    /// The shortened keys of named parameters as JSON (key → name), when a
    /// function type with named parameters may be printed (the runtime's
    /// `dart.namedSourceName`).
    fn named_source_table(&self) -> Option<String> {
        None
    }
    /// `-O3` and `-O4` (the `--omit-implicit-checks` of `dart2js`,
    /// `options.dart`, `deriveOptions`): implicit downcasts, conditions and
    /// parameter checks (covariance, the arguments of dynamic calls) are not
    /// emitted unless a member's pragma asks for them ([`crate::checks`]),
    /// and neither are the interop null assertions (`nativeNullAssertions`
    /// is off from `-O3` up). **Unsafe**: a program with a type error keeps
    /// running instead of throwing `TypeError`.
    fn omit_implicit_checks(&self) -> bool {
        false
    }
    /// `-O4` (the `--trust-primitives` of `dart2js`): index range checks on
    /// the indexable primitives are not emitted unless a member's pragma asks
    /// for them ([`crate::checks`]). **Unsafe**.
    fn trust_primitives(&self) -> bool {
        false
    }
    /// The `LateError` messages leave the variable's name out (the
    /// `--omit-late-names` of `dart2js -O2`); `-O1` keeps it.
    fn omit_late_names(&self) -> bool {
        true
    }
    /// Every class the world has instantiated below `c` (a receiver's static
    /// class) is an indexable primitive: `JSArray`, `JSString` or a native
    /// typed array, so `r[i]` reads the JavaScript element directly. Without
    /// a filter, never.
    fn indexable_primitive(&self, _c: ClassId) -> bool {
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
