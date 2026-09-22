//! Interning de nomes: identificadores repetidos colapsam num índice de 4 bytes.
//!
//! A meta de memória exige colapsar identificadores repetidos na camada
//! semântica em vez de uma `String` por ocorrência. Hoje o AST empresta `&'a str`
//! da fonte em 54 pontos (`crates/syntax`), então nada sobrevive entre revisões
//! e nenhum cache por unidade é possível: o empréstimo amarra o modelo ao texto
//! de uma revisão específica.
//!
//! Este crate é a peça que quebra esse empréstimo. O desenho:
//!
//! * [`SymbolId`] é `Copy`, 4 bytes, e não aponta para revisão alguma;
//! * [`Interner`] é a arena de nomes de **uma revisão**: interna uma vez por
//!   nome distinto, resolve por índice, e informa [`Interner::payload_bytes`]
//!   para a verificação de `live_bytes` do modelo antes/depois;
//! * entre revisões, só o `SymbolId` atravessa; a arena velha cai com a revisão
//!   velha, e o descarte é determinístico como qualquer `drop`.
//!
//! O que ainda falta (fronteira registrada, não esquecida): trocar os 54 pontos
//! de `&'a str` no AST por `SymbolId` exige enfiar um `Interner` no lexer/parser
//! e mudar todos os consumidores — semântica, HIR, codegen. Este crate entrega a
//! facilidade e os testes; a migração dos consumidores é o passo seguinte e tem
//! verificação própria (item 4 da meta).
use std::collections::HashMap;

/// Identificador interno de um nome distinto: 4 bytes, `Copy`, sem empréstimo.
///
/// Só é significativo dentro do [`Interner`] que o produziu. Comparar IDs de
/// arenas diferentes é erro lógico — por isso não há ordem total entre arenas,
/// apenas igualdade dentro do mesmo espaço de nomes por construção.
///
/// Guardado como `NonZeroU32` (índice + 1) para que `Option<SymbolId>` e
/// `Option<Name>` custem o mesmo que o valor sem `Option`: a árvore tem
/// milhares de nomes opcionais (argumento nomeado, construtor nomeado,
/// parâmetro sem nome), e o nicho economiza 8 bytes em cada um.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SymbolId(std::num::NonZeroU32);

impl SymbolId {
    /// Devolve o índice numérico bruto do símbolo dentro da sua arena
    /// (a partir de zero).
    pub const fn as_u32(self) -> u32 {
        self.0.get() - 1
    }

    /// Constrói a partir do índice na arena; só o [`Interner`] chama.
    const fn from_index(index: u32) -> Self {
        // `index + 1` nunca é zero enquanto a arena couber em `u32`.
        match std::num::NonZeroU32::new(index.wrapping_add(1)) {
            Some(n) => SymbolId(n),
            None => panic!("arena de nomes esgotou o espaço de u32"),
        }
    }
}

/// Arena de nomes de uma revisão: cada texto distinto vive exatamente uma vez.
///
/// Internar o mesmo texto duas vezes devolve o mesmo [`SymbolId`] sem alocar de
/// novo; é assim que N ocorrências de um identificador custam uma `Box<str>` em
/// vez de N. A estimativa de [`Interner::payload_bytes`] soma os bytes das
/// caixas retidas, sem medir capacidade ociosa do mapa nem overhead do alocador
/// — o mesmo contrato de honestidade do cache de macros.
#[derive(Debug, Default)]
pub struct Interner {
    ids: HashMap<Box<str>, SymbolId>,
    textos: Vec<Box<str>>,
    payload_bytes: usize,
}

impl Interner {
    /// Consulta se um texto já foi internado nesta arena sem interná-lo novamente.
    pub fn lookup(&self, texto: &str) -> Option<SymbolId> {
        self.ids.get(texto).copied()
    }

