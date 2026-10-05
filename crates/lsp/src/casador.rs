//! O `FuzzyMatcher` do analyzer (`AN:src/utilities/fuzzy_matcher.dart`;
//! docs/LSP-ESPECIFICACAO.md §14.7.2): o score de um candidato contra o
//! padrão digitado, em unidades UTF-16 como o Dart, com os três estilos
//! (`TEXT` no completar, `SYMBOL` no `workspace/symbol`, `FILENAME`).
//! Escrito sem compilar nem executar (2026-10-05).

/// `MatchStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Estilo {
    Texto,
    Simbolo,
    Arquivo,
}

/// `CharRole`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Papel {
    Nenhum,
    Separador,
    Cauda,
    CaudaMaiuscula,
    Cabeca,
}

/// `_CharType` (`NONE`, `PUNCT`, `LOWER`, `UPPER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tipo {
    Nenhum = 0,
    Pontuacao = 1,
    Minuscula = 2,
    Maiuscula = 3,
}

const MAX_ENTRADA: usize = 127;
const MAX_PADRAO: usize = 63;
const MIN_SCORE: i32 = -10000;

/// A tabela `TYPES` (os códigos 0..127).
const TIPOS: &[u8; 128] = b"0000000000000000000000000000000010000000000000112222222222100000\
0333333333333333333333333330000002222222222222222222222222200000";

fn tipo(c: u16) -> Tipo {
    if c >= 128 {
        return Tipo::Minuscula;
    }
    match TIPOS[c as usize] {
        b'1' => Tipo::Pontuacao,
        b'2' => Tipo::Minuscula,
        b'3' => Tipo::Maiuscula,
        _ => Tipo::Nenhum,
    }
}

/// `toLowerCase` em UTF-16, mantendo o comprimento (o que o casamento por
/// índice exige; um caractere cuja minúscula muda o comprimento fica como
/// está).
fn minusculas(s: &[u16]) -> Vec<u16> {
    let texto = String::from_utf16_lossy(s);
    let baixo: Vec<u16> = texto.to_lowercase().encode_utf16().collect();
    if baixo.len() == s.len() {
        return baixo;
    }
    s.iter().map(|&c| if (b'A' as u16..=b'Z' as u16).contains(&c) { c + 32 } else { c }).collect()
}

/// Um casador para um padrão.
pub(crate) struct Casador {
    padrao: Vec<u16>,
    padrao_min: Vec<u16>,
    curto: Vec<u16>,
    estilo: Estilo,
    sensivel: bool,
    deslocamento: usize,
    papeis_padrao: Vec<Papel>,
    papeis: Vec<Papel>,
    tabela: Vec<Vec<i32>>,
    escala: f64,
    ultimo_comprimento: usize,
}

impl Casador {
    /// `FuzzyMatcher(pattern, matchStyle)`.
    pub(crate) fn novo(padrao: &str, estilo: Estilo) -> Casador {
        let mut p: Vec<u16> = padrao.encode_utf16().collect();
        p.truncate(MAX_PADRAO);
        let padrao_min = minusculas(&p);
        let sensivel = p != padrao_min;
        let curto = padrao_min[..padrao_min.len().min(3)].to_vec();
        let deslocamento = p.len() + 1;
        let tabela = vec![vec![0i32; 2 * deslocamento]; MAX_ENTRADA + 1];
        let mut c = Casador {
            padrao: p,
            padrao_min,
            curto,
            estilo,
            sensivel,
            deslocamento,
            papeis_padrao: Vec::new(),
            papeis: vec![Papel::Nenhum; MAX_ENTRADA],
            tabela,
            escala: 0.0,
            ultimo_comprimento: 0,
        };
        let mut papeis = vec![Papel::Nenhum; c.padrao.len()];
        let padrao = c.padrao.clone();
        c.mapear(&padrao, &mut papeis);
        c.papeis_padrao = papeis;
        let max = if estilo == Estilo::Texto { 6.0 } else { 4.0 };
        c.escala = if c.padrao.is_empty() { 0.0 } else { 1.0 / (max * c.padrao.len() as f64) };
        c
    }

