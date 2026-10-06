//! Relevância dos itens do completar como o servidor do Dart 3.6.2 a calcula
//! (`services/completion/dart/relevance_computer.dart` e
//! `feature_computer.dart`): uma média ponderada de características,
//! levada a `0..1000`, e `sortText = 9999 − relevância`.
//!
//! * **tipo de contexto** — o tipo que a posição espera (parâmetro do
//!   argumento, alvo da atribuição, tipo escrito da variável, retorno da
//!   função, condição `bool`) contra o tipo do item: igual 1,0; subtipo 0,40;
//!   supertipo 0,02; sem relação 0,13; sem um dos dois, 0;
//! * **espécie do elemento** — a faixa de probabilidade da espécie (classe,
//!   local, parâmetro, campo, método, função…) no local do completar
//!   (`Block_statement`, `ArgumentList_method_unnamed`,
//!   `PropertyAccess_propertyName`…), das tabelas geradas do Dart
//!   (`relevancia_tabelas.rs`); o meio da faixa, ou, com distância
//!   (locais pela proximidade, membros pela herança), `meio + (alto −
//!   baixo) × distância / 2`;
//! * **palavra-chave** — o alto da faixa da palavra no local;
//! * **não importado** −1, **começa com `$`** −1 (peso 0,5), **noSuchMethod**
//!   −1, **distância do local** `0,9^d`.
//!
//! `relevância = ⌊((Σ vᵢ·pᵢ / Σ pᵢ) + 1) / 2 × 1000⌋`, com os pesos do Dart
//! (todos 1, menos depreciado e `$`, 0,5). Argumentos nomeados têm valor
//! fixo (900; 950 se obrigatório), como no Dart.

use dartforge_frontend::ast::{self, ExprId};

/// Espécie de elemento das tabelas do Dart (`computeElementKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Especie {
    Classe,
    Construtor,
    Enum,
    Campo,
    Funcao,
    AliasDeFuncao,
    Local,
    Metodo,
    Mixin,
    Parametro,
    Prefixo,
    VariavelDeTopo,
    ParametroDeTipo,
    Desconhecido,
    /// Espécies sem tabela (constante de enum, extensão, alias de tipo):
    /// característica de espécie 0.
    SemTabela,
}

/// O que se sabe de um item para pontuá-lo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Rel {
    /// Espécie, quando o item é um elemento.
    pub especie: Option<Especie>,
    /// O tipo do item (variável, retorno, getter), para o tipo de contexto.
    pub tipo: Option<dartforge_types::TypeId>,
    /// Distância (`d`, que vira `0,9^d`): local pela proximidade, membro
    /// pela herança.
    pub distancia: Option<u32>,
    /// Local ou parâmetro: a distância entra também como característica.
    pub local: bool,
    pub nao_importado: bool,
    /// Palavra-chave (o texto).
    pub palavra: Option<&'static str>,
    /// Relevância fixa (argumentos nomeados).
    pub fixa: Option<i32>,
    pub comeca_com_dolar: bool,
    pub no_such_method: bool,
    /// `hasOrInheritsDeprecated` (`hasDeprecated`, peso 0,5).
    pub obsoleto: bool,
    /// `isConstantFeature`: construtor `const`, campo estático `const`,
    /// variável de topo `const` (só vale com `preferConstants`).
    pub constante: bool,
    /// `superMatches`: o nome é o do método que contém o `super.▮`.
    pub super_corresponde: bool,
}

/// `distanceToPercent`.
pub(crate) fn porcentagem(distancia: usize) -> f64 {
    0.9f64.powi(distancia as i32)
}

/// A característica de espécie no `local`.
fn caracteristica_de_especie(especie: Especie, local: Option<&str>, distancia: Option<f64>) -> f64 {
    let Some(local) = local else { return 0.0 };
    let Some((_, baixo, alto)) = super::relevancia_tabelas::especies(local).iter().find(|(e, _, _)| *e == especie) else {
        return 0.0;
    };
    let meio = (baixo + alto) / 2.0;
    match distancia {
        None => meio,
        Some(d) => meio + ((alto - baixo) * d / 2.0),
    }
}

/// A característica de palavra-chave no `local` (o alto da faixa).
fn caracteristica_de_palavra(palavra: &str, local: Option<&str>) -> f64 {
    let Some(local) = local else { return 0.0 };
    let tabela = super::relevancia_tabelas::palavras(local);
    let achar = |p: &str| tabela.iter().find(|(k, _, _)| *k == p).map(|(_, _, alto)| *alto);
    achar(palavra)
        .or_else(|| palavra.find(|c: char| !c.is_ascii_lowercase()).filter(|i| *i > 0).and_then(|i| achar(&palavra[..i])))
        .unwrap_or(0.0)
}

/// A relevância (`0..1000`) de um item.
pub(crate) fn relevancia(rel: &Rel, local: Option<&str>, tipo_de_contexto: f64, preferir_constantes: bool) -> i32 {
    if let Some(f) = rel.fixa {
        return f;
    }
    let distancia = rel.distancia.map(|d| porcentagem(d as usize));
    // `_computeFormalParameterRelevance`: a espécie do parâmetro é sem a
    // distância (só o local a usa, `_computeLocalVariableRelevance`).
    let especie = rel.especie.map_or(0.0, |e| caracteristica_de_especie(e, local, if e == Especie::Parametro { None } else { distancia }));
    let palavra = rel.palavra.map_or(0.0, |p| caracteristica_de_palavra(p, local));
    let nao_importado = if rel.nao_importado { -1.0 } else { 0.0 };
    let dolar = if rel.comeca_com_dolar { -1.0 } else { 0.0 };
    let nsm = if rel.no_such_method { -1.0 } else { 0.0 };
    let distancia_local = if rel.local { distancia.unwrap_or(0.0) } else { 0.0 };
    let obsoleto = if rel.obsoleto { -1.0 } else { 0.0 };
    let constante = if preferir_constantes && rel.constante { 1.0 } else { 0.0 };
    let super_corresponde = if rel.super_corresponde { 1.0 } else { 0.0 };
    // Pesos do Dart: contexto 1, espécie 1, depreciado 0,5, constante 1,
    // noSuchMethod 1, não importado 1, palavra 1, `$` 0,5, super 1,
    // distância do local 1 — total 9.
    let soma = tipo_de_contexto + especie + 0.5 * obsoleto + constante + nsm + nao_importado + palavra + 0.5 * dolar + super_corresponde + distancia_local;
    let media = soma / 9.0;
    (((media + 1.0) / 2.0) * 1000.0) as i32
}

fn elementos_contem(elementos: &[ast::CollectionElement], alvo: ExprId) -> bool {
    elementos.iter().any(|e| match e {
        ast::CollectionElement::Expression(x) | ast::CollectionElement::NullAwareExpression(x) => *x == alvo,
        ast::CollectionElement::MapEntry { key, value, .. } => *key == alvo || *value == alvo,
        _ => false,
    })
}

fn elementos_valor(elementos: &[ast::CollectionElement], alvo: ExprId) -> bool {
    elementos.iter().any(|e| matches!(e, ast::CollectionElement::MapEntry { value, .. } if *value == alvo))
}
