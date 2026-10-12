//! Porte de `short/line_splitting/*`: o divisor de linhas (`LineSplitter`),
//! os estados de solução (`SolveState`), a fila de prioridade com fusão de
//! estados sobrepostos (`SolveStateQueue`), o `RuleSet` e o `SplitSet`.
//!
//! A busca é a mesma do original, inclusive o limite de 5000 tentativas e a
//! ordem de exploração, porque a escolha entre soluções de mesmo custo depende
//! dela.

use std::collections::HashSet;

use super::arena::{Arenas, ChunkId, DEVE_QUEBRAR, NAO_QUEBRA, RegraId, recuo};
use super::escritor::{CacheDeBlocos, formatar_bloco};

const MAX_TENTATIVAS: usize = 5000;

/// `SplitSet`.
#[derive(Debug, Clone)]
pub struct Quebras {
    colunas: Vec<i32>,
    pub custo: i32,
}

impl Quebras {
    /// `shouldSplitAt(index)`.
    pub fn quebra_em(&self, i: usize) -> bool {
        i < self.colunas.len() && self.colunas[i] != -1
    }

    /// `getColumn(index)`.
    pub fn coluna(&self, i: usize) -> i32 {
        self.colunas[i]
    }
}

/// `RuleSet`: valores por índice de regra.
#[derive(Debug, Clone)]
struct Valores(Vec<Option<i32>>);

impl Valores {
    fn indice(a: &Arenas, r: RegraId) -> Option<usize> {
        a.regras[r as usize].indice
    }

    fn contem(&self, a: &Arenas, r: RegraId) -> bool {
        if a.endurecida(r) {
            return true;
        }
        Self::indice(a, r).and_then(|i| self.0.get(i).copied().flatten()).is_some()
    }

    fn valor(&self, a: &Arenas, r: RegraId) -> i32 {
        if a.endurecida(r) {
            return a.valor_total(r);
        }
        Self::indice(a, r).and_then(|i| self.0.get(i).copied().flatten()).unwrap_or(NAO_QUEBRA)
    }

    fn bruto(&self, a: &Arenas, r: RegraId) -> Option<i32> {
        Self::indice(a, r).and_then(|i| self.0.get(i).copied().flatten())
    }

    fn definir(&mut self, a: &Arenas, r: RegraId, v: i32) {
        if let Some(i) = Self::indice(a, r)
            && i < self.0.len()
        {
            self.0[i] = Some(v);
        }
    }

