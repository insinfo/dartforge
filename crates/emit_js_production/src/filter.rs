//! The closed world (`crates/world`) as seen by the emitter
//! (`emit_js::filter`).

use dartforge_elements::model::{ClassId, FunctionElementId, Program, VariableId};
use dartforge_emit_js::filter::{Level, Liveness, State};
use dartforge_world::{ClassLevel, World};

pub struct Adapter<'m> {
    pub world: &'m World,
    pub program: &'m Program,
    /// Verifier mode: what is dead becomes a *stub* that reports the call
    /// (`dart_podado("…")`) instead of disappearing.
    pub stub: bool,
    /// Names whose signature entries are emitted (those of dynamic dispatch
    /// and of the *tearoffs* in the text); `None`: all of them.
    pub signatures: Option<&'m std::collections::HashSet<String>>,
    /// Names torn off through their `dartx` symbol (`dart.bind(o, $name)`):
    /// the `dartx` signature entries of Dart classes outside the native
    /// chains; `None`: all of them.
    pub symbol_signatures: Option<&'m std::collections::HashSet<String>>,
    /// Constant conditions and `assert` (profile with the own SDK).
    pub constants: Option<&'m dartforge_world::Constants<'m>>,
    /// Short keys for named parameters (`named_args.rs`).
    pub named_args: Option<&'m crate::named_args::NamedArgs>,
    /// `--omit-checks`.
    pub omit_checks: bool,
    /// `identityHashCode` of `dart:core` (`None`: unknown, assumed live).
    pub identity_hash: Option<FunctionElementId>,
    /// `equals` of `dart:_runtime` (`None`: unknown, assumed live).
    pub runtime_equals: Option<FunctionElementId>,
}

impl Liveness for Adapter<'_> {
    fn class_level(&self, c: ClassId) -> Level {
        match self.world.class_level(c) {
            ClassLevel::Instantiated => Level::Instantiated,
            ClassLevel::Type => Level::Type,
            ClassLevel::Dead if self.stub => Level::Type,
            ClassLevel::Dead => Level::Dead,
        }
    }
    fn function(&self, f: FunctionElementId) -> State {
        if self.world.is_live_function(f) {
            State::Live
        } else if self.stub && self.program.function(f).extension.is_none() {
            State::Stub
        } else if self.stub {
            // Extension member: the emitter has no stub form for the key
            // `L['Ext|m']`; in verifier mode it stays.
            State::Live
        } else {
            State::Dead
        }
    }
    fn variable(&self, v: VariableId) -> State {
        if self.world.is_live_variable(v) {
            State::Live
        } else if self.stub && self.program.variable(v).extension.is_none() {
            State::Stub
        } else if self.stub {
            State::Live
        } else {
            State::Dead
        }
    }
    fn selector(&self, name: &str) -> bool {
        // `noSuchMethod` forwarders: it is enough for the name to be live in
        // some way (unrestricted or restricted to any cone) — emitting too
        // much only costs bytes.
        self.stub || self.world.selector(name) || self.world.has_restriction(name)
    }
    fn write_selector(&self, base_name: &str) -> bool {
        // The write is live through the setter key in `instance_members`,
        // unrestricted or restricted to any cone (as in `selector`, above):
        // `p.modo = v` with `p: Proxy` only registers `modo_=` in the cone of
        // `Proxy`.
        let key = format!("{base_name}_=");
        self.stub || self.world.selector(&key) || self.world.has_restriction(&key)
    }
    fn tearoff_ctor(&self, f: FunctionElementId) -> bool {
        self.stub || self.world.ctor_tearoff(f)
    }
    fn signature(&self, name: &str) -> bool {
        // The text cites the JS name (`dload(o, "_constructor")`,
        // `dsend(o, "_get")`); the emitter asks with the Dart name of fields
        // and accessors.
        self.signatures.is_none_or(|s| {
            s.contains(name) || s.contains(dartforge_emit_js::body::js_member_name(name).as_str())
        })
    }
    fn symbol_signature(&self, name: &str) -> bool {
        // `DARTFORGE_JSPROD_SYMBOL_SIGNATURES=0` emits every `dartx` entry.
        std::env::var("DARTFORGE_JSPROD_SYMBOL_SIGNATURES").is_ok_and(|v| v == "0")
            || self.symbol_signatures.is_none_or(|s| {
                s.contains(name)
                    || s.contains(dartforge_emit_js::body::js_member_name(name).as_str())
            })
    }
    fn rti_resources(&self, c: ClassId) -> bool {
        // `DARTFORGE_JSPROD_RTI_RESOURCES=0` emits them for every class.
        self.stub
            || self.world.in_instance_prototype_chain(c)
            || std::env::var("DARTFORGE_JSPROD_RTI_RESOURCES").is_ok_and(|v| v == "0")
    }
    fn identity_hash_code(&self) -> bool {
        // `DARTFORGE_JSPROD_PRIMITIVE_HASH=0` always defines it.
        self.stub
            || std::env::var("DARTFORGE_JSPROD_PRIMITIVE_HASH").is_ok_and(|v| v == "0")
            || self
                .identity_hash
                .is_none_or(|f| self.world.is_live_function(f))
    }
    fn runtime_equals(&self) -> bool {
        self.stub
            || std::env::var("DARTFORGE_JSPROD_PRIMITIVE_EQUALS").is_ok_and(|v| v == "0")
            || self
                .runtime_equals
                .is_none_or(|f| self.world.is_live_function(f))
    }
    fn legacy_types(&self) -> bool {
        self.constants.is_none_or(|c| !c.without_legacy())
    }
    fn dead_case(
        &self,
        u: dartforge_elements::model::UnitId,
        p: dartforge_frontend::ast::PatternId,
    ) -> bool {
        self.constants.is_some_and(|c| c.dead_case(u, p))
    }
    fn keep_asserts(&self) -> bool {
        self.constants.is_none_or(|c| !c.without_asserts())
    }
    fn const_bool(
        &self,
        u: dartforge_elements::model::UnitId,
        x: dartforge_frontend::ast::ExprId,
    ) -> Option<bool> {
        self.constants?.bool_of(u, x)
    }
    fn named_key_of(&self, name: &str) -> Option<&str> {
        self.named_args?.map.get(name).map(String::as_str)
    }
    fn omit_implicit_checks(&self) -> bool {
        self.omit_checks
    }
    fn named_table(&self) -> Option<String> {
        self.named_args.filter(|n| n.table).map(|n| n.json())
    }
}
