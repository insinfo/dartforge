//! The closed world (`crates/world`) as seen by the emitter
//! (`emit_js::filter`).

use dartforge_elements::model::{ClassId, FunctionElementId, Program, VariableId};
use dartforge_emit_js::filter::{Level, Liveness, State};
use dartforge_world::{ClassLevel, World};

/// The short recipe ids ranked by `world`: the live classes among the
/// frequent ones first (`int`, `String`, `List`…), then the instantiated
/// classes, then those only used as types, then the dead ones
/// (`dartforge_emit_js::recipe_ids_ranked`). The live classes get the short
/// ids and the name table (`t$N`) has no holes for the dead ones. Ranking by
/// how often the text cites each class was measured too: a little smaller in
/// the corpus, but larger in gzip and in both projects.
/// `DARTFORGE_JSPROD_RANKED_IDS=0` keeps the program's order.
pub fn recipe_ids(
    world: &World,
    program: &Program,
    interner: &dartforge_intern::Interner,
) -> Option<Vec<String>> {
    if std::env::var("DARTFORGE_JSPROD_RANKED_IDS").is_ok_and(|v| v == "0") {
        return None;
    }
    Some(dartforge_emit_js::recipe_ids_ranked(
        program,
        interner,
        |c, frequent| match world.class_level(c) {
            ClassLevel::Dead => (2u8, 0u8),
            ClassLevel::Instantiated => (u8::from(!frequent), 0),
            ClassLevel::Type => (u8::from(!frequent), 1),
        },
    ))
}

pub struct Adapter<'m> {
    pub world: &'m World,
    pub program: &'m Program,
    pub interner: &'m dartforge_intern::Interner,
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
    /// The short recipe ids ranked by the world (`recipe_ids`); `None`: the
    /// program's order.
    pub recipe_ids: Option<&'m [String]>,
    /// Constant conditions and `assert` (profile with the own SDK).
    pub constants: Option<&'m dartforge_world::Constants<'m>>,
    /// Short keys for named parameters (`named_args.rs`).
    pub named_args: Option<&'m crate::named_args::NamedArgs>,
    /// `--omit-implicit-checks` (`-O3`, `-O4`).
    pub omit_implicit_checks: bool,
    /// `--trust-primitives` (`-O4`).
    pub trust_primitives: bool,
    /// `--omit-late-names` (`-O2` up).
    pub omit_late_names: bool,
    /// `identityHashCode` of `dart:core` (`None`: unknown, assumed live).
    pub identity_hash: Option<FunctionElementId>,
    /// `equals` of `dart:_runtime` (`None`: unknown, assumed live).
    pub runtime_equals: Option<FunctionElementId>,
    /// The shortened named-parameter keys the text's function types cite
    /// (`own_sdk::named_keys_in_recipes`): the runtime's printer translates
    /// them back (`dart.namedSourceName`). `None`: none, no table.
    pub named_source_keys: Option<&'m std::collections::BTreeSet<String>>,
    /// [`rti_live`] of the round's world.
    pub rti_live: bool,
}

/// Whether the rti may run in `world`: a function of `dart:_rti` is live, or
/// one of the runtime functions that read the rti resources of a class
/// (`isDartClass`, `getClassName`). Monotone over the fixed point: whatever
/// the bootstrap emits once it holds (the rules, `findType`) keeps it.
/// `DARTFORGE_JSPROD_RTI_GATE=0` always says yes.
pub fn rti_live(world: &World, program: &Program, interner: &dartforge_intern::Interner) -> bool {
    if std::env::var("DARTFORGE_JSPROD_RTI_GATE").is_ok_and(|v| v == "0") {
        return true;
    }
    let lib = |uri: &str| program.libraries.iter().position(|l| l.uri == uri);
    let (Some(rti), Some(runtime)) = (lib("dart:_rti"), lib("dart:_runtime")) else {
        return true;
    };
    program.functions.iter().enumerate().any(|(i, f)| {
        let l = f.library.0 as usize;
        (l == rti
            || (l == runtime
                && f.class.is_none()
                && matches!(interner.resolve(f.name), "isDartClass" | "getClassName")))
            && world.is_live_function(FunctionElementId(i as u32))
    })
}

impl Adapter<'_> {
    /// An abstract class or a mixin outside the libraries that declare
    /// native classes (whose JavaScript objects may belong to an abstract
    /// class): the world lists it among the instantiated classes when a
    /// concrete class mixes it in (`ListBase` in the typed arrays), but every
    /// object is an instance of a concrete class, listed on its own.
    fn without_own_instances(&self, k: ClassId) -> bool {
        let class = self.program.class(k);
        (class.modifiers.abstract_ || class.kind == dartforge_elements::model::ClassKind::Mixin)
            && !matches!(
                self.program.library(class.library).uri.as_str(),
                "dart:_interceptors"
                    | "dart:_native_typed_data"
                    | "dart:_js_helper"
                    | "dart:html"
                    | "dart:svg"
                    | "dart:web_audio"
                    | "dart:web_gl"
                    | "dart:indexed_db"
            )
    }

    /// `k` is `JSArray` or `JSString` (`dart:_interceptors`), or a native
    /// typed array of numbers (`dart:_native_typed_data`, below
    /// `NativeTypedArrayOfInt` or `NativeTypedArrayOfDouble`): its `[]` reads
    /// the JavaScript element. The SIMD lists (`NativeInt32x4List`…) build an
    /// object per element and are not.
    fn is_indexable_primitive_class(&self, k: ClassId) -> bool {
        let class = self.program.class(k);
        let lib = self.program.library(class.library).uri.as_str();
        let name = self.interner.resolve(class.name);
        match lib {
            "dart:_interceptors" => matches!(name, "JSArray" | "JSString"),
            "dart:_native_typed_data" => {
                self.world.instantiated_subtypes(k).contains(&k)
                    && self.program.classes.iter().enumerate().any(|(i, c)| {
                        self.program.library(c.library).uri == "dart:_native_typed_data"
                            && matches!(
                                self.interner.resolve(c.name),
                                "NativeTypedArrayOfInt" | "NativeTypedArrayOfDouble"
                            )
                            && self.world.is_subclass(k, ClassId(i as u32))
                    })
            }
            _ => false,
        }
    }
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
    fn recipe_ids(&self) -> Option<&[String]> {
        self.recipe_ids
    }
    fn rti_live(&self) -> bool {
        self.stub || self.rti_live
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
        self.omit_implicit_checks
    }
    fn trust_primitives(&self) -> bool {
        self.trust_primitives
    }
    fn omit_late_names(&self) -> bool {
        self.omit_late_names
    }
    fn indexable_primitive(&self, c: ClassId) -> bool {
        let subs = self.world.instantiated_subtypes(c);
        !subs.is_empty()
            && subs
                .iter()
                .all(|&k| self.is_indexable_primitive_class(k) || self.without_own_instances(k))
    }
    fn named_table(&self) -> Option<String> {
        self.named_args.filter(|n| n.table).map(|n| n.json())
    }
    fn named_source_table(&self) -> Option<String> {
        let keys = self.named_source_keys?;
        Some(self.named_args?.reverse_json(keys))
    }
}
