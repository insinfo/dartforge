//! Filtro de alcance para o perfil de produção (`crates/emit_js_producao`,
//! `docs/JS-PRODUCAO.md` §1.7).
//!
//! O emissor pergunta ao filtro se uma classe, função ou variável vive, e não
//! emite o que está morto. **Sem filtro (`Ctx::filtro == None`), nada muda**:
//! é o perfil de desenvolvimento e o `dartforge serve`, byte a byte como antes
//! — o teste `identidade` do `emit_js_producao` compila os mesmos programas
//! pelos dois caminhos e compara.

use dartforge_elements::model::{ClassId, FunctionElementId, VariableId};

/// Nível de alcance de uma classe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Nivel {
    /// Não emitida.
    Morta,
    /// Só identidade: casca, `addRtiResources`, regras rti e estáticos vivos.
    Tipo,
    /// Construída: tudo o que o filtro disser vivo.
    Instanciada,
}

/// Estado de uma função ou variável.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    Viva,
    Morta,
    /// Emitida como *stub* que denuncia a chamada (`dart_podado("…")`): o
    /// modo verificador da poda.
    Stub,
}

pub trait Vivos {
    fn classe(&self, c: ClassId) -> Nivel;
    fn funcao(&self, f: FunctionElementId) -> Estado;
    fn variavel(&self, v: VariableId) -> Estado;
    /// Nome invocado em algum lugar (encaminhadores de `noSuchMethod`).
    fn seletor(&self, nome: &str) -> bool;
    /// Nome escrito em algum lugar (`x.nome = v`): os encaminhadores de
    /// `noSuchMethod` para setters abstratos. Sem filtro, sempre.
    fn seletor_escrita(&self, nome_base: &str) -> bool;
    /// O tearoff estático `_#nome#tearOff` do construtor é citado.
    fn tearoff_ctor(&self, f: FunctionElementId) -> bool;
}

/// Tudo vivo: tem de emitir exatamente o que o caminho sem filtro emite.
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
