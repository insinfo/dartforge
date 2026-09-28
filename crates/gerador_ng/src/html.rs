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
pub(crate) const VAZIOS: &[&str] = &[
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
pub(crate) const NGSP: char = '\u{E500}';
const NBSP: char = '\u{00A0}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum No {
    Elemento(Elemento),
    Texto(String),
    /// `{{ expressão }}`, com o intervalo em bytes da forma inteira
    /// (`{{` a `}}`) no arquivo do template — é o que vai no comentário
    /// `/* REF:url:inicio:fim */` que o oficial escreve.
    Interpolacao {
        expr: String,
        inicio: usize,
        fim: usize,
    },
    Comentario(String),
    /// `<ng-content select="...">`.
    Conteudo {
        seletor: Option<String>,
    },
}

/// Uma ligação escrita no elemento.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ligacao {
    /// `hidden` em `[hidden]`, `click` em `(click)`.
    pub nome: String,
    /// Expressão Dart entre aspas, como escrita.
    pub valor: String,
    /// Intervalo em bytes do atributo inteiro no template (do nome ao fim do
    /// valor), que é o que vai no comentário `/* REF:url:inicio:fim */`.
    pub inicio: usize,
    pub fim: usize,
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
    /// `@i18n="descrição"`, `@i18n:title="…"`: as anotações (`AnnotationAst`
    /// do ngast), com o nome sem o `@`.
    pub anotacoes: Vec<Ligacao>,
    pub filhos: Vec<No>,
    /// As ligações `[dirX]="e"` de um `<template dir let-x [dirX]="e">`
    /// escrito à mão, depois de ele ser reescrito como `*dir` (ver
    /// `template_como_container` em `visao.rs`): o `REF` de cada entrada é o
    /// intervalo da ligação escrita, não o da `estrela` inteira. Vazio no
    /// resto.
    pub ligacoes_do_molde: Vec<Ligacao>,
    /// A microssintaxe já decomposta desse `<template>` reescrito: as
    /// ligações e os `let-` escritos à parte, sem voltar ao texto (que o
    /// `isMicroExpression` nem sempre reconheceria, caso j75).
    pub micro_do_molde: Option<crate::micro::Micro>,
    /// Posição do `<` que abre o elemento no template (em bytes): identifica
    /// o nó, como o início da `estrela` identifica o `*` (a âncora de um
    /// `<template>` escrito nas consultas de visão).
    pub inicio: usize,
}

impl Elemento {
    /// A microssintaxe do `*` do elemento: a do `<template>` reescrito, ou a
    /// do texto da `estrela`.
    pub fn micro_da_estrela(&self) -> Option<crate::micro::Micro> {
        let estrela = self.estrela.as_ref()?;
        Some(match &self.micro_do_molde {
            Some(m) => m.clone(),
            None => crate::micro::analisar(&estrela.nome, &estrela.valor),
        })
    }

    /// Tem ligação de propriedade: `[x]` ou atributo com `{{ }}` — o que
    /// faz o elemento virar campo da visão.
    pub fn liga_propriedade(&self) -> bool {
        !self.propriedades.is_empty() || self.atributos.iter().any(|a| a.valor.contains("{{"))
    }

    fn em_linha(&self) -> bool {
        EM_LINHA.contains(&self.nome.to_ascii_lowercase().as_str())
    }
}

/// O template projeta conteúdo (`<ng-content>`)? Quem usa o componente
/// precisa saber: com projeção a visão-filha é criada por
/// `createAndProject`, sem ela por `create`.
pub fn tem_projecao(nos: &[No]) -> bool {
    nos.iter().any(|n| match n {
        No::Conteudo { .. } => true,
        No::Elemento(e) => tem_projecao(&e.filhos),
        _ => false,
    })
}

/// Os `<ng-content>` do template, em ordem de documento, com o `select` de
/// cada um — os `ngContentSelectors` do componente.
pub fn projecoes(nos: &[No]) -> Vec<Option<String>> {
    let mut saida = Vec::new();
    fn andar(nos: &[No], saida: &mut Vec<Option<String>>) {
        for n in nos {
            match n {
                No::Conteudo { seletor } => saida.push(seletor.clone()),
                No::Elemento(e) => andar(&e.filhos, saida),
                _ => {}
            }
        }
    }
    andar(nos, &mut saida);
    saida
}