    /// `fuzzyMap`.
    fn mapear(&self, s: &[u16], papeis: &mut [Papel]) {
        let mut anterior = Tipo::Nenhum;
        for (i, &c) in s.iter().enumerate() {
            let t = tipo(c);
            let mut papel = Papel::Nenhum;
            match t {
                Tipo::Minuscula => papel = if anterior <= Tipo::Pontuacao { Papel::Cabeca } else { Papel::Cauda },
                Tipo::Maiuscula => {
                    papel = Papel::Cabeca;
                    let seguinte_minuscula = i + 1 < s.len() && (b'a' as u16..=b'z' as u16).contains(&s[i + 1]);
                    if anterior == Tipo::Maiuscula && !seguinte_minuscula {
                        papel = Papel::CaudaMaiuscula;
                    }
                }
                Tipo::Pontuacao => {
                    let separa = (self.estilo == Estilo::Arquivo && c == b'/' as u16)
                        || (self.estilo == Estilo::Simbolo && (c == b'.' as u16 || c == b':' as u16 || c == b' ' as u16));
                    if separa {
                        papel = Papel::Separador;
                    }
                }
                Tipo::Nenhum => {}
            }
            papeis[i] = papel;
            anterior = t;
        }
        let mut i = s.len();
        while i > 0 && papeis[i - 1] == Papel::Separador {
            papeis[i - 1] = Papel::Nenhum;
            i -= 1;
        }
    }

    fn score_em(&self, i: usize, j: usize, k: usize) -> i32 {
        self.tabela[i][j + k * self.deslocamento] >> 1
    }

    fn melhor_k(&self, i: usize, j: usize) -> usize {
        if self.score_em(i, j, 0) < self.score_em(i, j, 1) { 1 } else { 0 }
    }

    fn k_anterior(&self, i: usize, j: usize, k: usize) -> usize {
        (self.tabela[i][j + k * self.deslocamento] & 1) as usize
    }

    /// `computeScore`.
    fn calcular(&mut self, cand: &[u16], cand_min: &[u16]) -> i32 {
        let m = self.padrao.len();
        let o = self.deslocamento;
        for j in 0..=m {
            self.tabela[0][j] = if j == 0 { 0 } else { MIN_SCORE << 1 };
            self.tabela[0][o + j] = MIN_SCORE << 1;
        }
        let mut segmentos = 1;
        let mut ultimo_inicio = 0usize;
        for i in 0..cand.len() {
            if self.papeis[i] == Papel::Separador {
                segmentos += 1;
                ultimo_inicio = i + 1;
            }
        }
        for i in 1..=cand.len() {
            let cabeca = self.papeis[i - 1] == Papel::Cabeca;
            if self.papeis[i - 1] == Papel::Separador && segmentos > 1 {
                segmentos -= 1;
            }
            let seg_score = if segmentos > 1 { 0 } else { 1 };
            let mut pena = 0;
            if segmentos == 1 && cabeca && self.estilo != Estilo::Texto {
                pena += 1;
            }
            if i - 1 == ultimo_inicio {
                pena += 3;
            }
            for j in 0..=m {
                self.tabela[i][o + j] = MIN_SCORE << 1;
                if segmentos > 1 && j == m {
                    self.tabela[i][j] = MIN_SCORE << 1;
                    continue;
                }
                let k = self.melhor_k(i - 1, j);
                let mut pular = self.score_em(i - 1, j, k);
                if j != m {
                    pular -= pena;
                }
                self.tabela[i][j] = (pular << 1) + k as i32;
                if j == 0 || cand_min[i - 1] != self.padrao_min[j - 1] {
                    continue;
                }
                let mut ponto = seg_score;
                if self.papeis[i - 1] == Papel::Cauda && self.papeis_padrao[j - 1] == Papel::Cabeca {
                    if j > 1 {
                        continue;
                    }
                    let fim = cand_min.len().min(i - 1 + self.curto.len());
                    if self.curto[..] != cand_min[i - 1..fim] {
                        continue;
                    }
                    ponto -= 4;
                }
                if cand[i - 1] == self.padrao[j - 1] || (cabeca && (!self.sensivel || self.papeis_padrao[j - 1] == Papel::Cabeca)) {
                    ponto += 1;
                }
                for k in 0..2 {
                    let mut s = self.score_em(i - 1, j - 1, k) + ponto;
                    let anterior_casou = k == 1;
                    let consecutivo = anterior_casou || i - 1 == 0 || i - 1 == ultimo_inicio;
                    if consecutivo || (self.estilo == Estilo::Texto && j - 1 == 0) {
                        s += if self.estilo == Estilo::Texto { 4 } else { 2 };
                    }
                    if !anterior_casou && matches!(self.papeis[i - 1], Papel::Cauda | Papel::CaudaMaiuscula) {
                        s -= 3;
                    }
                    if s > (self.tabela[i][o + j] >> 1) {
                        self.tabela[i][o + j] = (s << 1) + k as i32;
                    }
                }
            }
        }
        let n = cand.len();
        self.score_em(n, m, self.melhor_k(n, m))
    }

