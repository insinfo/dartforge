//! Directive CSS selectors and matching them against template elements.
//!
//! Port of `selector.dart` and `attribute_matcher.dart` from `ngcompiler`. The
//! official compiler decides which directives an element gets by matching the
//! selector of each one (`@Directive(selector: 'form:not([ngNoForm])')`)
//! against a description of the element (`createElementCssSelector`, in
//! `template_parser.dart`): the tag name, each attribute with its value, each
//! property binding and each event as an attribute, and the classes of
//! `class="..."`.
//!
//! The generator needs the same answer for two reasons: to know that an element
//! belongs to a directive it does not emit yet (and refuse — emitting the plain
//! element compiles and does something else) and, further on, to instantiate
//! the directives it knows how to emit, in the right order.

/// A simple selector: `tag.class[attr=value]:not(...)`. A comma-separated list
/// becomes several of these.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seletor {
    pub elemento: Option<String>,
    pub classes: Vec<String>,
    pub atributos: Vec<Atributo>,
    pub nao: Vec<Seletor>,
}

/// `[nome]`, `[nome=valor]`, `[nome~=valor]`…
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atributo {
    pub nome: String,
    pub casamento: Casamento,
}

/// As formas de `attribute_matcher.dart`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Casamento {
    /// `[nome]`: basta existir.
    Existe,
    /// `[name=value]`, with the value already lowercase.
    Igual(Option<String>),
    /// `[name~=value]`: one of the space-separated items.
    Lista(String),
    /// `[nome|=valor]`: o valor ou `valor-…`.
    Hifen(String),
    /// `[nome^=valor]`.
    Prefixo(String),
    /// `[nome$=valor]`.
    Sufixo(String),
    /// `[nome*=valor]`.
    Contem(String),
}

impl Casamento {
    fn aceita(&self, valor: Option<&str>) -> bool {
        match self {
            Casamento::Existe => true,
            Casamento::Igual(v) => v.as_deref() == valor,
            // The others get the value with `!` in the official code: an
            // attribute without a value does not get here in practice; without
            // a value, it does not match.
            Casamento::Lista(v) => valor.is_some_and(|x| x.split_whitespace().any(|i| i == v)),
            Casamento::Hifen(v) => valor.is_some_and(|x| x == v || x.starts_with(&format!("{v}-"))),
            Casamento::Prefixo(v) => valor.is_some_and(|x| x.starts_with(v.as_str())),
            Casamento::Sufixo(v) => valor.is_some_and(|x| x.ends_with(v.as_str())),
            Casamento::Contem(v) => valor.is_some_and(|x| x.contains(v.as_str())),
        }
    }
}

