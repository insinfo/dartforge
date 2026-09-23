//! Comparação com o oráculo e placar por código.
//!
//! Chave: `(arquivo, código, offset UTF-16, comprimento)`. Um par com a mesma
//! chave é **acerto**; entre os acertos, severidade, mensagem ou correção
//! diferentes contam como **mensagem errada**. O que sobra de cada lado, no
//! mesmo arquivo e com o mesmo código, é pareado em ordem de offset como
//! **posição errada**; o resto é **falso positivo** (só nosso) ou **falso
//! negativo** (só do oráculo). Tudo em `BTreeMap`: o texto do placar não
//! depende da ordem em que os lotes terminaram.

use crate::oraculo::Registro;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Contadores de um código.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Conta {
    pub oraculo: usize,
    pub nosso: usize,
    pub acertos: usize,
    pub mensagem_errada: usize,
    pub posicao_errada: usize,
    pub falsos_positivos: usize,
    pub falsos_negativos: usize,
}

impl Conta {
    pub fn somar(&mut self, o: &Conta) {
        self.oraculo += o.oraculo;
        self.nosso += o.nosso;
        self.acertos += o.acertos;
        self.mensagem_errada += o.mensagem_errada;
        self.posicao_errada += o.posicao_errada;
        self.falsos_positivos += o.falsos_positivos;
        self.falsos_negativos += o.falsos_negativos;
    }

    /// Acerto exato (posição e mensagem) em todos os casos, com pelo menos um.
    pub fn perfeito(&self) -> bool {
        self.oraculo > 0 && self.acertos == self.oraculo && self.nosso == self.oraculo && self.mensagem_errada == 0
    }
}

/// Placar por código, e amostras de divergência por código.
#[derive(Debug, Default, Clone)]
pub struct Placar {
    pub por_codigo: BTreeMap<String, Conta>,
    /// Até 3 exemplos por (código, categoria): `arquivo:linha:coluna texto`.
    pub amostras: BTreeMap<(String, &'static str), Vec<String>>,
}

impl Placar {
    pub fn total(&self) -> Conta {
        let mut t = Conta::default();
        for c in self.por_codigo.values() {
            t.somar(c);
        }
        t
    }

    pub fn somar(&mut self, outro: &Placar) {
        for (k, c) in &outro.por_codigo {
            self.por_codigo.entry(k.clone()).or_default().somar(c);
        }
        for (k, v) in &outro.amostras {
            let d = self.amostras.entry(k.clone()).or_default();
            for s in v {
                if d.len() < 3 {
                    d.push(s.clone());
                }
            }
        }
    }

    fn amostra(&mut self, codigo: &str, cat: &'static str, r: &Registro, extra: &str) {
        let v = self.amostras.entry((codigo.to_string(), cat)).or_default();
        if v.len() < 3 {
            v.push(format!("{}:{}:{} {}{extra}", r.arquivo, r.line, r.column, r.problem_message));
        }
    }

    /// Compara os diagnósticos de um grupo (os dois lados relativos à mesma raiz).
    pub fn comparar(&mut self, oraculo: &[Registro], nosso: &[Registro]) {
        // (arquivo, código) → registros de cada lado.
        type Lados<'r> = (Vec<&'r Registro>, Vec<&'r Registro>);
        let mut lados: BTreeMap<(&str, &str), Lados<'_>> = BTreeMap::new();
        for r in oraculo {
            lados.entry((&r.arquivo, &r.code)).or_default().0.push(r);
        }
        for r in nosso {
            lados.entry((&r.arquivo, &r.code)).or_default().1.push(r);
        }
        for ((_, codigo), (mut o, mut n)) in lados {
            o.sort();
            n.sort();
            let mut conta = Conta { oraculo: o.len(), nosso: n.len(), ..Conta::default() };
            let mut resto_o = Vec::new();
            let mut usados = vec![false; n.len()];
            for ro in &o {
                let achado = (0..n.len()).find(|&i| !usados[i] && n[i].offset == ro.offset && n[i].length == ro.length);
                match achado {
                    Some(i) => {
                        usados[i] = true;
                        conta.acertos += 1;
                        let rn = n[i];
                        if rn.severity != ro.severity
                            || rn.problem_message != ro.problem_message
                            || rn.correction_message != ro.correction_message
                        {
                            conta.mensagem_errada += 1;
                            let extra = format!("  <> nosso: {}", rn.problem_message);
                            self.amostra(codigo, "mensagem", ro, &extra);
                        }
                    }
                    None => resto_o.push(*ro),
                }
            }
            let resto_n: Vec<&Registro> = (0..n.len()).filter(|&i| !usados[i]).map(|i| n[i]).collect();
            let pares = resto_o.len().min(resto_n.len());
            conta.posicao_errada = pares;
            for k in 0..pares {
                let extra = format!("  <> nosso {}:{} (+{})", resto_n[k].line, resto_n[k].column, resto_n[k].length);
                self.amostra(codigo, "posição", resto_o[k], &extra);
            }
            conta.falsos_negativos = resto_o.len() - pares;
            conta.falsos_positivos = resto_n.len() - pares;
            for r in &resto_o[pares..] {
                self.amostra(codigo, "FN", r, "");
            }
            for r in &resto_n[pares..] {
                self.amostra(codigo, "FP", r, "");
            }
            self.por_codigo.entry(codigo.to_string()).or_default().somar(&conta);
        }
    }