    /// `isPoorMatch`.
    fn ruim(&self) -> bool {
        if self.padrao.len() < 2 {
            return false;
        }
        let mut i = self.ultimo_comprimento;
        let mut j = self.padrao.len();
        let mut k = self.melhor_k(i, j);
        let mut contador = 0;
        let mut comprimento = 0;
        while i > 0 {
            let pega = k == 1;
            k = self.k_anterior(i, j, k);
            if pega {
                comprimento += 1;
                if k == 0 && comprimento < 3 && self.papeis[i - 1] == Papel::Cauda {
                    contador += 1;
                    if contador > 1 {
                        return true;
                    }
                }
                j -= 1;
            } else {
                comprimento = 0;
            }
            i -= 1;
        }
        false
    }

    /// `match`.
    fn casa(&mut self, cand: &[u16], cand_min: &[u16]) -> bool {
        let mut i = 0;
        let mut j = 0;
        while i < cand_min.len() && j < self.padrao_min.len() {
            if cand_min[i] == self.padrao_min[j] {
                j += 1;
            }
            i += 1;
        }
        if j != self.padrao_min.len() {
            return false;
        }
        let mut papeis = std::mem::take(&mut self.papeis);
        self.mapear(cand, &mut papeis);
        self.papeis = papeis;
        if self.estilo != Estilo::Texto {
            let mut sep = cand_min.len() as isize - 1;
            while sep >= i as isize && self.papeis[sep as usize] != Papel::Separador {
                sep -= 1;
            }
            if sep >= i as isize {
                let ultimo = *self.padrao_min.last().unwrap_or(&0);
                return cand_min[sep as usize..].contains(&ultimo);
            }
        }
        true
    }

    /// `score(candidate)`: −1 quando não casa; 1 no padrão vazio.
    pub(crate) fn score(&mut self, candidato: &str) -> f64 {
        let mut cand: Vec<u16> = candidato.encode_utf16().collect();
        if cand.len() > MAX_ENTRADA {
            if self.estilo == Estilo::Arquivo {
                cand = cand[cand.len() - MAX_ENTRADA..].to_vec();
            } else {
                cand.truncate(MAX_ENTRADA);
            }
        }
        if self.padrao.is_empty() {
            return 1.0;
        }
        self.ultimo_comprimento = cand.len();
        let cand_min = minusculas(&cand);
        if self.casa(&cand, &cand_min) {
            let mut s = self.calcular(&cand, &cand_min);
            if s > MIN_SCORE / 2 && !self.ruim() {
                if self.padrao.len() == cand.len() {
                    return 1.0;
                }
                if s < 0 {
                    s = 0;
                }
                let n = s as f64 * self.escala;
                return if n > 1.0 { 1.0 } else { n };
            }
        }
        -1.0
    }
}

#[cfg(test)]
mod testes {
    use super::{Casador, Estilo};

    fn s(p: &str, c: &str) -> f64 {
        Casador::novo(p, Estilo::Texto).score(c)
    }

    #[test]
    fn exemplos_da_especificacao() {
        assert_eq!(s("St", "StateError"), 1.0);
        assert_eq!(s("St", "StackTrace"), 1.0);
        assert_eq!(s("st", "StateError"), 1.0);
        assert!((s("st", "toString") - 0.75).abs() < 1e-9);
        assert_eq!(s("pr", "print"), 1.0);
        assert_eq!(s("print", "print"), 1.0);
        assert_eq!(s("x", "max"), 0.0);
        assert_eq!(s("", "qualquer"), 1.0);
        assert_eq!(s("zz", "print"), -1.0);
    }
}
