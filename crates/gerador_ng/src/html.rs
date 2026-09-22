//! Parser dos templates HTML do ngdart.
//!
//! Equivale ao `package:ngast` (o parser que o `ngcompiler` usa), na parte que
//! o gerador precisa: elementos, texto, interpolação, comentários, as formas
//! de ligação (`[x]`, `(x)`, `[(x)]`, `#ref`, `*ngIf`) e `<ng-content>`.
//!
//! A redução de espaço em branco (`preserveWhitespace: false`, que é o padrão)
//! segue as regras do `MinimizeWhitespaceVisitor` do `ngast`, porque elas
//! decidem quais nós de texto existem — e, portanto, o código gerado. Foram
//! lidas do próprio pacote, não adivinhadas.

/// Elementos que o HTML fecha sozinho.
const VAZIOS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elementos normalmente `display: inline` — a lista do `ngast`, que decide
/// onde o espaço em branco é significativo.
const EM_LINHA: &[&str] = &[
    "a", "abbr", "acronym", "b", "bdo", "big", "br", "button", "cite", "code", "dfn", "em", "i",
    "img", "input", "kbd", "label", "map", "object", "q", "samp", "script", "select", "small",
    "span", "strong", "sub", "sup", "textarea", "time", "tt", "var",
];

/// `&ngsp;` vira este caractere no scanner do ngast e volta a ser espaço no
/// fim, escapando da redução.
const NGSP: char = '\u{E500}';
const NBSP: char = '\u{00A0}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum No {
    Elemento(Elemento),
    Texto(String),
    /// `{{ expressão }}`.
    Interpolacao(String),
    Comentario(String),
    /// `<ng-content select="...">`.
    Conteudo { seletor: Option<String> },
}

/// Uma ligação escrita no elemento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ligacao {
    /// `hidden` em `[hidden]`, `click` em `(click)`.
    pub nome: String,
    /// Expressão Dart entre aspas, como escrita.
    pub valor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Elemento {
    pub nome: String,
    /// `class="x"` — valor literal, que pode conter interpolação.
    pub atributos: Vec<Ligacao>,
    /// `[x]="e"`.
    pub propriedades: Vec<Ligacao>,
    /// `(x)="e"`.
    pub eventos: Vec<Ligacao>,
    /// `[(x)]="e"`.
    pub bananas: Vec<Ligacao>,
    /// `#ref` ou `#ref="ngForm"`.
    pub referencias: Vec<Ligacao>,
    /// `*ngIf="e"` — a forma abreviada do `<template>`.
    pub estrela: Option<Ligacao>,
    pub filhos: Vec<No>,
}

impl Elemento {
    fn em_linha(&self) -> bool {
        EM_LINHA.contains(&self.nome.to_ascii_lowercase().as_str())
    }
}

/// Analisa um template. Erros de forma não interrompem: o parser recupera e
/// segue, como o do ngast, para que um template quebrado não derrube a
/// geração inteira.
pub fn analisar(fonte: &str) -> Vec<No> {
    let mut p = Parser { b: fonte.as_bytes(), i: 0, fonte };
    let nos = p.nos(None);
    reduzir_espacos(nos)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    fonte: &'a str,
}

impl<'a> Parser<'a> {
    fn fim(&self) -> bool {
        self.i >= self.b.len()
    }

    fn olhar(&self, k: usize) -> u8 {
        *self.b.get(self.i + k).unwrap_or(&0)
    }

    /// Comparação em bytes: o cursor anda byte a byte e pode parar no meio de
    /// um caractere multibyte, onde fatiar a string entraria em pânico.
    fn comeca_com(&self, s: &str) -> bool {
        self.b[self.i.min(self.b.len())..].starts_with(s.as_bytes())
    }

    /// Lê nós até o fim ou até o fechamento de `pai`.
    fn nos(&mut self, pai: Option<&str>) -> Vec<No> {
        let mut saida = Vec::new();
        while !self.fim() {
            if self.comeca_com("</") {
                let fecha = self.ler_fechamento();
                match pai {
                    // Fechamento do nosso elemento: devolve ao chamador.
                    Some(p) if fecha.eq_ignore_ascii_case(p) => return saida,
                    // Fechamento de outro: o HTML permite, ignoramos.
                    _ => continue,
                }
            }
            if self.comeca_com("<!--") {
                saida.push(self.ler_comentario());
                continue;
            }
            if self.olhar(0) == b'<' && (self.olhar(1).is_ascii_alphabetic() || self.olhar(1) == b'!')
            {
                if self.olhar(1) == b'!' {
                    // `<!DOCTYPE …>` e afins: fora do template do Angular.
                    self.ate(b'>');
                    continue;
                }
                if let Some(no) = self.ler_elemento() {
                    saida.push(no);
                }
                continue;
            }
            self.ler_texto(&mut saida);
        }
        saida
    }

