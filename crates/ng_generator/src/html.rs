//! Parser of ngdart HTML templates.
//!
//! Equivalent to `package:ngast` (the parser `ngcompiler` uses), in the part
//! the generator needs: elements, text, interpolation, comments, the binding
//! forms (`[x]`, `(x)`, `[(x)]`, `#ref`, `*ngIf`) and `<ng-content>`.
//!
//! The whitespace reduction (`preserveWhitespace: false`, which is the default)
//! follows the rules of ngast's `MinimizeWhitespaceVisitor`, because they
//! decide which text nodes exist — and therefore the generated code. They were
//! read from the package itself, not guessed.

/// Elementos que o HTML fecha sozinho.
pub(crate) const VAZIOS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements that are normally `display: inline` — ngast's list, which decides
/// where whitespace is significant.
const EM_LINHA: &[&str] = &[
    "a", "abbr", "acronym", "b", "bdo", "big", "br", "button", "cite", "code", "dfn", "em", "i",
    "img", "input", "kbd", "label", "map", "object", "q", "samp", "script", "select", "small",
    "span", "strong", "sub", "sup", "textarea", "time", "tt", "var",
];

/// `&ngsp;` becomes this character in the ngast scanner and turns back into a
/// space at the end, escaping the reduction.
pub(crate) const NGSP: char = '\u{E500}';
const NBSP: char = '\u{00A0}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum No {
    Elemento(Elemento),
    Texto(String),
    /// `{{ expression }}`, with the byte range of the whole form (`{{` to `}}`)
    /// in the template file — it is what goes in the `/* REF:url:start:end */`
    /// comment the official compiler writes.
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

/// A binding written on the element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ligacao {
    /// `hidden` em `[hidden]`, `click` em `(click)`.
    pub nome: String,
    /// Dart expression between quotes, as written.
    pub valor: String,
    /// Byte range of the whole attribute in the template (from the name to the
    /// end of the value), which is what goes in the `/* REF:url:start:end */`
    /// comment.
    pub inicio: usize,
    pub fim: usize,
    /// Written without `=` (`<input required>`, and the `dir` that desugaring
    /// `*dir="key: x"` generates): `EmptyExpr`, which for a `bool` input is
    /// `true`. `x=""` is not this.
    pub sem_valor: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Elemento {
    pub nome: String,
    /// `class="x"` — a literal value, which may contain interpolation.
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
    /// `@i18n="description"`, `@i18n:title="…"`: the annotations (ngast's
    /// `AnnotationAst`), with the name without the `@`.
    pub anotacoes: Vec<Ligacao>,
    pub filhos: Vec<No>,
    /// The `[dirX]="e"` bindings of a hand-written `<template dir let-x
    /// [dirX]="e">`, after it is rewritten as `*dir` (see
    /// `template_como_container` in `view.rs`): the `REF` of each input is the
    /// range of the written binding, not that of the whole `estrela`. Empty for
    /// the rest.
    pub ligacoes_do_molde: Vec<Ligacao>,
    /// The already decomposed microsyntax of that rewritten `<template>`: the
    /// bindings and the `let-`s written separately, without going back to the
    /// text (which `isMicroExpression` would not always recognize, case j75).
    pub micro_do_molde: Option<crate::micro::Micro>,
    /// Position of the `<` that opens the element in the template (in bytes):
    /// it identifies the node, as the start of the `estrela` identifies the `*`
    /// (the anchor of a written `<template>` in view queries).
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

    /// Has a property binding: `[x]` or an attribute with `{{ }}` — what makes
    /// the element become a field of the view.
    pub fn liga_propriedade(&self) -> bool {
        !self.propriedades.is_empty() || self.atributos.iter().any(|a| a.valor.contains("{{"))
    }

    fn em_linha(&self) -> bool {
        EM_LINHA.contains(&self.nome.to_ascii_lowercase().as_str())
    }
}

/// Does the template project content (`<ng-content>`)? The component's user
/// needs to know: with projection the child view is created by
/// `createAndProject`, without it by `create`.
pub fn tem_projecao(nos: &[No]) -> bool {
    nos.iter().any(|n| match n {
        No::Conteudo { .. } => true,
        No::Elemento(e) => tem_projecao(&e.filhos),
        _ => false,
    })
}