/// Dart's `\w` (without unicode) plus the hyphen: `[-\w]`.
fn de_nome(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Reads a `[-\w]+` name starting at `i`; returns the end.
fn nome_ate(b: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < b.len() && de_nome(b[j]) {
        j += 1;
    }
    j
}

/// Tenta a alternativa do atributo: `\[([-\w]+)(?:([~|^$*]?=)(['"]?)([^\]'"]*)\6)?\]`.
/// Devolve (nome, operador, valor, fim).
fn atributo_em(s: &str, i: usize) -> Option<(String, Option<String>, Option<String>, usize)> {
    let b = s.as_bytes();
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let ini = i + 1;
    let fim_nome = nome_ate(b, ini);
    if fim_nome == ini {
        return None;
    }
    let nome = s[ini..fim_nome].to_string();
    // O grupo opcional: operador, aspas, valor e a mesma aspa.
    let opcional = (|| {
        let mut j = fim_nome;
        let mut op = String::new();
        if matches!(b.get(j), Some(b'~' | b'|' | b'^' | b'$' | b'*')) {
            op.push(b[j] as char);
            j += 1;
        }
        if b.get(j) != Some(&b'=') {
            return None;
        }
        op.push('=');
        j += 1;
        let aspa = match b.get(j) {
            Some(&q @ (b'\'' | b'"')) => {
                j += 1;
                Some(q)
            }
            _ => None,
        };
        let vini = j;
        while j < b.len() && !matches!(b[j], b']' | b'\'' | b'"') {
            j += 1;
        }
        let valor = s[vini..j].to_string();
        if let Some(q) = aspa {
            if b.get(j) != Some(&q) {
                return None;
            }
            j += 1;
        }
        Some((op, valor, j))
    })();
    match opcional {
        Some((op, valor, j)) if b.get(j) == Some(&b']') => {
            Some((nome, Some(op), Some(valor), j + 1))
        }
        // Without the optional group, the `]` must come right after the name.
        _ if b.get(fim_nome) == Some(&b']') => Some((nome, None, None, fim_nome + 1)),
        _ => None,
    }
}

/// The selector that receives what is read: the outer one or, inside `:not(`,
/// the last negated one.
fn alvo(atual: &mut Seletor, em_nao: bool) -> &mut Seletor {
    if em_nao {
        atual.nao.last_mut().expect("`:not(` abriu um seletor")
    } else {
        atual
    }
}

/// Whitespace, comma, whitespace: `\s*,\s*`.
fn virgula_em(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if b.get(j) != Some(&b',') {
        return None;
    }
    j += 1;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    Some(j)
}

impl Seletor {
    /// `CssSelector.parse`: the alternatives of the official regular
    /// expression, in the same order, and whatever matches none of them is
    /// skipped.
    pub fn analisar(texto: &str) -> Vec<Seletor> {
        fn fechar(res: &mut Vec<Seletor>, mut s: Seletor) {
            if !s.nao.is_empty()
                && s.elemento.is_none()
                && s.classes.is_empty()
                && s.atributos.is_empty()
            {
                s.elemento = Some("*".into());
            }
            res.push(s);
        }
        let b = texto.as_bytes();
        let mut resultados = Vec::new();
        let mut atual = Seletor::default();
        // Inside `:not(`, the selector that receives is the last one of
        // `atual.nao`.
        let mut em_nao = false;
        let mut i = 0;
        while i < b.len() {
            // 1. `:not(`
            if b[i..].starts_with(b":not(") {
                em_nao = true;
                atual.nao.push(Seletor::default());
                i += 5;
                continue;
            }
            // 2. nome de tag
            if de_nome(b[i]) {
                let f = nome_ate(b, i);
                alvo(&mut atual, em_nao).elemento = Some(texto[i..f].to_string());
                i = f;
                continue;
            }
            // 3. `.classe`
            if b[i] == b'.' && i + 1 < b.len() && de_nome(b[i + 1]) {
                let f = nome_ate(b, i + 1);
                alvo(&mut atual, em_nao)
                    .classes
                    .push(texto[i + 1..f].to_ascii_lowercase());
                i = f;
                continue;
            }
            // 4. `[atributo]`
            if let Some((nome, op, valor, f)) = atributo_em(texto, i) {
                let valor = valor.map(|v| v.to_lowercase());
                let casamento = match op.as_deref() {
                    None => Some(Casamento::Existe),
                    Some("=") => Some(Casamento::Igual(valor)),
                    Some(o) => {
                        let v = valor.unwrap_or_default();
                        // With an empty value these match nothing, and the
                        // official code does not even register them.
                        if v.is_empty() {
                            None
                        } else {
                            match o {
                                "~=" => Some(Casamento::Lista(v)),
                                "|=" => Some(Casamento::Hifen(v)),
                                "^=" => Some(Casamento::Prefixo(v)),
                                "$=" => Some(Casamento::Sufixo(v)),
                                _ => Some(Casamento::Contem(v)),
                            }
                        }
                    }
                };
                if let Some(casamento) = casamento {
                    alvo(&mut atual, em_nao)
                        .atributos
                        .push(Atributo { nome, casamento });
                }
                i = f;
                continue;
            }
            // 5. `)`
            if b[i] == b')' {
                em_nao = false;
                i += 1;
                continue;
            }
            // 6. `,`
            if let Some(f) = virgula_em(b, i) {
                fechar(&mut resultados, std::mem::take(&mut atual));
                em_nao = false;
                i = f;
                continue;
            }
            i += 1;
        }
        fechar(&mut resultados, atual);
        resultados
    }

    /// Is the selector just a tag name (`li-modal`)? It is the shape the child
    /// component emitter knows how to match by name.
    pub fn so_tag(&self) -> Option<&str> {
        match self {
            Seletor {
                elemento: Some(e),
                classes,
                atributos,
                nao,
            } if classes.is_empty() && atributos.is_empty() && nao.is_empty() && e != "*" => {
                Some(e)
            }
            _ => None,
        }
    }

    /// `SelectorMatcher.match` for a single selector, against the description
    /// of an element. The rules the official matching tree produces: `*` alone
    /// matches any element; `*` with a class or attribute matches nothing (the
    /// partial is looked up by tag name); each class and each attribute must be
    /// present; no `:not(...)` may match.
    pub fn casa(&self, el: &Elemento) -> bool {
        let simples = self.classes.is_empty() && self.atributos.is_empty();
        match self.elemento.as_deref() {
            Some("*") if simples => {}
            Some("*") => return false,
            Some(e) if e != el.nome => return false,
            _ => {}
        }
        if self.elemento.is_none() && simples {
            // Empty selector: does not go into any map.
            return false;
        }
        if !self.classes.iter().all(|c| el.classes.contains(c)) {
            return false;
        }
        let atributos_ok = self.atributos.iter().all(|a| {
            el.atributos
                .iter()
                .any(|(n, v)| *n == a.nome && a.casamento.aceita(v.as_deref()))
        });
        if !atributos_ok {
            return false;
        }
        !self.nao.iter().any(|n| n.casa_como_nao(el))
    }

    /// A `:not(...)` selector is matched by a separate `SelectorMatcher`,
    /// without the `*` rule that `addResult` applies.
    fn casa_como_nao(&self, el: &Elemento) -> bool {
        let simples = self.classes.is_empty() && self.atributos.is_empty();
        if simples && self.elemento.is_none() {
            return false;
        }
        Seletor {
            elemento: self.elemento.clone(),
            classes: self.classes.clone(),
            atributos: self.atributos.clone(),
            nao: Vec::new(),
        }
        .casa(el)
    }
}

/// The description of an element for matching (`createElementCssSelector`):
/// name, attributes with value (lowercase) and classes.
#[derive(Debug, Clone, Default)]
pub struct Elemento {
    pub nome: String,
    pub atributos: Vec<(String, Option<String>)>,
    pub classes: Vec<String>,
}

impl Elemento {
    pub fn novo(nome: &str) -> Self {
        Elemento {
            nome: nome.to_string(),
            ..Default::default()
        }
    }

    /// `cssSelector.addAttribute(name, '=', value)`, and the classes when the
    /// attribute is `class`.
    pub fn atributo(&mut self, nome: &str, valor: Option<&str>) {
        let valor = valor.map(|v| v.to_lowercase());
        if nome.eq_ignore_ascii_case("class")
            && let Some(v) = &valor
        {
            self.classes
                .extend(v.split_whitespace().map(str::to_string));
        }
        self.atributos.push((nome.to_string(), valor));
    }

    /// The description of a template element, as `_elementSelector` builds it:
    /// attributes, then properties (the `[(x)]` become the property `x` and the
    /// event `xChange`, like ngast's `DesugarVisitor`), then events.
    pub fn do_template(e: &crate::html::Elemento) -> Self {
        let mut el = Elemento::novo(&e.nome);
        for a in &e.atributos {
            el.atributo(&a.nome, Some(&a.valor));
        }
        for p in e.propriedades.iter().chain(&e.bananas) {
            el.atributo(&p.nome, Some(&p.valor));
        }
        for ev in &e.eventos {
            el.atributo(&ev.nome, Some(&ev.valor));
        }
        for b in &e.bananas {
            el.atributo(
                &format!("{}Change", b.nome),
                Some(&format!("{} = $event", b.valor)),
            );
        }
        el
    }
}

/// Algum seletor da lista casa?
pub fn casa_algum(seletores: &[Seletor], el: &Elemento) -> bool {
    seletores.iter().any(|s| s.casa(el))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn el(nome: &str, attrs: &[(&str, Option<&str>)]) -> Elemento {
        let mut e = Elemento::novo(nome);
        for (n, v) in attrs {
            e.atributo(n, *v);
        }
        e
    }

    #[test]
    fn analisa_as_formas_do_oficial() {
        let s = Seletor::analisar("form:not([ngNoForm]):not([ngFormModel]),ngForm,[ngForm]");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].elemento.as_deref(), Some("form"));
        assert_eq!(s[0].nao.len(), 2);
        assert_eq!(s[0].nao[0].atributos[0].nome, "ngNoForm");
        assert_eq!(s[1].elemento.as_deref(), Some("ngForm"));
        assert_eq!(s[2].atributos[0].nome, "ngForm");
        let s = Seletor::analisar("input:not([type=checkbox])[ngControl]");
        assert_eq!(
            s[0].nao[0].atributos[0].casamento,
            Casamento::Igual(Some("checkbox".into()))
        );
        assert_eq!(s[0].atributos[0].nome, "ngControl");
        let s = Seletor::analisar("[ngFor][ngForOf]");
        assert_eq!(s[0].atributos.len(), 2);
    }

    #[test]
    fn form_simples_casa_ng_form() {
        let s = Seletor::analisar("form:not([ngNoForm]):not([ngFormModel]),ngForm,[ngForm]");
        assert!(casa_algum(&s, &el("form", &[])));
        assert!(casa_algum(&s, &el("form", &[("class", Some("x"))])));
        assert!(!casa_algum(&s, &el("form", &[("ngNoForm", None)])));
        assert!(!casa_algum(&s, &el("div", &[])));
    }

    #[test]
    fn option_e_input_com_ng_model() {
        assert!(casa_algum(&Seletor::analisar("option"), &el("option", &[])));
        let s = Seletor::analisar(
            "input:not([type=checkbox])[ngControl],textarea[ngControl],input:not([type=checkbox])[ngModel],[ngDefaultControl]",
        );
        assert!(casa_algum(&s, &el("input", &[("ngModel", Some("x"))])));
        assert!(!casa_algum(
            &s,
            &el(
                "input",
                &[("type", Some("checkbox")), ("ngModel", Some("x"))]
            )
        ));
        assert!(!casa_algum(&s, &el("input", &[])));
    }

    #[test]
    fn classe_e_star() {
        let s = Seletor::analisar(".btn");
        assert!(casa_algum(&s, &el("a", &[("class", Some("x BTN"))])));
        assert!(!casa_algum(&s, &el("a", &[])));
        assert_eq!(Seletor::analisar("li-modal")[0].so_tag(), Some("li-modal"));
        assert_eq!(Seletor::analisar("[x]")[0].so_tag(), None);
    }
}