/// Analisa um template. Erros de forma não interrompem: o parser recupera e
/// segue, como o do ngast, para que um template quebrado não derrube a
/// geração inteira.
pub fn analisar(fonte: &str) -> Vec<No> {
    let mut p = Parser {
        b: fonte.as_bytes(),
        i: 0,
        fonte,
    };
    let mut nos = minimizar_espacos(p.nos(None));
    if !fonte.is_ascii() {
        em_utf16(&mut nos, fonte);
    }
    nos
}

/// Soma `k` às posições de todas as ligações e interpolações: o template
/// escrito na anotação, cujas posições no `REF` são as do `.dart` (o
/// oficial soma o `templateOffset`).
pub fn deslocar(nos: &mut [No], k: usize) {
    for no in nos {
        match no {
            No::Interpolacao { inicio, fim, .. } => {
                *inicio += k;
                *fim += k;
            }
            No::Elemento(e) => {
                for l in e
                    .atributos
                    .iter_mut()
                    .chain(e.propriedades.iter_mut())
                    .chain(e.eventos.iter_mut())
                    .chain(e.bananas.iter_mut())
                    .chain(e.referencias.iter_mut())
                    .chain(e.estrela.iter_mut())
                {
                    l.inicio += k;
                    l.fim += k;
                }
                deslocar(&mut e.filhos, k);
            }
            _ => {}
        }
    }
}