    fn ate(&mut self, fim: u8) {
        while !self.fim() && self.olhar(0) != fim {
            self.i += 1;
        }
        if !self.fim() {
            self.i += 1;
        }
    }

    fn ler_comentario(&mut self) -> No {
        self.i += 4;
        let inicio = self.i;
        while !self.fim() && !self.comeca_com("-->") {
            self.i += 1;
        }
        let texto = self.fonte[inicio..self.i.min(self.fonte.len())].to_string();
        if !self.fim() {
            self.i += 3;
        }
        No::Comentario(texto)
    }

    fn ler_fechamento(&mut self) -> String {
        self.i += 2;
        let inicio = self.i;
        while !self.fim() && self.olhar(0) != b'>' {
            self.i += 1;
        }
        let nome = self.fonte[inicio..self.i.min(self.fonte.len())].trim().to_string();
        if !self.fim() {
            self.i += 1;
        }
        nome
    }

    /// Texto e interpolações até o próximo `<`.
    fn ler_texto(&mut self, saida: &mut Vec<No>) {
        let inicio = self.i;
        while !self.fim() && self.olhar(0) != b'<' {
            self.i += 1;
        }
        let bruto = &self.fonte[inicio..self.i];
        let mut resto = bruto;
        while let Some(pos) = resto.find("{{") {
            if pos > 0 {
                saida.push(No::Texto(decodificar(&resto[..pos])));
            }
            resto = &resto[pos + 2..];
            match resto.find("}}") {
                Some(f) => {
                    saida.push(No::Interpolacao(resto[..f].trim().to_string()));
                    resto = &resto[f + 2..];
                }
                None => {
                    // `{{` sem fechar: vira texto, como qualquer outro caractere.
                    saida.push(No::Texto(decodificar(resto)));
                    return;
                }
            }
        }
        if !resto.is_empty() {
            saida.push(No::Texto(decodificar(resto)));
        }
    }

    fn ler_elemento(&mut self) -> Option<No> {
        self.i += 1;
        let inicio = self.i;
        while !self.fim() && !self.olhar(0).is_ascii_whitespace() && !matches!(self.olhar(0), b'>' | b'/')
        {
            self.i += 1;
        }
        let nome = self.fonte[inicio..self.i].to_string();
        let mut el = Elemento { nome, ..Default::default() };
        let mut seletor_do_conteudo = None;
        let mut sozinho = false;
        loop {
            self.pular_espacos();
            if self.fim() {
                break;
            }
            if self.olhar(0) == b'>' {
                self.i += 1;
                break;
            }
            if self.olhar(0) == b'/' && self.olhar(1) == b'>' {
                self.i += 2;
                sozinho = true;
                break;
            }
            let Some((nome, valor)) = self.ler_atributo() else { break };
            classificar(&mut el, &mut seletor_do_conteudo, nome, valor);
        }
        let vazio = VAZIOS.contains(&el.nome.to_ascii_lowercase().as_str());
        if !sozinho && !vazio {
            let nome = el.nome.clone();
            el.filhos = reduzir_espacos(self.nos(Some(&nome)));
        }
        if el.nome.eq_ignore_ascii_case("ng-content") {
            return Some(No::Conteudo { seletor: seletor_do_conteudo });
        }
        Some(No::Elemento(el))
    }

