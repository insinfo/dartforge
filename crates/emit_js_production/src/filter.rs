//! The closed world (`crates/world`) as seen by the emitter
//! (`emit_js::filtro`).

use dartforge_elements::model::{ClassId, FunctionElementId, Program, VariableId};
use dartforge_emit_js::filtro::{Estado, Nivel, Vivos};
use dartforge_world::{Mundo, NivelClasse};

pub struct Adapter<'m> {
    pub world: &'m Mundo,
    pub program: &'m Program,
    /// Verifier mode: what is dead becomes a *stub* that reports the call
    /// (`dart_podado("…")`) instead of disappearing.
    pub stub: bool,
    /// Names whose signature entries are emitted (those of dynamic dispatch
    /// and of the *tearoffs* in the text); `None`: all of them.
    pub signatures: Option<&'m std::collections::HashSet<String>>,
    /// Constant conditions and `assert` (profile with the own SDK).
    pub constants: Option<&'m dartforge_world::Constantes<'m>>,
    /// Short keys for named parameters (`named_args.rs`).
    pub named_args: Option<&'m crate::named_args::NamedArgs>,
    /// `--omitir-checagens`.
    pub omit_checks: bool,
}

impl Vivos for Adapter<'_> {
    fn classe(&self, c: ClassId) -> Nivel {
        match self.world.classe(c) {
            NivelClasse::Instanciada => Nivel::Instanciada,
            NivelClasse::Tipo => Nivel::Tipo,
            NivelClasse::Morta if self.stub => Nivel::Tipo,
            NivelClasse::Morta => Nivel::Morta,
        }
    }
    fn funcao(&self, f: FunctionElementId) -> Estado {
        if self.world.funcao(f) {
            Estado::Viva
        } else if self.stub && self.program.function(f).extension.is_none() {
            Estado::Stub
        } else if self.stub {
            // Extension member: the emitter has no stub form for the key
            // `L['Ext|m']`; in verifier mode it stays.
            Estado::Viva
        } else {
            Estado::Morta
        }
    }
    fn variavel(&self, v: VariableId) -> Estado {
        if self.world.variavel(v) {
            Estado::Viva
        } else if self.stub && self.program.variable(v).extension.is_none() {
            Estado::Stub
        } else if self.stub {
            Estado::Viva
        } else {
            Estado::Morta
        }
    }
    fn seletor(&self, name: &str) -> bool {
        // `noSuchMethod` forwarders: it is enough for the name to be live in
        // some way (unrestricted or restricted to any cone) — emitting too
        // much only costs bytes.
        self.stub || self.world.seletor(name) || self.world.tem_restricao(name)
    }
    fn seletor_escrita(&self, base_name: &str) -> bool {
        // The write is live through the setter key in `instance_members`,
        // unrestricted or restricted to any cone (as in `seletor`, above):
        // `p.modo = v` with `p: Proxy` only registers `modo_=` in the cone of
        // `Proxy`.
        let key = format!("{base_name}_=");
        self.stub || self.world.seletor(&key) || self.world.tem_restricao(&key)
    }
    fn tearoff_ctor(&self, f: FunctionElementId) -> bool {
        self.stub || self.world.tearoff_de_construtor(f)
    }
    fn assinatura(&self, name: &str) -> bool {
        // The text cites the JS name (`dload(o, "_constructor")`,
        // `dsend(o, "_get")`); the emitter asks with the Dart name of fields
        // and accessors.
        self.signatures.is_none_or(|s| s.contains(name) || s.contains(dartforge_emit_js::body::js_member_name(name).as_str()))
    }
    fn manter_asserts(&self) -> bool {
        self.constants.is_none_or(|c| !c.sem_asserts())
    }
    fn constante_bool(&self, u: dartforge_elements::model::UnitId, x: dartforge_frontend::ast::ExprId) -> Option<bool> {
        self.constants?.bool_de(u, x)
    }
    fn nome_nomeado(&self, name: &str) -> Option<&str> {
        self.named_args?.map.get(name).map(String::as_str)
    }
    fn omitir_checagens(&self) -> bool {
        self.omit_checks
    }
    fn tabela_de_nomeados(&self) -> Option<String> {
        self.named_args.filter(|n| n.table).map(|n| n.json())
    }
}