    /// Cria uma arena vazia, sem alocar.
    ///
    /// ```
    /// let arena = dartforge_intern::Interner::new();
    /// assert!(arena.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Interna o texto e devolve seu índice estável nesta arena.
    ///
    /// Texto repetido não aloca: o índice existente volta direto do mapa.
    ///
    /// ```
    /// let mut arena = dartforge_intern::Interner::new();
    /// let a = arena.intern("somar");
    /// assert_eq!(arena.intern("somar"), a);
    /// assert_eq!(arena.resolve(a), "somar");
    /// ```
    pub fn intern(&mut self, texto: &str) -> SymbolId {
        if let Some(id) = self.ids.get(texto) {
            return *id;
        }
        let id = SymbolId::from_index(self.textos.len() as u32);
        let guardado: Box<str> = texto.into();
        self.payload_bytes = self.payload_bytes.saturating_add(guardado.len());
        self.textos.push(guardado.clone());
        self.ids.insert(guardado, id);
        id
    }

    /// Resolve um índice produzido por [`Interner::intern`] nesta arena.
    ///
    /// # Panics
    ///
    /// Entra em `panic` quando o índice não pertence a esta arena — inclusive
    /// quando veio de outra revisão. Atravessar a fronteira de revisão com texto
    /// em vez de [`SymbolId`] é exatamente o empréstimo que este crate existe
    /// para eliminar, então a confusão falha alto em vez de ler lixo.
    ///
    /// ```
    /// let mut arena = dartforge_intern::Interner::new();
    /// let id = arena.intern("x");
    /// assert_eq!(arena.resolve(id), "x");
    /// ```
    pub fn resolve(&self, id: SymbolId) -> &str {
        &self.textos[id.as_u32() as usize]
    }

    /// Nomes distintos retidos por esta arena.
    pub fn len(&self) -> usize {
        self.textos.len()
    }

    /// Textos na ordem dos índices (`textos()[id.as_u32()]`), para
    /// reconstruir uma arena com os mesmos ids (cache do SDK).
    pub fn textos(&self) -> impl Iterator<Item = &str> {
        self.textos.iter().map(|t| &**t)
    }

    /// Verdadeiro quando nenhum nome foi internado.
    pub fn is_empty(&self) -> bool {
        self.textos.is_empty()
    }

    /// Bytes textuais retidos; somas saturadas impedem orçamento contornado.
    pub fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Esvazia a arena e zera os bytes retidos; IDs antigos não valem mais.
    pub fn clear(&mut self) {
        self.ids.clear();
        self.textos.clear();
        self.payload_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Repetição colapsa: N ocorrências, uma caixa, um índice.
    #[test]
    fn repeticao_colapsa_num_indice() {
        let mut arena = Interner::new();
        let primeiro = arena.intern("contator");
        for _ in 0..100 {
            assert_eq!(arena.intern("contator"), primeiro);
        }
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.payload_bytes(), "contator".len());
        assert_eq!(arena.resolve(primeiro), "contator");
    }

    /// Nomes vizinhos não se confundem, inclusive com prefixos comuns.
    #[test]
    fn distintos_tem_indices_distintos() {
        let mut arena = Interner::new();
        let soma = arena.intern("soma");
        let somar = arena.intern("somar");
        let outra_soma = arena.intern("soma");
        assert_ne!(soma, somar);
        assert_eq!(soma, outra_soma);
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.resolve(somar), "somar");
    }

    /// `clear` descarta tudo e a arena volta a medir zero.
    #[test]
    fn limpar_descarta_os_nomes() {
        let mut arena = Interner::new();
        arena.intern("a");
        arena.intern("bb");
        arena.clear();
        assert!(arena.is_empty());
        assert_eq!(arena.payload_bytes(), 0);
        let id = arena.intern("a");
        assert_eq!(arena.resolve(id), "a");
    }

    /// Índice de outra arena falha alto em vez de resolver errado.
    #[test]
    #[should_panic]
    fn indice_de_outra_arena_falha() {
        let mut velha = Interner::new();
        let id = velha.intern("x");
        let nova = Interner::new();
        let _ = nova.resolve(id);
    }
}
