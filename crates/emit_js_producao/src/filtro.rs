//! O mundo fechado (`crates/mundo`) visto pelo emissor (`emit_js::filtro`).

use dartforge_elements::model::{ClassId, FunctionElementId, Program, VariableId};
use dartforge_emit_js::filtro::{Estado, Nivel, Vivos};
use dartforge_mundo::{Mundo, NivelClasse};

pub struct Adaptador<'m> {
    pub mundo: &'m Mundo,
    pub program: &'m Program,
    /// Modo verificador: o que está morto vira *stub* que denuncia a chamada
    /// (`dart_podado("…")`) em vez de sumir.
    pub stub: bool,
}

impl Vivos for Adaptador<'_> {
    fn classe(&self, c: ClassId) -> Nivel {
        match self.mundo.classe(c) {
            NivelClasse::Instanciada => Nivel::Instanciada,
            NivelClasse::Tipo => Nivel::Tipo,
            NivelClasse::Morta if self.stub => Nivel::Tipo,
            NivelClasse::Morta => Nivel::Morta,
        }
    }
    fn funcao(&self, f: FunctionElementId) -> Estado {
        if self.mundo.funcao(f) {
            Estado::Viva
        } else if self.stub && self.program.function(f).extension.is_none() {
            Estado::Stub
        } else if self.stub {
            // Membro de extensão: o emissor não tem forma de stub para a
            // chave `L['Ext|m']`; no modo verificador ele fica.
            Estado::Viva
        } else {
            Estado::Morta
        }
    }
    fn variavel(&self, v: VariableId) -> Estado {
        if self.mundo.variavel(v) {
            Estado::Viva
        } else if self.stub && self.program.variable(v).extension.is_none() {
            Estado::Stub
        } else if self.stub {
            Estado::Viva
        } else {
            Estado::Morta
        }
    }
    fn seletor(&self, nome: &str) -> bool {
        self.stub || self.mundo.seletor(nome)
    }
    fn seletor_escrita(&self, nome_base: &str) -> bool {
        self.stub || self.mundo.seletor(&format!("{nome_base}="))
    }
    fn tearoff_ctor(&self, f: FunctionElementId) -> bool {
        self.stub || self.mundo.tearoff_de_construtor(f)
    }
}