    /// Tabela por código (ordem alfabética), com a linha de total.
    pub fn tabela(&self, titulo: &str) -> String {
        let mut s = String::new();
        let t = self.total();
        let _ = writeln!(
            s,
            "{titulo}: oráculo {} | nosso {} | acertos {} (mensagem errada {}) | posição errada {} | FP {} | FN {}",
            t.oraculo, t.nosso, t.acertos, t.mensagem_errada, t.posicao_errada, t.falsos_positivos, t.falsos_negativos
        );
        let perfeitos = self.por_codigo.values().filter(|c| c.perfeito()).count();
        let com_casos = self.por_codigo.values().filter(|c| c.oraculo > 0).count();
        let _ = writeln!(s, "códigos com 100% (posição e mensagem): {perfeitos} de {com_casos} com casos no oráculo");
        let _ = writeln!(s, "{:<58} {:>7} {:>7} {:>7} {:>6} {:>6} {:>6} {:>6}", "código", "oráculo", "nosso", "acerto", "msg≠", "pos≠", "FP", "FN");
        for (k, c) in &self.por_codigo {
            let _ = writeln!(
                s,
                "{:<58} {:>7} {:>7} {:>7} {:>6} {:>6} {:>6} {:>6}{}",
                k,
                c.oraculo,
                c.nosso,
                c.acertos,
                c.mensagem_errada,
                c.posicao_errada,
                c.falsos_positivos,
                c.falsos_negativos,
                if c.perfeito() { "  100%" } else { "" }
            );
        }
        s
    }

    /// As amostras de divergência, por código e categoria.
    pub fn amostras_texto(&self) -> String {
        let mut s = String::new();
        for ((c, cat), v) in &self.amostras {
            for a in v {
                let _ = writeln!(s, "  [{c}] {cat}: {a}");
            }
        }
        s
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn r(arq: &str, code: &str, off: usize, len: usize, msg: &str) -> Registro {
        Registro {
            arquivo: arq.into(),
            offset: off,
            length: len,
            code: code.into(),
            severity: "ERROR".into(),
            tipo: "COMPILE_TIME_ERROR".into(),
            line: 1,
            column: off + 1,
            problem_message: msg.into(),
            correction_message: None,
        }
    }

    #[test]
    fn categorias() {
        let o = vec![r("a", "x", 1, 1, "m"), r("a", "x", 5, 1, "m"), r("a", "y", 2, 2, "m"), r("a", "z", 3, 1, "m")];
        let n = vec![r("a", "x", 1, 1, "outra"), r("a", "x", 9, 1, "m"), r("a", "w", 0, 1, "m"), r("a", "z", 3, 1, "m")];
        let mut p = Placar::default();
        p.comparar(&o, &n);
        let x = p.por_codigo["x"];
        assert_eq!((x.acertos, x.mensagem_errada, x.posicao_errada), (1, 1, 1));
        assert_eq!(p.por_codigo["y"].falsos_negativos, 1);
        assert_eq!(p.por_codigo["w"].falsos_positivos, 1);
        assert!(p.por_codigo["z"].perfeito());
        assert!(!x.perfeito());
    }
}