/// O parser anda em bytes, mas o `REF:url:inicio:fim` do oficial conta em
/// unidades UTF-16: o `ngast` abre o template com `SourceFile.fromString`,
/// que usa `text.codeUnits`. Um `©` antes da ligação é 2 bytes e 1 unidade.
fn em_utf16(nos: &mut [No], fonte: &str) {
    let conv = |b: usize| fonte.get(..b).map_or(b, |s| s.encode_utf16().count());
    for no in nos {
        match no {
            No::Interpolacao { inicio, fim, .. } => {
                *inicio = conv(*inicio);
                *fim = conv(*fim);
            }
            No::Elemento(e) => {
                for l in e
                    .atributos
                    .iter_mut()
                    .chain(e.propriedades.iter_mut())
                    .chain(e.eventos.iter_mut())
                    .chain(e.bananas.iter_mut())
                    .chain(e.referencias.iter_mut())
                    .chain(e.estrela.iter_mut())
                {
                    l.inicio = conv(l.inicio);
                    l.fim = conv(l.fim);
                }
                em_utf16(&mut e.filhos, fonte);
            }
            _ => {}
        }
    }
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
            if self.olhar(0) == b'<'
                && (self.olhar(1).is_ascii_alphabetic() || self.olhar(1) == b'!')
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
        let nome = self.fonte[inicio..self.i.min(self.fonte.len())]
            .trim()
            .to_string();
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
                    let ini = resto.as_ptr() as usize - self.fonte.as_ptr() as usize - 2;
                    saida.push(No::Interpolacao {
                        expr: resto[..f].trim().to_string(),
                        inicio: ini,
                        fim: ini + 2 + f + 2,
                    });
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
        let abertura = self.i;
        self.i += 1;
        let inicio = self.i;
        while !self.fim()
            && !self.olhar(0).is_ascii_whitespace()
            && !matches!(self.olhar(0), b'>' | b'/')
        {
            self.i += 1;
        }
        let nome = self.fonte[inicio..self.i].to_string();
        let mut el = Elemento {
            nome,
            inicio: abertura,
            ..Default::default()
        };
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
            let inicio = self.i;
            let Some((nome, valor)) = self.ler_atributo() else {
                break;
            };
            classificar(
                &mut el,
                &mut seletor_do_conteudo,
                nome,
                valor,
                inicio,
                self.i,
            );
        }
        let vazio = VAZIOS.contains(&el.nome.to_ascii_lowercase().as_str());
        if !sozinho && !vazio {
            let nome = el.nome.clone();
            el.filhos = self.nos(Some(&nome));
        }
        if el.nome.eq_ignore_ascii_case("ng-content") {
            return Some(No::Conteudo {
                seletor: seletor_do_conteudo,
            });
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
        let fim_do_nome = self.i;
        self.pular_espacos();
        if self.fim() || self.olhar(0) != b'=' {
            // Sem valor, o intervalo do atributo (o do `REF`) é só o nome:
            // o espaço até o próximo fica fora.
            self.i = fim_do_nome;
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
fn classificar(
    el: &mut Elemento,
    conteudo: &mut Option<String>,
    nome: String,
    valor: String,
    inicio: usize,
    fim: usize,
) {
    let l = Ligacao {
        nome: String::new(),
        valor: valor.clone(),
        inicio,
        fim,
    };
    if let Some(interno) = nome.strip_prefix("[(").and_then(|n| n.strip_suffix(")]")) {
        el.bananas.push(Ligacao {
            nome: interno.to_string(),
            ..l
        });
    } else if let Some(interno) = nome.strip_prefix('[').and_then(|n| n.strip_suffix(']')) {
        el.propriedades.push(Ligacao {
            nome: interno.to_string(),
            ..l
        });
    } else if let Some(interno) = nome.strip_prefix('(').and_then(|n| n.strip_suffix(')')) {
        el.eventos.push(Ligacao {
            nome: interno.to_string(),
            ..l
        });
    } else if let Some(interno) = nome.strip_prefix('#') {
        el.referencias.push(Ligacao {
            nome: interno.to_string(),
            ..l
        });
    } else if let Some(interno) = nome.strip_prefix('*') {
        el.estrela = Some(Ligacao {
            nome: interno.to_string(),
            ..l
        });
    } else if let Some(interno) = nome.strip_prefix('@') {
        el.anotacoes.push(Ligacao {
            nome: interno.to_string(),
            ..l
        });
    } else if let Some(interno) = nome.strip_prefix("on-").filter(|n| !n.is_empty()) {
        // As formas longas que o `RecursiveAstParser` do ngast reconhece:
        // `on-x` é `(x)` e `bind-x` é `[x]` (não há `bindon-` nem `ref-`).
        el.eventos.push(Ligacao {
            nome: interno.to_string(),
            ..l
        });
    } else if let Some(interno) = nome.strip_prefix("bind-").filter(|n| !n.is_empty()) {
        el.propriedades.push(Ligacao {
            nome: interno.to_string(),
            ..l
        });
    } else {
        if el.nome.eq_ignore_ascii_case("ng-content") && nome == "select" {
            *conteudo = Some(valor.clone());
        }
        el.atributos.push(Ligacao {
            nome,
            valor,
            inicio,
            fim,
        });
    }
}

/// O `_unEscapeText` do tokenizador do ngast (`simple_tokenizer.dart`),
/// aplicado só ao texto (valor de atributo fica cru): cada casamento de
/// `&#([0-9]{2,4});|&#x([0-9A-Fa-f]{2,4});|&([a-zA-Z]+);`, da esquerda para
/// a direita, vira o caractere do código (`String.fromCharCode`) ou o da
/// tabela [`crate::entidades::ENTIDADES`] — e o nome que não está nela, o
/// próprio nome. Um código na faixa dos substitutos (`&#xD800;`) seria um
/// substituto solto na string do Dart, que o arquivo gravado em UTF-8 leva
/// como U+FFFD.
fn decodificar(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    let b = t.as_bytes();
    let mut saida = String::with_capacity(t.len());
    let mut i = 0;
    let mut copiado = 0;
    while i < b.len() {
        if b[i] != b'&' {
            i += 1;
            continue;
        }
        if let Some((fim, valor)) = entidade_em(t, i) {
            saida.push_str(&t[copiado..i]);
            saida.push_str(&valor);
            i = fim;
            copiado = fim;
        } else {
            i += 1;
        }
    }
    saida.push_str(&t[copiado..]);
    saida
}

/// A referência de caractere que começa em `i` (no `&`): onde ela termina
/// (depois do `;`) e o texto que a substitui.
fn entidade_em(t: &str, i: usize) -> Option<(usize, String)> {
    let b = t.as_bytes();
    let codigo =
        |n: u32| -> String { char::from_u32(n).map_or('\u{FFFD}'.to_string(), |c| c.to_string()) };
    // `&#` + 2 a 4 dígitos + `;`
    if b.get(i + 1) == Some(&b'#') {
        let inicio = i + 2;
        let n = b[inicio..]
            .iter()
            .take(4)
            .take_while(|c| c.is_ascii_digit())
            .count();
        if n >= 2 && b.get(inicio + n) == Some(&b';') {
            let v: u32 = t[inicio..inicio + n].parse().ok()?;
            return Some((inicio + n + 1, codigo(v)));
        }
        // `&#x` + 2 a 4 hexadecimais + `;`
        if b.get(i + 2) == Some(&b'x') {
            let inicio = i + 3;
            let n = b[inicio..]
                .iter()
                .take(4)
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            if n >= 2 && b.get(inicio + n) == Some(&b';') {
                let v = u32::from_str_radix(&t[inicio..inicio + n], 16).ok()?;
                return Some((inicio + n + 1, codigo(v)));
            }
        }
        return None;
    }
    // `&` + letras + `;`
    let inicio = i + 1;
    let n = b[inicio..]
        .iter()
        .take_while(|c| c.is_ascii_alphabetic())
        .count();
    if n >= 1 && b.get(inicio + n) == Some(&b';') {
        let nome = &t[inicio..inicio + n];
        let valor = crate::entidades::ENTIDADES
            .binary_search_by(|(k, _)| k.cmp(&nome))
            .map_or(nome, |k| crate::entidades::ENTIDADES[k].1);
        return Some((inicio + n + 1, valor.to_string()));
    }
    None
}

/// `MinimizeWhitespaceVisitor` do ngast: some com o nó de texto só de espaços
/// entre nós que não são de linha, e colapsa o resto num espaço só.
/// O `MinimizeWhitespaceVisitor` do ngast (`preserveWhitespace: false`), de
/// cima para baixo como ele: cada lista de filhos é reduzida olhando os
/// vizinhos **crus** (os filhos deles ainda sem redução) e só depois cada
/// filho é visitado. `<pre>` e o que tem `@preserveWhitespace` ficam como
/// estão, com tudo o que há dentro (`_bailOutToPreserveWhitespace`).
fn minimizar_espacos(nos: Vec<No>) -> Vec<No> {
    reduzir_espacos(nos)
        .into_iter()
        .map(|n| match n {
            No::Elemento(mut e) => {
                let preserva =
                    e.nome == "pre" || e.anotacoes.iter().any(|a| a.nome == "preserveWhitespace");
                if !preserva && !e.filhos.is_empty() {
                    e.filhos = minimizar_espacos(std::mem::take(&mut e.filhos));
                }
                No::Elemento(e)
            }
            outro => outro,
        })
        .collect()
}

fn reduzir_espacos(nos: Vec<No>) -> Vec<No> {
    let mut saida: Vec<No> = Vec::with_capacity(nos.len());
    // `anterior` é o nó *já processado* — se o texto anterior sumiu, o vizinho
    // à esquerda é o que sobrou antes dele. `seguinte` é o nó original de
    // índice i+1, ainda não processado. É assim no `ngast`.
    let mut anterior_e_colapsavel = true;
    for (i, no) in nos.iter().enumerate() {
        let No::Texto(texto) = no else {
            anterior_e_colapsavel = colapsa_ao_lado(Some(no), true);
            saida.push(no.clone());
            continue;
        };
        let colapsa_esq = anterior_e_colapsavel;
        let colapsa_dir = colapsa_ao_lado(nos.get(i + 1), false);
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

/// Colapsa espaços de um texto: todo bloco de dois ou mais vira um espaço
/// (`RegExp(r'\s\s+')` — um caractere sozinho, `\n` inclusive, fica como
/// está), e as pontas são aparadas conforme os vizinhos.
fn colapsar(texto: &str, apara_esq: bool, apara_dir: bool) -> Option<String> {
    let mut v = String::with_capacity(texto.len());
    let mut bloco = String::new();
    let fechar = |bloco: &mut String, v: &mut String| {
        if bloco.chars().count() == 1 {
            v.push_str(bloco);
        } else if !bloco.is_empty() {
            v.push(' ');
        }
        bloco.clear();
    };
    for c in texto.chars() {
        if c.is_whitespace() && c != NBSP {
            bloco.push(c);
            continue;
        }
        fechar(&mut bloco, &mut v);
        v.push(c);
    }
    fechar(&mut bloco, &mut v);
    // O `&ngsp;` só vira espaço depois de aparar as pontas (o `visitText`
    // do `MinimizeWhitespaceVisitor` roda sobre o texto já colapsado): um
    // `\n  &ngsp;` depois de elemento de bloco fica `' '`.
    let mut v = v;
    if apara_esq {
        v = v.trim_start().to_string();
    }
    if apara_dir {
        v = v.trim_end().to_string();
    }
    let v = v.replace(NGSP, " ");
    if v.is_empty() { None } else { Some(v) }
}

/// Regra do `_shouldCollapseAdjacentTo`: texto, interpolação e comentário não
/// seguram espaço; elemento segura se for de linha; `<template>`,
/// `<ng-container>` e o elemento de um `*` (o `EmbeddedTemplateAst` que o
/// envolve) decidem pelo filho da ponta que encosta no texto
/// (`_shouldCollapseWrapperNode`; `ultimo`: o vizinho está à esquerda).
fn colapsa_ao_lado(no: Option<&No>, ultimo: bool) -> bool {
    match no {
        None => true,
        Some(No::Elemento(e)) if e.estrela.is_some() => {
            let mut dentro = e.clone();
            dentro.estrela = None;
            colapsa_envolvido(&[No::Elemento(dentro)], ultimo)
        }
        Some(No::Elemento(e)) if e.nome == "template" || e.nome == "ng-container" => {
            colapsa_envolvido(&e.filhos, ultimo)
        }
        Some(No::Elemento(e)) => !e.em_linha(),
        Some(No::Interpolacao { .. }) => false,
        Some(No::Conteudo { .. }) => false,
        // Texto e comentário são nós do template como qualquer outro
        // (`StandaloneTemplateAst`): o espaço ao lado deles é significativo.
        Some(No::Texto(_)) => false,
        Some(No::Comentario(_)) => false,
    }
}

/// `_shouldCollapseWrapperNode`: sem filhos, pode vir conteúdo em linha (não
/// colapsa); o filho da ponta só de espaços cede a vez ao vizinho dele,
/// exceto quando é o único. O filho é julgado com `lastNode` falso, como no
/// ngast.
fn colapsa_envolvido(filhos: &[No], ultimo: bool) -> bool {
    let (Some(primeiro), Some(fim)) = (filhos.first(), filhos.last()) else {
        return false;
    };
    let mut ponta = if ultimo { fim } else { primeiro };
    if let No::Texto(t) = ponta
        && t.trim().is_empty()
    {
        if filhos.len() == 1 {
            return false;
        }
        ponta = if ultimo {
            &filhos[filhos.len() - 2]
        } else {
            &filhos[1]
        };
    }
    colapsa_ao_lado(Some(ponta), false)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn template_vazio_e_comentario() {
        assert!(analisar("").is_empty());
        assert_eq!(
            analisar("<!--{{message}}-->"),
            vec![No::Comentario("{{message}}".into())]
        );
    }

    /// `bind-x`/`on-x` são ligação e evento, `@x` é anotação: nenhum dos
    /// três é atributo do DOM (caso i36 e i20 do corpus).
    #[test]
    fn formas_longas_e_anotacoes() {
        let nos = analisar(r#"<p bind-title="t" on-click="f()" @i18n="d">x</p>"#);
        let [No::Elemento(e)] = nos.as_slice() else {
            panic!("não é um elemento só: {nos:?}");
        };
        assert!(e.atributos.is_empty());
        assert_eq!(e.propriedades[0].nome, "title");
        assert_eq!((e.propriedades[0].inicio, e.propriedades[0].fim), (3, 17));
        assert_eq!(e.eventos[0].nome, "click");
        assert_eq!(e.anotacoes[0].nome, "i18n");
        assert_eq!(e.anotacoes[0].valor, "d");
    }

    #[test]
    fn ng_content() {
        assert_eq!(
            analisar("<ng-content></ng-content>"),
            vec![No::Conteudo { seletor: None }]
        );
        assert_eq!(
            analisar("<ng-content select='.x'></ng-content>"),
            vec![No::Conteudo {
                seletor: Some(".x".into())
            }]
        );
    }

    #[test]
    fn elemento_com_filhos_e_atributos() {
        let n = analisar("<div class=\"a\"><span>oi</span></div>");
        let No::Elemento(div) = &n[0] else {
            panic!("{n:?}")
        };
        assert_eq!(div.nome, "div");
        assert_eq!(div.atributos[0].nome, "class");
        assert_eq!(div.atributos[0].valor, "a");
        let No::Elemento(span) = &div.filhos[0] else {
            panic!("{:?}", div.filhos)
        };
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
            vec![
                No::Texto("a".into()),
                No::Interpolacao {
                    expr: "b".into(),
                    inicio: 1,
                    fim: 8
                },
                No::Texto("c".into())
            ]
        );
    }

    /// Caso do footer do new_sali: o `©` antes da interpolação conta uma
    /// unidade, não dois bytes.
    #[test]
    fn posicao_em_unidades_utf16() {
        assert_eq!(
            analisar("<span>© {{ano}}</span>"),
            vec![No::Elemento(Elemento {
                nome: "span".into(),
                filhos: vec![
                    No::Texto("© ".into()),
                    No::Interpolacao {
                        expr: "ano".into(),
                        inicio: 8,
                        fim: 15
                    },
                ],
                ..Default::default()
            })]
        );
    }

    #[test]
    fn elemento_vazio_nao_engole_o_resto() {
        let n = analisar("<br><span>x</span>");
        assert_eq!(n.len(), 2);
        let No::Elemento(br) = &n[0] else {
            panic!("{n:?}")
        };
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
        let No::Elemento(div) = &n[0] else {
            panic!("{n:?}")
        };
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
