//! As tabelas das regras de lint do SDK 3.6.2
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): o registro das 240 regras
//! ([`tabela_g`]) e os códigos que elas relatam ([`codigos_g`]), geradas por
//! `scripts/gerar-tabelas-analise.py`. As regras implementadas estão em
//! [`regras`] a [`regras10`] (oitenta, as que só olham a árvore e o texto; [`andar`] é o passeio com ancestrais e [`cordas`] relê os literais de string e os comentários); as tabelas servem também à
//! validação de `linter: rules:` do `analysis_options.yaml`
//! (`crate::naodart::opcoes`).
//!
//! Escrito sem compilar nem executar (2026-10-05).

pub mod andar;
pub mod codigos_g;
pub mod cordas;
pub mod regras;
pub mod regras2;
pub mod regras3;
pub mod regras4;
pub mod regras5;
pub mod regras6;
pub mod regras7;
pub mod regras8;
pub mod regras10;
pub mod regras11;
pub mod regras12;
pub mod regras13;
pub mod regras9;
pub mod tabela_g;

/// O `state:` de uma regra (`linter/lib/src/analyzer.dart`, `State`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDaRegra {
    Estavel,
    Experimental,
    /// Só para o próprio SDK.
    Interna,
    Depreciada,
    Removida,
}

/// O conjunto do `package:lints` que liga a regra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conjunto {
    Nenhum,
    /// `package:lints/core.yaml` (e portanto também o `recommended`).
    Core,
    /// Só `package:lints/recommended.yaml`.
    Recommended,
}

/// Uma regra registrada.
#[derive(Debug, PartialEq, Eq)]
pub struct InfoRegra {
    pub nome: &'static str,
    pub estado: EstadoDaRegra,
    /// A versão do `since:` do estado, quando há.
    pub desde: Option<(u32, u32, u32)>,
    /// `incompatibleRules`.
    pub incompativeis: &'static [&'static str],
    pub conjunto: Conjunto,
    /// O arquivo da regra em `linter/lib/src/`.
    pub arquivo: &'static str,
}

/// Um código de lint (`LinterLintCode`).
#[derive(Debug, PartialEq, Eq)]
pub struct CodigoLint {
    /// O nome relatado (o da regra).
    pub nome: &'static str,
    /// `uniqueName`.
    pub unico: &'static str,
    pub mensagem: &'static str,
    pub correcao: Option<&'static str>,
    pub documentado: bool,
}

/// Roda todas as regras implementadas que estão ligadas (`ligada(nome)`)
/// sobre uma unidade; os relatos saem em ordem de posição.
pub fn executar(
    u: crate::Unidade<'_>,
    interner: &dartforge_intern::Interner,
    ligada: &dyn Fn(&str) -> bool,
) -> Vec<regras::RelatoDeLint> {
    let mut out = regras::executar(u, interner, ligada);
    out.extend(regras2::executar(u, interner, ligada));
    out.extend(regras3::executar(u, interner, ligada));
    out.extend(regras4::executar(u, interner, ligada));
    out.extend(regras5::executar(u, interner, ligada));
    out.extend(regras6::executar(u, interner, ligada));
    out.extend(regras7::executar(u, interner, ligada));
    out.extend(regras8::executar(u, interner, ligada));
    out.extend(regras9::executar(u, interner, ligada));
    out.extend(regras10::executar(u, interner, ligada));
    out.extend(regras11::executar(u, interner, ligada));
    out.extend(regras12::executar(u, interner, ligada));
    out.extend(regras13::executar(u, interner, ligada));
    out.sort_by_key(|r| (r.span.start, r.span.end));
    out
}

/// A regra registrada de nome `nome`.
pub fn regra(nome: &str) -> Option<&'static InfoRegra> {
    tabela_g::REGRAS.iter().find(|r| r.nome == nome)
}