/// The `<ng-content>`s of the template, in document order, with the `select` of
/// each one — the component's `ngContentSelectors`.
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

/// Parses a template. Shape errors do not stop it: the parser recovers and goes
/// on, like ngast's, so that a broken template does not bring down the whole
/// generation.
pub fn analisar(fonte: &str) -> Vec<No> {
    analisar_com_erro(fonte).0
}

/// Like [`analisar`], with the first shape error that ngast rejects: with it,
/// the official compiler fails to compile the template.
pub fn analisar_com_erro(fonte: &str) -> (Vec<No>, Option<String>) {
    analisar_no_modo(fonte, false)
}

/// `preservar`: the official `_PreserveWhitespaceVisitor`
/// (`ast_template_parser.dart:1610-1657`) in place of the
/// `MinimizeWhitespaceVisitor` — no text disappears or is trimmed; only
/// `&ngsp;` becomes a space, in all text.
pub fn analisar_no_modo(fonte: &str, preservar: bool) -> (Vec<No>, Option<String>) {
    let mut p = Parser {
        b: fonte.as_bytes(),
        i: 0,
        fonte,
        erro: None,
    };
    let crus = p.nos(None);
    let mut nos = if preservar {
        trocar_ngsp(crus)
    } else {
        minimizar_espacos(crus)
    };
    if !fonte.is_ascii() {
        em_utf16(&mut nos, fonte);
    }
    (nos, p.erro)
}

/// `&ngsp;` becomes a space in all text of the tree.
fn trocar_ngsp(nos: Vec<No>) -> Vec<No> {
    nos.into_iter()
        .map(|n| match n {
            No::Texto(t) => No::Texto(t.replace(NGSP, " ")),
            No::Elemento(mut e) => {
                e.filhos = trocar_ngsp(std::mem::take(&mut e.filhos));
                No::Elemento(e)
            }
            outro => outro,
        })
        .collect()
}

/// Adds `k` to the positions of all bindings and interpolations: the template
/// written in the annotation, whose `REF` positions are those of the `.dart`
/// (the official compiler adds the `templateOffset`).
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

/// The parser walks in bytes, but the official `REF:url:start:end` counts in
/// UTF-16 code units: `ngast` opens the template with `SourceFile.fromString`,
/// which uses `text.codeUnits`. A `©` before the binding is 2 bytes and 1 unit.
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
    /// A shape error ngast does not recover from (the official compiler fails
    /// and generates nothing).
    erro: Option<String>,
}

impl<'a> Parser<'a> {
    fn fim(&self) -> bool {
        self.i >= self.b.len()
    }

    fn olhar(&self, k: usize) -> u8 {
        *self.b.get(self.i + k).unwrap_or(&0)
    }

    /// Comparison in bytes: the cursor walks byte by byte and may stop in the
    /// middle of a multibyte character, where slicing the string would panic.
    fn comeca_com(&self, s: &str) -> bool {
        self.b[self.i.min(self.b.len())..].starts_with(s.as_bytes())
    }

    /// Reads nodes up to the end or up to the closing of `pai`.
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