    fn pular_espacos(&mut self) {
        while !self.fim() && self.olhar(0).is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn ler_atributo(&mut self) -> Option<(String, String)> {
        let inicio = self.i;
        while !self.fim()
            && !self.olhar(0).is_ascii_whitespace()
            && !matches!(self.olhar(0), b'=' | b'>' | b'/')
        {
            self.i += 1;
        }
        if self.i == inicio {
            // Caractere que não abre nome de atributo: anda para não travar.
            self.i += 1;
            return None;
        }
        let nome = self.fonte[inicio..self.i].to_string();
        self.pular_espacos();
        if self.fim() || self.olhar(0) != b'=' {
            return Some((nome, String::new()));
        }
        self.i += 1;
        self.pular_espacos();
        let aspas = self.olhar(0);
        if aspas == b'"' || aspas == b'\'' {
            self.i += 1;
            let ini = self.i;
            while !self.fim() && self.olhar(0) != aspas {
                self.i += 1;
            }
            let valor = self.fonte[ini..self.i].to_string();
            if !self.fim() {
                self.i += 1;
            }
            Some((nome, valor))
        } else {
            let ini = self.i;
            while !self.fim()
                && !self.olhar(0).is_ascii_whitespace()
                && !matches!(self.olhar(0), b'>' | b'/')
            {
                self.i += 1;
            }
            Some((nome, self.fonte[ini..self.i].to_string()))
        }
    }
}

/// Põe o atributo lido na lista certa do elemento.
fn classificar(el: &mut Elemento, conteudo: &mut Option<String>, nome: String, valor: String) {
    let l = Ligacao { nome: String::new(), valor: valor.clone() };
    if let Some(interno) = nome.strip_prefix("[(").and_then(|n| n.strip_suffix(")]")) {
        el.bananas.push(Ligacao { nome: interno.to_string(), ..l });
    } else if let Some(interno) = nome.strip_prefix('[').and_then(|n| n.strip_suffix(']')) {
        el.propriedades.push(Ligacao { nome: interno.to_string(), ..l });
    } else if let Some(interno) = nome.strip_prefix('(').and_then(|n| n.strip_suffix(')')) {
        el.eventos.push(Ligacao { nome: interno.to_string(), ..l });
    } else if let Some(interno) = nome.strip_prefix('#') {
        el.referencias.push(Ligacao { nome: interno.to_string(), ..l });
    } else if let Some(interno) = nome.strip_prefix('*') {
        el.estrela = Some(Ligacao { nome: interno.to_string(), ..l });
    } else if nome.eq_ignore_ascii_case("bind-") {
        // forma longa não abreviada; sem uso nos projetos do proprietário
    } else {
        if el.nome.eq_ignore_ascii_case("ng-content") && nome == "select" {
            *conteudo = Some(valor.clone());
        }
        el.atributos.push(Ligacao { nome, valor });
    }
}

/// Entidades que aparecem em template de aplicação. O conjunto completo do
/// HTML não cabe aqui e nem o ngast o traz inteiro.
fn decodificar(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    t.replace("&ngsp;", &NGSP.to_string())
        .replace("&nbsp;", &NBSP.to_string())
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

/// `MinimizeWhitespaceVisitor` do ngast: some com o nó de texto só de espaços
/// entre nós que não são de linha, e colapsa o resto num espaço só.
fn reduzir_espacos(nos: Vec<No>) -> Vec<No> {
    let mut saida: Vec<No> = Vec::with_capacity(nos.len());
    // `anterior` é o nó *já processado* — se o texto anterior sumiu, o vizinho
    // à esquerda é o que sobrou antes dele. `seguinte` é o nó original de
    // índice i+1, ainda não processado. É assim no `ngast`.
    let mut anterior_e_colapsavel = true;
    for (i, no) in nos.iter().enumerate() {
        let No::Texto(texto) = no else {
            anterior_e_colapsavel = colapsa_ao_lado(Some(no));
            saida.push(no.clone());
            continue;
        };
        let colapsa_esq = anterior_e_colapsavel;
        let colapsa_dir = colapsa_ao_lado(nos.get(i + 1));
        if colapsa_esq && colapsa_dir && texto.trim().is_empty() && !texto.contains(NBSP) {
            // Texto só de espaços entre dois nós de bloco: some.
            anterior_e_colapsavel = true;
            continue;
        }
        match colapsar(texto, colapsa_esq, colapsa_dir) {
            Some(t) => {
                // Nó de texto é `StandaloneTemplateAst`: segura o espaço do
                // vizinho, como qualquer outro nó que não seja elemento de
                // bloco.
                anterior_e_colapsavel = false;
                saida.push(No::Texto(t));
            }
            // Texto removido: para o vizinho seguinte é como se não houvesse
            // nada à esquerda (`prevNode = null` no ngast).
            None => anterior_e_colapsavel = true,
        }
    }
    saida
}

/// Colapsa espaços de um texto: todo bloco de dois ou mais vira um espaço, e
/// as pontas são aparadas conforme os vizinhos.
fn colapsar(texto: &str, apara_esq: bool, apara_dir: bool) -> Option<String> {
    let mut v = String::with_capacity(texto.len());
    let mut espacos = 0usize;
    for c in texto.chars() {
        if c.is_whitespace() && c != NBSP {
            espacos += 1;
            continue;
        }
        if espacos > 0 {
            v.push(' ');
            espacos = 0;
        }
        v.push(c);
    }
    if espacos > 0 {
        v.push(' ');
    }
    let mut v = v.replace(NGSP, " ");
    if apara_esq {
        v = v.trim_start().to_string();
    }
    if apara_dir {
        v = v.trim_end().to_string();
    }
    if v.is_empty() { None } else { Some(v) }
}

/// Regra do `_shouldCollapseAdjacentTo`: texto, interpolação e comentário não
/// seguram espaço; elemento segura se for de linha.
fn colapsa_ao_lado(no: Option<&No>) -> bool {
    match no {
        None => true,
        Some(No::Elemento(e)) => !e.em_linha(),
        Some(No::Interpolacao(_)) => false,
        Some(No::Conteudo { .. }) => false,
        // Texto e comentário são nós do template como qualquer outro
        // (`StandaloneTemplateAst`): o espaço ao lado deles é significativo.
        Some(No::Texto(_)) => false,
        Some(No::Comentario(_)) => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn template_vazio_e_comentario() {
        assert!(analisar("").is_empty());
        assert_eq!(analisar("<!--{{message}}-->"), vec![No::Comentario("{{message}}".into())]);
    }

    #[test]
    fn ng_content() {
        assert_eq!(analisar("<ng-content></ng-content>"), vec![No::Conteudo { seletor: None }]);
        assert_eq!(
            analisar("<ng-content select='.x'></ng-content>"),
            vec![No::Conteudo { seletor: Some(".x".into()) }]
        );
    }

    #[test]
    fn elemento_com_filhos_e_atributos() {
        let n = analisar("<div class=\"a\"><span>oi</span></div>");
        let No::Elemento(div) = &n[0] else { panic!("{n:?}") };
        assert_eq!(div.nome, "div");
        assert_eq!(div.atributos[0].nome, "class");
        assert_eq!(div.atributos[0].valor, "a");
        let No::Elemento(span) = &div.filhos[0] else { panic!("{:?}", div.filhos) };
        assert_eq!(span.filhos, vec![No::Texto("oi".into())]);
    }

    #[test]
    fn formas_de_ligacao() {
        let n = analisar("<x [p]=\"a\" (e)=\"b()\" [(v)]=\"c\" #r *ngIf=\"d\"></x>");
        let No::Elemento(x) = &n[0] else { panic!() };
        assert_eq!(x.propriedades[0].nome, "p");
        assert_eq!(x.eventos[0].nome, "e");
        assert_eq!(x.bananas[0].nome, "v");
        assert_eq!(x.referencias[0].nome, "r");
        assert_eq!(x.estrela.as_ref().unwrap().nome, "ngIf");
        assert_eq!(x.estrela.as_ref().unwrap().valor, "d");
    }

    #[test]
    fn interpolacao_separa_o_texto() {
        assert_eq!(
            analisar("a{{ b }}c"),
            vec![No::Texto("a".into()), No::Interpolacao("b".into()), No::Texto("c".into())]
        );
    }

    #[test]
    fn elemento_vazio_nao_engole_o_resto() {
        let n = analisar("<br><span>x</span>");
        assert_eq!(n.len(), 2);
        let No::Elemento(br) = &n[0] else { panic!("{n:?}") };
        assert!(br.filhos.is_empty());
    }

    /// Template real do `NoDataComponent`: entre `<img>` (elemento de linha) e
    /// o comentário sobra um espaço, e todo o resto do espaço em branco some.
    /// O compilador oficial emite ali exatamente um `appendText(_el_0, ' ')`.
    #[test]
    fn espaco_do_no_data_igual_ao_oficial() {
        let n = analisar(
            "<div class=\"c\">\n      <img src=\"a.svg\">  \n     <!-- x -->\n    <div>{{m}}</div>\n</div>",
        );
        let No::Elemento(div) = &n[0] else { panic!("{n:?}") };
        let textos: Vec<&String> = div
            .filhos
            .iter()
            .filter_map(|f| match f {
                No::Texto(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(textos, vec![&" ".to_string()], "{:?}", div.filhos);
    }

    /// Regra do ngast: `<div>\n  <span>x</span>\n</div>` colapsa para
    /// `<div><span>x</span></div>`.
    #[test]
    fn espaco_entre_blocos_some() {
        let n = analisar("<div>\n  <p>x</p>\n</div>");
        let No::Elemento(div) = &n[0] else { panic!() };
        assert_eq!(div.filhos.len(), 1, "{:?}", div.filhos);
    }
}
