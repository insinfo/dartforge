//! O ponto fixo do mundo fechado, em forma genérica.
//!
//! É o mesmo laço do `dart2js` (`resolution/enqueuer.dart:283-302`), reduzido
//! ao essencial: conjuntos que só crescem, uma unidade acende no máximo uma
//! vez, e cada símbolo tem a lista de unidades que esperam por ele para não
//! haver varredura a cada mudança — que é o papel dos índices invertidos de
//! `universe/resolution_world_builder.dart:232-240`.
//!
//! Uma unidade acende quando
//!
//! * **algum** dos seus gatilhos está vivo (ou ela não tem gatilho), **e**
//! * **todos** os seus requisitos estão vivos.
//!
//! A conjunção é o que dá granularidade de membro: um método de classe tem
//! requisitos `[a classe, o seletor]`, que é a regra "membro `m` de `C` vive se
//! `C` está instanciada **e** existe um seletor aplicável" da §1.1 do plano.

use std::collections::HashMap;

pub type Sym = u32;

/// Tabela de internação de símbolos. Os nomes são strings como `core.Object`,
/// `dartx.length`, `C#42` ou `sel:toString`.
#[derive(Default)]
pub struct Simbolos {
    mapa: HashMap<String, Sym>,
    nomes: Vec<String>,
}

impl Simbolos {
    pub fn interna(&mut self, nome: &str) -> Sym {
        if let Some(&s) = self.mapa.get(nome) {
            return s;
        }
        let s = self.nomes.len() as Sym;
        self.nomes.push(nome.to_string());
        self.mapa.insert(nome.to_string(), s);
        s
    }
    pub fn procura(&self, nome: &str) -> Option<Sym> {
        self.mapa.get(nome).copied()
    }
    pub fn nome(&self, s: Sym) -> &str {
        &self.nomes[s as usize]
    }
    pub fn total(&self) -> usize {
        self.nomes.len()
    }
}

/// Uma unidade indivisível do arquivo de saída.
#[derive(Default)]
pub struct Unidade {
    /// Viva desde o começo, sem condição (o `export`, o `trackLibraries`).
    pub sempre: bool,
    /// OR: qualquer um vivo satisfaz esta parte.
    pub gatilhos: Vec<Sym>,
    /// AND: todos têm de estar vivos.
    pub requisitos: Vec<Sym>,
    /// Símbolos que o texto desta unidade referencia.
    pub refs: Vec<Sym>,
    /// Seletores que o texto desta unidade usa (`sel:…`), já internados.
    pub seletores: Vec<Sym>,
}

/// Roda o ponto fixo e devolve o vetor de "unidade viva?".
pub fn resolver(unidades: &[Unidade], raizes: &[Sym], total_simbolos: usize) -> Vec<bool> {
    // Índice invertido: símbolo → unidades que o mencionam na condição.
    let mut espera: Vec<Vec<u32>> = vec![Vec::new(); total_simbolos];
    for (i, u) in unidades.iter().enumerate() {
        for &s in u.gatilhos.iter().chain(u.requisitos.iter()) {
            espera[s as usize].push(i as u32);
        }
    }
    let mut viva = vec![false; unidades.len()];
    let mut simbolo_vivo = vec![false; total_simbolos];
    let mut fila: Vec<u32> = Vec::new();
    let mut pendentes: Vec<Sym> = Vec::new();

    let satisfeita = |u: &Unidade, sv: &[bool]| {
        (u.gatilhos.is_empty() || u.gatilhos.iter().any(|&s| sv[s as usize]))
            && u.requisitos.iter().all(|&s| sv[s as usize])
    };

    for (i, u) in unidades.iter().enumerate() {
        if u.sempre {
            viva[i] = true;
            fila.push(i as u32);
        }
    }
    pendentes.extend_from_slice(raizes);

    loop {
        // Acende os símbolos pendentes e reexamina quem esperava por eles.
        while let Some(s) = pendentes.pop() {
            if simbolo_vivo[s as usize] {
                continue;
            }
            simbolo_vivo[s as usize] = true;
            for &i in &espera[s as usize] {
                let i = i as usize;
                if !viva[i] && satisfeita(&unidades[i], &simbolo_vivo) {
                    viva[i] = true;
                    fila.push(i as u32);
                }
            }
        }
        // Propaga o que as unidades acesas referenciam.
        let Some(i) = fila.pop() else { break };
        let u = &unidades[i as usize];
        for &s in u.refs.iter().chain(u.seletores.iter()) {
            if !simbolo_vivo[s as usize] {
                pendentes.push(s);
            }
        }
    }
    viva
}

