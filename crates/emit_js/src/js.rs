//! Texto JavaScript: expressões com precedência e escrita indentada.

/// Precedências (maior liga mais forte).
pub const P_PRIMARY: u8 = 20;
pub const P_NEW: u8 = 19;
pub const P_UNARY: u8 = 17;
pub const P_MUL: u8 = 15;
pub const P_ADD: u8 = 14;
pub const P_SHIFT: u8 = 13;
pub const P_REL: u8 = 12;
pub const P_EQ: u8 = 11;
pub const P_BITAND: u8 = 10;
pub const P_BITXOR: u8 = 9;
pub const P_BITOR: u8 = 8;
pub const P_AND: u8 = 7;
pub const P_OR: u8 = 6;
pub const P_COND: u8 = 4;
pub const P_ASSIGN: u8 = 3;
pub const P_YIELD: u8 = 2;
pub const P_COMMA: u8 = 1;

/// Contadores de medição (exemplo `memoria`): quantos `Js` foram construídos,
/// quantos bytes carregam, quantas vezes `at` emprestou o texto, quantas
/// precisou de parênteses e quantos nomes/literais foram formatados.
pub static JS_CONSTRUIDOS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static JS_BYTES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static AT_EMPRESTIMOS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static AT_PARENS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static NOMES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[inline]
fn conta(c: &std::sync::atomic::AtomicUsize, n: usize) {
    c.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
}

/// Expressão JS já escrita, com a precedência do operador mais externo.
#[derive(Clone, Debug)]
pub struct Js {
    pub code: String,
    pub prec: u8,
}

impl Js {
    pub fn new(code: impl Into<String>, prec: u8) -> Js {
        let code = code.into();
        conta(&JS_CONSTRUIDOS, 1);
        conta(&JS_BYTES, code.len());
        Js { code, prec }
    }
    pub fn prim(code: impl Into<String>) -> Js {
        Js::new(code, P_PRIMARY)
    }
    /// Texto com parênteses se a precedência for menor que `min`. Empresta o
    /// texto no caso comum (sem parênteses): quem o interpola num `format!`
    /// não paga uma cópia por filho.
    pub fn at(&self, min: u8) -> std::borrow::Cow<'_, str> {
        if self.prec < min {
            conta(&AT_PARENS, 1);
            std::borrow::Cow::Owned(format!("({})", self.code))
        } else {
            conta(&AT_EMPRESTIMOS, 1);
            std::borrow::Cow::Borrowed(&self.code)
        }
    }
    /// Como [`Js::at`], consumindo o `Js`: devolve o próprio texto sem copiar.
    pub fn into_at(self, min: u8) -> String {
        if self.prec < min {
            conta(&AT_PARENS, 1);
            format!("({})", self.code)
        } else {
            self.code
        }
    }
    pub fn paren(&self) -> Js {
        Js::prim(format!("({})", self.code))
    }
}

/// Literal de string JS a partir de unidades UTF-16 (surrogates soltos viram `\uXXXX`).
pub fn string_literal_units(units: impl Iterator<Item = u16>) -> String {
    let mut out = String::with_capacity(16);
    out.push('"');
    let units: Vec<u16> = units.collect();
    let mut i = 0;
    while i < units.len() {
        let u = units[i];
        match u {
            0x22 => out.push_str("\\\""),
            0x5C => out.push_str("\\\\"),
            0x0A => out.push_str("\\n"),
            0x0D => out.push_str("\\r"),
            0x09 => out.push_str("\\t"),
            0x08 => out.push_str("\\b"),
            0x0C => out.push_str("\\f"),
            0x0B => out.push_str("\\v"),
            0x2028 => out.push_str("\\u2028"),
            0x2029 => out.push_str("\\u2029"),
            0x00..=0x1F | 0x7F => out.push_str(&format!("\\u{:04X}", u)),
            0xD800..=0xDBFF => {
                if i + 1 < units.len() && (0xDC00..=0xDFFF).contains(&units[i + 1]) {
                    let hi = u32::from(u) - 0xD800;
                    let lo = u32::from(units[i + 1]) - 0xDC00;
                    let cp = 0x10000 + (hi << 10) + lo;
                    out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                    i += 1;
                } else {
                    out.push_str(&format!("\\u{:04X}", u));
                }
            }
            0xDC00..=0xDFFF => out.push_str(&format!("\\u{:04X}", u)),
            _ => out.push(char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}')),
        }
        i += 1;
    }
    out.push('"');
    out
}

pub fn string_literal(s: &str) -> String {
    conta(&NOMES, 1);
    string_literal_units(s.encode_utf16())
}

/// Escritor com indentação.
pub struct Writer {
    pub out: String,
    pub indent: usize,
}

impl Default for Writer {
    /// Buffer com 1 KiB de partida: um corpo de função cresce sem realocar
    /// nas primeiras dezenas de linhas (o `String` vazio realoca a cada
    /// dobra: 8, 16, 32… bytes).
    fn default() -> Self {
        Writer { out: String::with_capacity(1024), indent: 0 }
    }
}

