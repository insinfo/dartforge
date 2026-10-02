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
    /// Nomes cujas entradas de assinatura são emitidas (os do despacho
    /// dinâmico e dos *tearoffs* no texto); `None`: todas.
    pub assinaturas: Option<&'m std::collections::HashSet<String>>,
    /// Condições constantes e `assert` (perfil com o SDK próprio).
    pub constantes: Option<&'m dartforge_mundo::Constantes<'m>>,
    /// Chaves curtas dos parâmetros nomeados (`nomeados.rs`).
    pub nomeados: Option<&'m crate::nomeados::Nomeados>,
    /// `--omitir-checagens`.
    pub omitir_checagens: bool,
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
        // Encaminhadores de `noSuchMethod`: basta o nome viver de algum jeito
        // (irrestrito ou restrito a qualquer cone) — emitir a mais é só bytes.
        self.stub || self.mundo.seletor(nome) || self.mundo.tem_restricao(nome)
    }
    fn seletor_escrita(&self, nome_base: &str) -> bool {
        // A escrita vive pela chave do setter no `instance_members`, irrestrita
        // ou restrita a qualquer cone (como em `seletor`, acima): `p.modo = v`
        // com `p: Proxy` só registra `modo_=` no cone de `Proxy`.
        let chave = format!("{nome_base}_=");
        self.stub || self.mundo.seletor(&chave) || self.mundo.tem_restricao(&chave)
    }
    fn tearoff_ctor(&self, f: FunctionElementId) -> bool {
        self.stub || self.mundo.tearoff_de_construtor(f)
    }
    fn assinatura(&self, nome: &str) -> bool {
        // O texto cita o nome JS (`dload(o, "_constructor")`, `dsend(o, "_get")`);
        // o emissor pergunta pelo nome Dart dos campos e acessores.
        self.assinaturas.is_none_or(|s| s.contains(nome) || s.contains(dartforge_emit_js::body::js_member_name(nome).as_str()))
    }
    fn manter_asserts(&self) -> bool {
        self.constantes.is_none_or(|c| !c.sem_asserts())
    }
    fn constante_bool(&self, u: dartforge_elements::model::UnitId, x: dartforge_frontend::ast::ExprId) -> Option<bool> {
        self.constantes?.bool_de(u, x)
    }
    fn nome_nomeado(&self, nome: &str) -> Option<&str> {
        self.nomeados?.mapa.get(nome).map(String::as_str)
    }
    fn omitir_checagens(&self) -> bool {
        self.omitir_checagens
    }
    fn tabela_de_nomeados(&self) -> Option<String> {
        self.nomeados.filter(|n| n.tabela).map(|n| n.json())
    }
}