    /// Text and interpolations up to the next `<`.
    fn ler_texto(&mut self, saida: &mut Vec<No>) {
        let inicio = self.i;
        // `_allTextMatches` (`simple_tokenizer.dart:30`): the text goes up to
        // the next `<`, but inside `{{ }}` the state is interpolation, where
        // `<` is just the operator (`a < b`). Outside it, a `<` that opens
        // neither a tag nor a comment is an `openTagStart` without a name: an
        // error in ngast, and the official compiler generates nothing; here it
        // is consumed (without hanging) and recorded.
        while !self.fim() {
            if self.comeca_com("{{") {
                match self.fonte[self.i + 2..].find("}}") {
                    Some(f) => self.i += 2 + f + 2,
                    None => self.i = self.b.len(),
                }
                continue;
            }
            if self.olhar(0) == b'<' {
                let abre =
                    self.olhar(1).is_ascii_alphabetic() || matches!(self.olhar(1), b'/' | b'!');
                if abre {
                    break;
                }
                self.erro
                    .get_or_insert_with(|| "`<` that does not open a tag in the text".into());
            }
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
                    // The value of a text interpolation goes through
                    // `_unEscapeText` like the text (R1.5: `{{ a &amp;&amp; b
                    // }}` is `a && b`).
                    saida.push(No::Interpolacao {
                        expr: decodificar(&resto[..f]).trim().to_string(),
                        inicio: ini,
                        fim: ini + 2 + f + 2,
                    });
                    resto = &resto[f + 2..];
                }
                None => {
                    // An unclosed `{{`: becomes text, like any other character.
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
            let Some((nome, valor, sem_valor)) = self.ler_atributo() else {
                break;
            };
            classificar(
                &mut el,
                &mut seletor_do_conteudo,
                nome,
                valor,
                (inicio, self.i),
                sem_valor,
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

    /// O nome, o valor e se foi escrito sem `=`.
    fn ler_atributo(&mut self) -> Option<(String, String, bool)> {
        let inicio = self.i;
        while !self.fim()
            && !self.olhar(0).is_ascii_whitespace()
            && !matches!(self.olhar(0), b'=' | b'>' | b'/')
        {
            self.i += 1;
        }
        if self.i == inicio {
            // A character that does not start an attribute name: advance so as
            // not to hang.
            self.i += 1;
            return None;
        }
        let nome = self.fonte[inicio..self.i].to_string();
        let fim_do_nome = self.i;
        self.pular_espacos();
        if self.fim() || self.olhar(0) != b'=' {
            // Without a value, the attribute range (that of the `REF`) is just
            // the name: the whitespace up to the next one is left out.
            self.i = fim_do_nome;
            return Some((nome, String::new(), true));
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
            Some((nome, valor, false))
        } else {
            let ini = self.i;
            while !self.fim()
                && !self.olhar(0).is_ascii_whitespace()
                && !matches!(self.olhar(0), b'>' | b'/')
            {
                self.i += 1;
            }
            Some((nome, self.fonte[ini..self.i].to_string(), false))
        }
    }
}

/// Puts the attribute read into the right list of the element.
fn classificar(
    el: &mut Elemento,
    conteudo: &mut Option<String>,
    nome: String,
    valor: String,
    (inicio, fim): (usize, usize),
    sem_valor: bool,
) {
    let l = Ligacao {
        nome: String::new(),
        valor: valor.clone(),
        inicio,
        fim,
        sem_valor,
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
        // The long forms that ngast's `RecursiveAstParser` recognizes: `on-x`
        // is `(x)` and `bind-x` is `[x]` (there is no `bindon-` or `ref-`).
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
            sem_valor,
        });
    }
}

/// The `_unEscapeText` of the ngast tokenizer (`simple_tokenizer.dart`),
/// applied only to text (attribute values stay raw): each match of
/// `&#([0-9]{2,4});|&#x([0-9A-Fa-f]{2,4});|&([a-zA-Z]+);`, from left to right,
/// becomes the character of the code (`String.fromCharCode`) or that of the
/// table [`crate::entities::ENTIDADES`] — and a name that is not in it, the
/// name itself. A code in the surrogate range (`&#xD800;`) would be a lone
/// surrogate in the Dart string, which the file written in UTF-8 carries as
/// U+FFFD.
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

/// The character reference that starts at `i` (at the `&`): where it ends
/// (after the `;`) and the text that replaces it.
fn entidade_em(t: &str, i: usize) -> Option<(usize, String)> {
    let b = t.as_bytes();
    let codigo =
        |n: u32| -> String { char::from_u32(n).map_or('\u{FFFD}'.to_string(), |c| c.to_string()) };
    // `&#` + 2 to 4 digits + `;`
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
        let valor = crate::entities::ENTIDADES
            .binary_search_by(|(k, _)| k.cmp(&nome))
            .map_or(nome, |k| crate::entities::ENTIDADES[k].1);
        return Some((inicio + n + 1, valor.to_string()));
    }
    None
}