/// Caminho mais curto de uma raiz até `destino`, em símbolos — a ferramenta de
/// diagnóstico que responde "**quem** puxou isto para o arquivo?". Sem ela, um
/// alcance grande demais só se investiga por tentativa.
pub fn caminho(unidades: &[Unidade], raizes: &[Sym], total_simbolos: usize, destino: Sym) -> Option<Vec<Sym>> {
    let mut espera: Vec<Vec<u32>> = vec![Vec::new(); total_simbolos];
    for (i, u) in unidades.iter().enumerate() {
        for &s in u.gatilhos.iter().chain(u.requisitos.iter()) {
            espera[s as usize].push(i as u32);
        }
    }
    let mut viva = vec![false; unidades.len()];
    let mut vivo = vec![false; total_simbolos];
    // Para cada símbolo, de qual símbolo ele veio (largura, para ser o mais curto).
    let mut veio: Vec<Option<Sym>> = vec![None; total_simbolos];
    let mut fila_sym: std::collections::VecDeque<Sym> = std::collections::VecDeque::new();
    let mut fila_un: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
    for (i, u) in unidades.iter().enumerate() {
        if u.sempre {
            viva[i] = true;
            fila_un.push_back(i as u32);
        }
    }
    for &r in raizes {
        if !vivo[r as usize] {
            vivo[r as usize] = true;
            fila_sym.push_back(r);
        }
    }
    loop {
        while let Some(s) = fila_sym.pop_front() {
            if s == destino {
                let mut v = vec![s];
                let mut cur = s;
                while let Some(p) = veio[cur as usize] {
                    v.push(p);
                    cur = p;
                }
                v.reverse();
                return Some(v);
            }
            for &i in &espera[s as usize] {
                let i = i as usize;
                let u = &unidades[i];
                let ok = (u.gatilhos.is_empty() || u.gatilhos.iter().any(|&x| vivo[x as usize]))
                    && u.requisitos.iter().all(|&x| vivo[x as usize]);
                if !viva[i] && ok {
                    viva[i] = true;
                    fila_un.push_back(i as u32);
                }
            }
        }
        let Some(i) = fila_un.pop_front() else { return None };
        let u = &unidades[i as usize];
        let de = u.gatilhos.first().or(u.requisitos.first()).copied();
        for &s in u.refs.iter().chain(u.seletores.iter()) {
            if !vivo[s as usize] {
                vivo[s as usize] = true;
                veio[s as usize] = de;
                fila_sym.push_back(s);
            }
        }
    }
}

/// Refaz a descoberta a seco e devolve as unidades que deveriam estar vivas e
/// não estão — o equivalente de `checkEnqueuerConsistency`
/// (`pkg/compiler/lib/src/enqueue.dart:142-156`). Um resultado não vazio é um
/// defeito do ponto fixo, não do programa compilado.
pub fn conferir(unidades: &[Unidade], viva: &[bool], total_simbolos: usize) -> Vec<usize> {
    // Reconstrói o conjunto de símbolos vivos a partir **só** das unidades
    // marcadas vivas, sem reusar o estado do ponto fixo.
    let mut simbolo_vivo = vec![false; total_simbolos];
    for (i, u) in unidades.iter().enumerate() {
        if !viva[i] {
            continue;
        }
        for &s in u.refs.iter().chain(u.seletores.iter()) {
            simbolo_vivo[s as usize] = true;
        }
        for &s in u.gatilhos.iter().chain(u.requisitos.iter()) {
            simbolo_vivo[s as usize] = true;
        }
    }
    let mut faltando = Vec::new();
    for (i, u) in unidades.iter().enumerate() {
        if viva[i] {
            continue;
        }
        let ok = (u.gatilhos.is_empty() || u.gatilhos.iter().any(|&s| simbolo_vivo[s as usize]))
            && u.requisitos.iter().all(|&s| simbolo_vivo[s as usize]);
        if ok && !u.gatilhos.is_empty() {
            faltando.push(i);
        }
    }
    faltando
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cadeia_simples() {
        let mut s = Simbolos::default();
        let (a, b, c) = (s.interna("a"), s.interna("b"), s.interna("c"));
        let us = vec![
            Unidade { gatilhos: vec![a], refs: vec![b], ..Default::default() },
            Unidade { gatilhos: vec![b], ..Default::default() },
            Unidade { gatilhos: vec![c], ..Default::default() },
        ];
        let viva = resolver(&us, &[a], s.total());
        assert_eq!(viva, vec![true, true, false], "c não é alcançado a partir de a");
    }

    #[test]
    fn membro_exige_classe_e_seletor() {
        let mut s = Simbolos::default();
        let classe = s.interna("core.C");
        let sel_f = s.interna("sel:f");
        let sel_g = s.interna("sel:g");
        let us = vec![
            Unidade { gatilhos: vec![classe], ..Default::default() },
            // membro f: vive porque a classe vive e `f` é chamado
            Unidade { requisitos: vec![classe, sel_f], ..Default::default() },
            // membro g: a classe vive, mas ninguém chama `g`
            Unidade { requisitos: vec![classe, sel_g], ..Default::default() },
        ];
        let viva = resolver(&us, &[classe, sel_f], s.total());
        assert_eq!(viva, vec![true, true, false]);
    }

    #[test]
    fn seletor_usado_por_membro_vivo_acende_outro() {
        let mut s = Simbolos::default();
        let c = s.interna("core.C");
        let d = s.interna("core.D");
        let sel_f = s.interna("sel:f");
        let sel_h = s.interna("sel:h");
        let us = vec![
            Unidade { gatilhos: vec![c], ..Default::default() },
            Unidade { gatilhos: vec![d], ..Default::default() },
            // C.f usa `x.h()` e menciona D
            Unidade { requisitos: vec![c, sel_f], refs: vec![d], seletores: vec![sel_h], ..Default::default() },
            // D.h só vive porque C.f usa o seletor `h`
            Unidade { requisitos: vec![d, sel_h], ..Default::default() },
        ];
        let viva = resolver(&us, &[c, sel_f], s.total());
        assert_eq!(viva, vec![true, true, true, true]);
    }
}