    /// `tryBind(rules, rule, value, onSplitRule)`.
    fn tentar_ligar(&mut self, a: &Arenas, r: RegraId, valor: i32, ao_quebrar: &mut dyn FnMut(RegraId)) -> bool {
        debug_assert!(!a.endurecida(r));
        self.definir(a, r, valor);
        let mut i = 0;
        while let Some(outra) = a.restrita(r, i) {
            i += 1;
            let outro_valor = if a.endurecida(outra) {
                Some(a.valor_total(outra))
            } else {
                self.bruto(a, outra)
            };
            let restricao = a.restringe(r, valor, outra);
            match outro_valor {
                None => {
                    if restricao == Some(DEVE_QUEBRAR) {
                        if a.num_valores(outra) == 2 {
                            if !self.tentar_ligar(a, outra, 1, ao_quebrar) {
                                return false;
                            }
                        } else {
                            ao_quebrar(outra);
                        }
                    } else if let Some(c) = restricao {
                        if !self.tentar_ligar(a, outra, c, ao_quebrar) {
                            return false;
                        }
                    }
                }
                Some(ov) => {
                    if restricao == Some(DEVE_QUEBRAR) {
                        if ov == NAO_QUEBRA {
                            return false;
                        }
                    } else if let Some(c) = restricao
                        && ov != c
                    {
                        return false;
                    }
                    let inversa = a.restringe(outra, ov, r);
                    if inversa == Some(DEVE_QUEBRAR) {
                        if valor == NAO_QUEBRA {
                            return false;
                        }
                    } else if let Some(c) = inversa
                        && valor != c
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// `LineSplitter`.
pub struct DivisorDeLinhas {
    chunks: Vec<ChunkId>,
    regras: Vec<RegraId>,
    recuo_do_bloco: i32,
}

/// `SolveState`.
struct Estado {
    valores: Valores,
    quebras: Quebras,
    excesso: i32,
    completo: bool,
    vivas: Vec<RegraId>,
    vivas_set: HashSet<RegraId>,
    restricoes: Option<Vec<(RegraId, i32)>>,
    ligadas: Option<HashSet<RegraId>>,
    soltas: Option<HashSet<RegraId>>,
    restricoes_soltas: Option<Vec<(RegraId, Vec<i32>)>>,
    ligadas_em_linhas_soltas: Option<Vec<RegraId>>,
}

impl DivisorDeLinhas {
    /// `LineSplitter(writer, chunks, blockIndentation)`.
    pub fn novo(a: &mut Arenas, chunks: Vec<ChunkId>, recuo_do_bloco: i32) -> DivisorDeLinhas {
        let mut regras: Vec<RegraId> = Vec::new();
        let mut vistas = HashSet::new();
        for &c in &chunks {
            let r = a.chunks[c as usize].regra;
            if vistas.insert(r) {
                regras.push(r);
            }
        }
        for (i, &r) in regras.iter().enumerate() {
            a.regras[r as usize].indice = Some(i);
        }
        for &r in &regras {
            a.esquecer_nao_usadas(r);
        }
        DivisorDeLinhas {
            chunks,
            regras,
            recuo_do_bloco,
        }
    }

    /// `apply()`.
    pub fn aplicar(&self, a: &mut Arenas, cache: &mut CacheDeBlocos) -> Quebras {
        let mut fila = Fila { itens: Vec::new() };
        let inicial = self.novo_estado(a, cache, Valores(vec![None; self.regras.len()]));
        let mut melhor_idx;
        let mut estados: Vec<Estado> = vec![inicial];
        melhor_idx = 0usize;
        fila.adicionar(self, a, &mut estados, 0);

        let mut tentativas = 0;
        while !fila.itens.is_empty() {
            let atual = fila.remover_primeiro(self, a, &mut estados);
            if self.melhor_que(&estados[atual], &estados[melhor_idx]) {
                melhor_idx = atual;
                if estados[melhor_idx].excesso == 0 {
                    break;
                }
            }
            let ultrapassou = tentativas > MAX_TENTATIVAS;
            tentativas += 1;
            if ultrapassou {
                break;
            }
            self.expandir(a, cache, &mut estados, &mut fila, atual);
        }
        estados.swap_remove(melhor_idx).quebras
    }

    fn melhor_que(&self, e: &Estado, outro: &Estado) -> bool {
        if !e.completo {
            return false;
        }
        if e.excesso != outro.excesso {
            return e.excesso < outro.excesso;
        }
        e.quebras.custo < outro.quebras.custo
    }

    fn novo_estado(&self, a: &mut Arenas, cache: &mut CacheDeBlocos, valores: Valores) -> Estado {
        let mut e = Estado {
            valores,
            quebras: Quebras {
                colunas: Vec::new(),
                custo: 0,
            },
            excesso: 0,
            completo: true,
            vivas: Vec::new(),
            vivas_set: HashSet::new(),
            restricoes: None,
            ligadas: None,
            soltas: None,
            restricoes_soltas: None,
            ligadas_em_linhas_soltas: None,
        };
        self.calcular_quebras(a, &mut e);
        self.calcular_custo(a, cache, &mut e);
        e
    }

    fn calcular_quebras(&self, a: &mut Arenas, e: &mut Estado) {
        let mut usados = Vec::new();
        for &c in &self.chunks {
            let r = a.chunks[c as usize].regra;
            if a.quebra(r, e.valores.valor(a, r), c) {
                let n = a.chunks[c as usize].ninho;
                if a.marcar_ninho(n) {
                    usados.push(n);
                    a.limpar_total(n);
                }
            }
        }
        for &n in &usados {
            a.atualizar_total(n);
        }
        for &n in &usados {
            a.desmarcar_ninho(n);
        }
        let mut colunas = vec![-1; self.chunks.len()];
        for (i, &c) in self.chunks.iter().enumerate() {
            let r = a.chunks[c as usize].regra;
            if a.quebra(r, e.valores.valor(a, r), c) {
                let mut col = 0;
                let ch = &a.chunks[c as usize];
                if !ch.rente {
                    col = self.recuo_do_bloco + ch.recuo;
                    col += a.total_ninho(ch.ninho);
                    let valores = &e.valores;
                    if a.recua_bloco(c, &|r| valores.valor(a, r)) {
                        col += recuo::EXPRESSION;
                    }
                }
                colunas[i] = col;
            }
        }
        e.quebras = Quebras { colunas, custo: 0 };
    }

    fn calcular_custo(&self, a: &mut Arenas, cache: &mut CacheDeBlocos, e: &mut Estado) {
        let mut custo = 0;
        let mut comprimento: i64 = 0;
        let mut achou_regras = false;
        let mut inicio = 0usize;
        let largura = a.largura as i64;

        let mut spans_quebrados = Vec::new();
        let mut ninho_anterior = None;

        let fim_de_linha = |a: &mut Arenas, e: &mut Estado, fim: usize, comprimento: i64, inicio: &mut usize, achou: &mut bool| {
            if comprimento > largura {
                e.excesso += (comprimento - largura) as i32;
                if !*achou {
                    for i in *inicio..fim {
                        let r = a.chunks[self.chunks[i] as usize].regra;
                        if adicionar_vivas(a, e, r) {
                            *achou = true;
                        }
                    }
                }
            }
            *inicio = fim;
        };

        for i in 0..self.chunks.len() {
            let c = self.chunks[i];
            if e.quebras.quebra_em(i) {
                fim_de_linha(a, e, i, comprimento, &mut inicio, &mut achou_regras);
                let spans = a.chunks[c as usize].spans.clone();
                for s in spans {
                    if a.marcar_span(s) {
                        spans_quebrados.push(s);
                        custo += a.spans[s as usize].custo;
                    }
                }
                let ninho = a.chunks[c as usize].ninho;
                let total = a.total_ninho(ninho);
                if let Some(ant) = ninho_anterior
                    && total != 0
                    && total == a.total_ninho(ant)
                    && ninho != ant
                {
                    e.excesso += 10000;
                }
                ninho_anterior = Some(ninho);
                comprimento = e.quebras.coluna(i) as i64;
            } else if a.chunks[c as usize].espaco {
                comprimento += 1;
            }
            if a.chunks[c as usize].bloco.is_some() {
                if e.quebras.quebra_em(i) {
                    custo += formatar_bloco(a, cache, c, e.quebras.coluna(i)).custo;
                } else {
                    comprimento += a.comprimento_bloco(c) as i64;
                }
            }
            comprimento += a.chunks[c as usize].comp_texto as i64;
        }

        for (i, &r) in self.regras.iter().enumerate() {
            if let Some(v) = e.valores.0[i]
                && v != NAO_QUEBRA
            {
                custo += a.custo(r);
            }
        }

        for s in spans_quebrados {
            a.desmarcar_span(s);
        }

        fim_de_linha(a, e, self.chunks.len(), comprimento, &mut inicio, &mut achou_regras);
        e.quebras.custo = custo;
    }

    /// `expand()`.
    fn expandir(&self, a: &mut Arenas, cache: &mut CacheDeBlocos, estados: &mut Vec<Estado>, fila: &mut Fila, atual: usize) {
        let mut sem_quebra = estados[atual].valores.clone();
        let mut tentadas = 0;
        for &r in &self.regras {
            if estados[atual].vivas_set.contains(&r) {
                for valor in 1..a.num_valores(r) {
                    let mut ligadas = sem_quebra.clone();
                    let mut devem: Option<Vec<RegraId>> = None;
                    let valido = ligadas.tentar_ligar(a, r, valor, &mut |x| devem.get_or_insert_with(Vec::new).push(x));
                    if !valido {
                        continue;
                    }
                    let mut novo = self.novo_estado(a, cache, ligadas);
                    if let Some(devem) = devem {
                        novo.completo = false;
                        for x in devem {
                            if novo.vivas_set.insert(x) {
                                novo.vivas.push(x);
                            }
                        }
                    }
                    estados.push(novo);
                    let id = estados.len() - 1;
                    fila.adicionar(self, a, estados, id);
                }
                tentadas += 1;
                if tentadas == estados[atual].vivas.len() {
                    break;
                }
            }
            if !estados[atual].valores.contem(a, r) && !sem_quebra.tentar_ligar(a, r, 0, &mut |_| {}) {
                break;
            }
        }
    }

    fn valor(&self, a: &Arenas, e: &Estado, r: RegraId) -> i32 {
        e.valores.valor(a, r)
    }

    /// `compareOverlap(other)`: `<0` se `e` vence, `>0` se `outro` vence, `0`
    /// se não se sobrepõem.
    fn comparar_sobreposicao(&self, a: &mut Arenas, estados: &mut [Estado], e: usize, outro: usize) -> i32 {
        if !self.sobrepoe(a, estados, e, outro) {
            return 0;
        }
        for &r in &self.regras {
            let v = self.valor(a, &estados[e], r);
            let ov = self.valor(a, &estados[outro], r);
            if v != ov {
                return if v < ov { -1 } else { 1 };
            }
        }
        panic!("estados repetidos no divisor de linhas");
    }

    fn sobrepoe(&self, a: &mut Arenas, estados: &mut [Estado], e: usize, outro: usize) -> bool {
        self.preparar_ligadas_em_linhas(a, &mut estados[e]);
        self.preparar_ligadas_em_linhas(a, &mut estados[outro]);
        let le = estados[e].ligadas_em_linhas_soltas.as_ref().expect("preparado");
        let lo = estados[outro].ligadas_em_linhas_soltas.as_ref().expect("preparado");
        if le.len() != lo.len() {
            return false;
        }
        for &r in le {
            if !lo.contains(&r) {
                return false;
            }
            if self.valor(a, &estados[e], r) != self.valor(a, &estados[outro], r) {
                return false;
            }
        }

        self.preparar_restricoes(a, &mut estados[e]);
        self.preparar_restricoes(a, &mut estados[outro]);
        let re = estados[e].restricoes.as_ref().expect("preparado");
        let ro = estados[outro].restricoes.as_ref().expect("preparado");
        if re.len() != ro.len() {
            return false;
        }
        for (r, v) in re {
            if ro.iter().find(|(o, _)| o == r).map(|(_, x)| *x) != Some(*v) {
                return false;
            }
        }

        self.preparar_restricoes_soltas(a, &mut estados[e]);
        self.preparar_restricoes_soltas(a, &mut estados[outro]);
        let se = estados[e].restricoes_soltas.as_ref().expect("preparado");
        let so = estados[outro].restricoes_soltas.as_ref().expect("preparado");
        if se.len() != so.len() {
            return false;
        }
        for (r, proibidos) in se {
            let Some((_, outros)) = so.iter().find(|(o, _)| o == r) else {
                return false;
            };
            if proibidos.len() != outros.len() {
                return false;
            }
            for v in proibidos {
                if !outros.contains(v) {
                    return false;
                }
            }
        }
        true
    }

    fn preparar_ligadas_em_linhas(&self, a: &Arenas, e: &mut Estado) {
        if e.ligadas_em_linhas_soltas.is_some() {
            return;
        }
        let mut regras: Vec<RegraId> = Vec::new();
        let mut na_linha: Vec<RegraId> = Vec::new();
        let mut tem_solta = false;
        let juntar = |regras: &mut Vec<RegraId>, na_linha: &Vec<RegraId>| {
            for &r in na_linha {
                if !regras.contains(&r) {
                    regras.push(r);
                }
            }
        };
        for (i, &c) in self.chunks.iter().enumerate() {
            if e.quebras.quebra_em(i) {
                if tem_solta {
                    juntar(&mut regras, &na_linha);
                }
                na_linha.clear();
                tem_solta = false;
            }
            let r = a.chunks[c as usize].regra;
            if e.valores.contem(a, r) {
                if !na_linha.contains(&r) {
                    na_linha.push(r);
                }
            } else {
                tem_solta = true;
            }
        }
        if tem_solta {
            juntar(&mut regras, &na_linha);
        }
        e.ligadas_em_linhas_soltas = Some(regras);
    }

    fn preparar_restricoes(&self, a: &Arenas, e: &mut Estado) {
        if e.restricoes.is_some() {
            return;
        }
        let mut ligadas = HashSet::new();
        let mut soltas = HashSet::new();
        let mut ordem_ligadas = Vec::new();
        for &r in &self.regras {
            if e.valores.contem(a, r) {
                ligadas.insert(r);
                ordem_ligadas.push(r);
            } else {
                soltas.insert(r);
            }
        }
        let mut restricoes: Vec<(RegraId, i32)> = Vec::new();
        for &l in &ordem_ligadas {
            let mut i = 0;
            while let Some(s) = a.restrita(l, i) {
                i += 1;
                if !soltas.contains(&s) {
                    continue;
                }
                let v = e.valores.valor(a, l);
                if let Some(c) = a.restringe(l, v, s) {
                    if let Some(x) = restricoes.iter_mut().find(|(o, _)| *o == s) {
                        x.1 = c;
                    } else {
                        restricoes.push((s, c));
                    }
                }
            }
        }
        e.restricoes = Some(restricoes);
        e.ligadas = Some(ligadas);
        e.soltas = Some(soltas);
    }

    fn preparar_restricoes_soltas(&self, a: &Arenas, e: &mut Estado) {
        if e.restricoes_soltas.is_some() {
            return;
        }
        let ligadas = e.ligadas.as_ref().expect("restrições preparadas");
        let soltas = e.soltas.as_ref().expect("restrições preparadas");
        let mut resultado: Vec<(RegraId, Vec<i32>)> = Vec::new();
        // `_unboundRules` é um conjunto em ordem de inserção: a ordem das
        // regras do divisor.
        for &s in self.regras.iter().filter(|r| soltas.contains(r)) {
            let mut proibidos: Option<Vec<i32>> = None;
            let mut i = 0;
            while let Some(l) = a.restrita(s, i) {
                i += 1;
                if !ligadas.contains(&l) {
                    continue;
                }
                let valor_ligada = e.valores.valor(a, l);
                for v in 0..a.num_valores(s) {
                    let Some(c) = a.restringe(s, v, l) else {
                        continue;
                    };
                    if c == valor_ligada {
                        continue;
                    }
                    if c == DEVE_QUEBRAR && valor_ligada != NAO_QUEBRA {
                        continue;
                    }
                    let p = proibidos.get_or_insert_with(Vec::new);
                    if !p.contains(&v) {
                        p.push(v);
                    }
                }
            }
            if let Some(p) = proibidos {
                resultado.push((s, p));
            }
        }
        e.restricoes_soltas = Some(resultado);
    }
}

/// `_addLiveRules(rule)`.
fn adicionar_vivas(a: &mut Arenas, e: &mut Estado, r: RegraId) -> bool {
    let mut adicionou = false;
    for x in a.todas_restritas(r) {
        if e.valores.contem(a, x) {
            continue;
        }
        if e.vivas_set.insert(x) {
            e.vivas.push(x);
        }
        adicionou = true;
    }
    adicionou
}

/// `SolveStateQueue`: heap binário de índices em `estados`.
struct Fila {
    itens: Vec<usize>,
}

impl Fila {
    fn comparar_pontos(estados: &[Estado], x: usize, y: usize) -> i32 {
        let (cx, cy) = (estados[x].quebras.custo, estados[y].quebras.custo);
        if cx != cy {
            return if cx < cy { -1 } else { 1 };
        }
        let (ox, oy) = (estados[x].excesso, estados[y].excesso);
        ox.cmp(&oy) as i32
    }

    fn comparar_regras(d: &DivisorDeLinhas, a: &Arenas, estados: &[Estado], x: usize, y: usize) -> i32 {
        for &r in &d.regras {
            let vx = d.valor(a, &estados[x], r);
            let vy = d.valor(a, &estados[y], r);
            if vx != vy {
                return if vx < vy { -1 } else { 1 };
            }
        }
        panic!("estados repetidos na fila do divisor");
    }

    fn comparar(d: &DivisorDeLinhas, a: &Arenas, estados: &[Estado], x: usize, y: usize) -> i32 {
        let c = Self::comparar_pontos(estados, x, y);
        if c != 0 {
            return c;
        }
        Self::comparar_regras(d, a, estados, x, y)
    }

    fn adicionar(&mut self, d: &DivisorDeLinhas, a: &mut Arenas, estados: &mut [Estado], e: usize) {
        if self.tentar_sobrepor(d, a, estados, e) {
            return;
        }
        self.itens.push(e);
        let n = self.itens.len() - 1;
        self.subir(d, a, estados, e, n);
    }

    fn remover_primeiro(&mut self, d: &DivisorDeLinhas, a: &Arenas, estados: &mut [Estado]) -> usize {
        let resultado = self.itens[0];
        let ultimo = self.itens.pop().expect("fila não vazia");
        if !self.itens.is_empty() {
            self.descer(d, a, estados, ultimo, 0);
        }
        resultado
    }

    fn tentar_sobrepor(&mut self, d: &DivisorDeLinhas, a: &mut Arenas, estados: &mut [Estado], e: usize) -> bool {
        let n = self.itens.len();
        if n == 0 {
            return false;
        }
        let mut posicao = 1usize;
        loop {
            let indice = posicao - 1;
            let enfileirado = self.itens[indice];
            let mut comparacao = Self::comparar_pontos(estados, enfileirado, e);
            if comparacao == 0 {
                let sobre = d.comparar_sobreposicao(a, estados, enfileirado, e);
                if sobre < 0 {
                    return true;
                } else if sobre > 0 {
                    self.itens[indice] = e;
                    return true;
                } else {
                    comparacao = Self::comparar_regras(d, a, estados, enfileirado, e);
                }
            }
            if comparacao < 0 {
                let esquerda = posicao * 2;
                if esquerda <= n {
                    posicao = esquerda;
                    continue;
                }
            }
            loop {
                while posicao % 2 == 1 {
                    posicao >>= 1;
                }
                posicao += 1;
                if posicao <= n {
                    break;
                }
            }
            if posicao == 1 {
                break;
            }
        }
        false
    }

    fn subir(&mut self, d: &DivisorDeLinhas, a: &Arenas, estados: &[Estado], e: usize, mut indice: usize) {
        while indice > 0 {
            let pai_i = (indice - 1) / 2;
            let pai = self.itens[pai_i];
            if Self::comparar(d, a, estados, e, pai) > 0 {
                break;
            }
            self.itens[indice] = pai;
            indice = pai_i;
        }
        self.itens[indice] = e;
    }

    fn descer(&mut self, d: &DivisorDeLinhas, a: &Arenas, estados: &[Estado], e: usize, mut indice: usize) {
        let n = self.itens.len();
        let mut direita = indice * 2 + 2;
        while direita < n {
            let esquerda = direita - 1;
            let fe = self.itens[esquerda];
            let fd = self.itens[direita];
            let (menor_i, menor) = if Self::comparar(d, a, estados, fe, fd) < 0 {
                (esquerda, fe)
            } else {
                (direita, fd)
            };
            if Self::comparar(d, a, estados, e, menor) <= 0 {
                self.itens[indice] = e;
                return;
            }
            self.itens[indice] = menor;
            indice = menor_i;
            direita = indice * 2 + 2;
        }
        let esquerda = direita - 1;
        if esquerda < n {
            let filho = self.itens[esquerda];
            if Self::comparar(d, a, estados, e, filho) > 0 {
                self.itens[indice] = filho;
                indice = esquerda;
            }
        }
        self.itens[indice] = e;
    }
}