/// ngast's `MinimizeWhitespaceVisitor`: drops a whitespace-only text node
/// between nodes that are not inline, and collapses the rest into a single
/// space.
/// ngast's `MinimizeWhitespaceVisitor` (`preserveWhitespace: false`), top-down
/// like it: each list of children is reduced looking at the **raw** neighbors
/// (their children not yet reduced) and only then is each child visited.
/// `<pre>` and whatever has `@preserveWhitespace` stay as they are, with
/// everything inside (`_bailOutToPreserveWhitespace`).
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
    // `anterior` is the *already processed* node — if the previous text
    // disappeared, the left neighbor is what remained before it. `seguinte` is
    // the original node at index i+1, not yet processed. That is how `ngast`
    // does it.
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
            // Whitespace-only text between two block nodes: dropped.
            anterior_e_colapsavel = true;
            continue;
        }
        match colapsar(texto, colapsa_esq, colapsa_dir) {
            Some(t) => {
                // A text node is a `StandaloneTemplateAst`: it holds the
                // neighbor's whitespace, like any other node that is not a
                // block element.
                anterior_e_colapsavel = false;
                saida.push(No::Texto(t));
            }
            // Text removed: for the next neighbor it is as if there were
            // nothing to the left (`prevNode = null` in ngast).
            None => anterior_e_colapsavel = true,
        }
    }
    saida
}

/// Collapses the whitespace of a text: every run of two or more becomes one
/// space (`RegExp(r'\s\s+')` — a single character, `\n` included, stays as is),
/// and the ends are trimmed according to the neighbors.
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
    // `&ngsp;` only becomes a space after trimming the ends (the `visitText` of
    // `MinimizeWhitespaceVisitor` runs over the already collapsed text): a `\n
    // &ngsp;` after a block element becomes `' '`. `&nbsp;` is protected before
    // trimming (`preserveNbsp`, ``): Rust's `trim` would take it along.
    let branco = |c: char| c.is_whitespace() && c != NBSP;
    let mut v = v;
    if apara_esq {
        v = v.trim_start_matches(branco).to_string();
    }
    if apara_dir {
        v = v.trim_end_matches(branco).to_string();
    }
    let v = v.replace(NGSP, " ");
    if v.is_empty() { None } else { Some(v) }
}

/// The rule of `_shouldCollapseAdjacentTo`: text, interpolation and comments do
/// not hold whitespace; an element holds it if it is inline; `<template>`,
/// `<ng-container>` and the element of a `*` (the `EmbeddedTemplateAst` that
/// wraps it) decide by the child at the end touching the text
/// (`_shouldCollapseWrapperNode`; `ultimo`: the neighbor is on the left).
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
        // Text and comments are template nodes like any other
        // (`StandaloneTemplateAst`): the whitespace next to them is
        // significant.
        Some(No::Texto(_)) => false,
        Some(No::Comentario(_)) => false,
    }
}

/// `_shouldCollapseWrapperNode`: without children, inline content may come
/// (does not collapse); a whitespace-only child at the end gives way to its
/// neighbor, except when it is the only one. The child is judged with
/// `lastNode` false, as in ngast.
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

    /// A stray `<` in the text does not hang the parser and is a shape error
    /// (ngast reads it as `openTagStart`); inside `{{ }}` it is the operator.
    #[test]
    fn menor_que_no_texto_e_na_interpolacao() {
        let (nos, erro) = analisar_com_erro("<div>a < b</div><i><3</i>");
        assert!(erro.is_some());
        assert_eq!(nos.len(), 2);
        let (nos, erro) = analisar_com_erro("<p>{{ x < 3 }}</p>");
        assert!(erro.is_none());
        let No::Elemento(p) = &nos[0] else { panic!() };
        assert!(matches!(&p.filhos[..], [No::Interpolacao { expr, .. }] if expr == "x < 3"));
    }

    use super::*;

    #[test]
    fn template_vazio_e_comentario() {
        assert!(analisar("").is_empty());
        assert_eq!(
            analisar("<!--{{message}}-->"),
            vec![No::Comentario("{{message}}".into())]
        );
    }

    /// `bind-x`/`on-x` are a binding and an event, `@x` is an annotation: none
    /// of the three is a DOM attribute (corpus cases i36 and i20).
    #[test]
    fn formas_longas_e_anotacoes() {
        let nos = analisar(r#"<p bind-title="t" on-click="f()" @i18n="d">x</p>"#);
        let [No::Elemento(e)] = nos.as_slice() else {
            panic!("not a single element: {nos:?}");
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

    /// The new_sali footer case: the `©` before the interpolation counts one
    /// unit, not two bytes.
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

    /// Real template of `NoDataComponent`: between `<img>` (an inline element)
    /// and the comment one space remains, and all the rest of the whitespace
    /// disappears. The official compiler emits exactly one `appendText(_el_0, '
    /// ')` there.
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