impl Writer {
    pub fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }
    pub fn open(&mut self, s: &str) {
        self.line(s);
        self.indent += 1;
    }
    pub fn close(&mut self, s: &str) {
        self.indent = self.indent.saturating_sub(1);
        self.line(s);
    }
    pub fn push_raw(&mut self, s: &str) {
        self.out.push_str(s);
    }
    /// Escreve a indentação da linha corrente (usado pelas macros `linha!`,
    /// `abre!` e `fecha!`, que formatam direto no buffer).
    pub fn recuo(&mut self) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
    }
    pub fn fim_de_linha(&mut self) {
        self.out.push('\n');
    }
}

/// `linha!(w, "…{}", x)` escreve a linha formatando **dentro** do buffer do
/// [`Writer`], sem a `String` intermediária que `w.line(&format!(…))` aloca.
#[macro_export]
macro_rules! linha {
    ($w:expr, $($a:tt)*) => {{
        use std::fmt::Write as _;
        $w.recuo();
        let _ = write!($w.out, $($a)*);
        $w.fim_de_linha();
    }};
}

/// Como [`linha!`], abrindo um bloco (indenta as linhas seguintes).
#[macro_export]
macro_rules! abre {
    ($w:expr, $($a:tt)*) => {{
        $crate::linha!($w, $($a)*);
        $w.indent += 1;
    }};
}

/// Como [`linha!`], fechando o bloco aberto.
#[macro_export]
macro_rules! fecha {
    ($w:expr, $($a:tt)*) => {{
        $w.indent = $w.indent.saturating_sub(1);
        $crate::linha!($w, $($a)*);
    }};
}

/// Palavras reservadas do JS e nomes que o módulo usa; identificadores Dart
/// iguais a estes recebem `$` no fim.
pub fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "break" | "case" | "catch" | "class" | "const" | "continue" | "debugger" | "default" | "delete"
            | "do" | "else" | "enum" | "export" | "extends" | "false" | "finally" | "for" | "function"
            | "if" | "import" | "in" | "instanceof" | "new" | "null" | "return" | "super" | "switch"
            | "this" | "throw" | "true" | "try" | "typeof" | "var" | "void" | "while" | "with"
            | "yield" | "let" | "static" | "implements" | "interface" | "package" | "private"
            | "protected" | "public" | "await" | "async" | "arguments" | "eval" | "undefined"
            | "NaN" | "Infinity" | "Object" | "Symbol" | "Array" | "Math" | "JSON" | "Map" | "Set"
            | "Promise" | "String" | "Number" | "Boolean" | "Error" | "Function" | "Reflect"
            | "Proxy" | "Date" | "RegExp" | "globalThis" | "window" | "self" | "global" | "dart"
            | "dartx" | "core" | "dart_rti" | "_interceptors" | "_js_helper" | "_internal"
            | "collection" | "convert" | "math" | "typed_data" | "isolate" | "developer" | "io"
            | "js" | "js_util" | "js_interop" | "html" | "svg" | "web_audio" | "web_gl"
            | "indexed_db" | "html_common" | "_native_typed_data" | "_isolate_helper"
            | "_js_primitives" | "_foreign_helper" | "_ddc_only" | "_debugger" | "_js_names"
            | "_js_types" | "_metadata" | "_http" | "CT" | "C" | "I" | "asyncScope" | "constructor"
            | "prototype" | "__proto__" | "of" | "toString" | "valueOf" | "hasOwnProperty"
    )
}

/// Identificador JS para um nome Dart de variável/função local.
pub fn ident(name: &str) -> String {
    conta(&NOMES, 1);
    if is_reserved(name) || name.starts_with("t$") || name.starts_with("L$") {
        format!("{name}$")
    } else {
        name.to_string()
    }
}

/// Nome de propriedade JS: identificador direto ou `['...']`.
pub fn prop_access(name: &str) -> String {
    conta(&NOMES, 1);
    if is_js_ident(name) {
        format!(".{name}")
    } else {
        format!("[{}]", string_literal(name))
    }
}

pub fn is_js_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        && !matches!(
            name,
            "break" | "case" | "catch" | "class" | "const" | "continue" | "debugger" | "default"
                | "delete" | "do" | "else" | "enum" | "export" | "extends" | "false" | "finally"
                | "for" | "function" | "if" | "import" | "in" | "instanceof" | "new" | "null"
                | "return" | "super" | "switch" | "this" | "throw" | "true" | "try" | "typeof"
                | "var" | "void" | "while" | "with" | "yield" | "let" | "static"
        )
}

/// Chave de propriedade em literal de objeto ou em membro de classe.
pub fn prop_key(name: &str) -> String {
    conta(&NOMES, 1);
    if is_js_ident(name) {
        name.to_string()
    } else {
        format!("[{}]", string_literal(name))
    }
}
