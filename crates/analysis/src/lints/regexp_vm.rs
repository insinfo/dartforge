//! A validação de expressões regulares do `RegExp(...)` da VM do Dart 3.6.2
//! (`runtime/vm/regexp_parser.cc`): o que o `valid_regexps` pega como
//! `FormatException` quando o analyzer (que roda na VM) constrói o `RegExp`
//! com o texto do literal.
//!
//! As bandeiras são as do construtor com os padrões (`multiLine: false`,
//! `caseSensitive: true`, `dotAll: false`) e o `unicode` do argumento. O
//! porte segue o analisador linha a linha: o leitor (`ReadNext`, `Advance`,
//! `Reset` e a posição, que junta os pares de surrogates só com `unicode`), a
//! disjunção com a pilha de grupos, as classes de caracteres, os escapes, os
//! nomes de grupos (`ID_Start`/`ID_Continue` do Unicode 15.1, o do ICU 74.2),
//! as referências numeradas (com o `ScanForCaptures`) e nomeadas, e o
//! `\p{...}` com as tabelas de `regexp_unicode_g`.
//!
//! Do `RegExpBuilder` fica o que decide se um quantificador vale
//! (`AddQuantifierToAtom`): o vazio pendente, o surrogate pendente e a espécie
//! do último termo (texto, asserção, lookahead, lookbehind ou outro); o grupo
//! sem captura com um termo só é esse termo. Desvio conhecido: o tamanho
//! máximo casado (`max_match`) só é zero para asserções e lookarounds (uma
//! captura que só casa vazio conta como outro termo), o que só muda o
//! resultado com um surrogate isolado literal antes de um quantificador num
//! padrão `unicode`; e o caso em que a VM chega ao `UNREACHABLE` (nada a
//! quantificar depois de um surrogate pendente) não é relatado.
//! Escrito sem compilar nem executar (2026-10-05).

use super::regexp_unicode_g as tabelas;

const FIM: u32 = 1 << 21;
const MAX_CAPTURAS: i64 = 1 << 16;
const INFINITO: i64 = i32::MAX as i64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Grupo {
    Inicial,
    Captura,
    Agrupamento,
    LookaroundPositivo,
    LookaroundNegativo,
}

/// A espécie de um termo do construtor.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Termo {
    Texto,
    Assercao,
    Lookahead,
    Lookbehind,
    Outro,
}

impl Termo {
    fn lookaround(self) -> bool {
        matches!(self, Termo::Lookahead | Termo::Lookbehind)
    }
    /// `max_match() == 0`.
    fn so_vazio(self) -> bool {
        matches!(self, Termo::Assercao | Termo::Lookahead | Termo::Lookbehind)
    }
}

/// O `RegExpBuilder`, reduzido ao que decide os quantificadores.
#[derive(Default)]
struct Construtor {
    vazio_pendente: bool,
    surrogate_pendente: bool,
    termos: Vec<Termo>,
    alternativas: usize,
}

impl Construtor {
    fn descarregar_surrogate(&mut self) {
        if self.surrogate_pendente {
            self.surrogate_pendente = false;
            self.adicionar_termo(Termo::Outro);
        }
    }
    fn adicionar_termo(&mut self, t: Termo) {
        self.descarregar_surrogate();
        self.vazio_pendente = false;
        self.termos.push(t);
    }
    fn adicionar_caractere(&mut self) {
        self.adicionar_termo(Termo::Texto);
    }
    fn adicionar_atomo(&mut self, t: Option<Termo>) {
        match t {
            None => self.vazio_pendente = true,
            Some(t) => self.adicionar_termo(t),
        }
    }
    fn adicionar_vazio(&mut self) {
        self.vazio_pendente = true;
    }
    fn adicionar_lead(&mut self) {
        self.descarregar_surrogate();
        self.surrogate_pendente = true;
    }
    fn adicionar_trail(&mut self) {
        if self.surrogate_pendente {
            self.surrogate_pendente = false;
            self.adicionar_termo(Termo::Texto);
        } else {
            self.surrogate_pendente = true;
            self.descarregar_surrogate();
        }
    }
    fn adicionar_unicode(&mut self, c: u32, unicode: bool) {
        if c > 0xFFFF {
            self.adicionar_lead();
            self.adicionar_trail();
        } else if unicode && lead(c) {
            self.adicionar_lead();
        } else if unicode && trail(c) {
            self.adicionar_trail();
        } else {
            self.adicionar_caractere();
        }
    }
    fn adicionar_unicode_escapado(&mut self, c: u32, unicode: bool) {
        self.descarregar_surrogate();
        self.adicionar_unicode(c, unicode);
        self.descarregar_surrogate();
    }
    fn nova_alternativa(&mut self) {
        self.descarregar_surrogate();
        self.vazio_pendente = false;
        self.termos.clear();
        self.alternativas += 1;
    }
    /// `ToRegExp`: `None` para o `RegExpEmpty`; o termo único de uma
    /// alternativa única é o próprio termo.
    fn para_regexp(&mut self) -> Option<Termo> {
        self.descarregar_surrogate();
        if self.alternativas == 0 {
            return match self.termos[..] {
                [] => None,
                [t] => Some(t),
                _ => Some(Termo::Outro),
            };
        }
        Some(Termo::Outro)
    }
    /// `AddQuantifierToAtom`: `Err` no "invalid quantifier.".
    fn quantificar(&mut self, min: i64, unicode: bool) -> Result<(), ()> {
        if self.vazio_pendente {
            self.vazio_pendente = false;
            return Ok(());
        }
        let Some(&t) = self.termos.last() else {
            // `UNREACHABLE()` da VM: não relatado.
            return Ok(());
        };
        if t.lookaround() && (unicode || t == Termo::Lookbehind) {
            return Err(());
        }
        self.termos.pop();
        if t.so_vazio() {
            if min != 0 {
                self.termos.push(t);
            }
            return Ok(());
        }
        self.termos.push(Termo::Outro);
        Ok(())
    }
}

struct Estado {
    grupo: Grupo,
    lookbehind: bool,
    indice_de_captura: i64,
    nome: Option<Vec<u16>>,
    construtor: Construtor,
}

fn lead(c: u32) -> bool {
    (0xD800..=0xDBFF).contains(&c)
}

fn trail(c: u32) -> bool {
    (0xDC00..=0xDFFF).contains(&c)
}

fn ch(c: char) -> u32 {
    c as u32
}

fn digito(c: u32) -> bool {
    (ch('0')..=ch('9')).contains(&c)
}

fn alfanumerico(c: u32) -> bool {
    digito(c) || (ch('a')..=ch('z')).contains(&c) || (ch('A')..=ch('Z')).contains(&c)
}

/// `HexValue`.
fn hex(c: u32) -> i64 {
    let c = c.wrapping_sub(ch('0'));
    if c <= 9 {
        return c as i64;
    }
    let c = (c | 0x20).wrapping_sub(ch('a') - ch('0'));
    if c <= 5 {
        return c as i64 + 10;
    }
    -1
}

fn nas_faixas(faixas: &[(u32, u32)], c: u32) -> bool {
    faixas
        .binary_search_by(|&(a, b)| {
            if b < c {
                std::cmp::Ordering::Less
            } else if a > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn ascii_de_identificador(c: u32) -> bool {
    alfanumerico(c) || c == ch('_') || c == ch('$')
}

/// `IsIdentifierStart`.
fn inicio_de_identificador(c: u32) -> bool {
    if c > 127 {
        return nas_faixas(&tabelas::ID_START, c);
    }
    ascii_de_identificador(c) && !digito(c)
}

/// `IsIdentifierPart`.
fn parte_de_identificador(c: u32) -> bool {
    if c > 127 {
        return nas_faixas(&tabelas::ID_CONTINUE, c) || c == 0x200C || c == 0x200D;
    }
    ascii_de_identificador(c)
}

/// `IsSyntaxCharacterOrSlash`.
fn sintatico_ou_barra(c: u32) -> bool {
    matches!(char::from_u32(c), Some('^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '/'))
}

/// `IsUnicodePropertyValueCharacter(char)`: o código chega truncado a um
/// byte (`char`), e o byte acima de 127 é negativo.
fn caractere_de_propriedade(c: u32) -> bool {
    let b = c & 0xFF;
    alfanumerico(b) || b == ch('_')
}

struct Analisador<'a> {
    entrada: &'a [u16],
    unicode: bool,
    atual: u32,
    proxima: usize,
    tem_mais: bool,
    capturas_iniciadas: i64,
    contagem_de_capturas: i64,
    varrido: bool,
    tem_nomeadas: bool,
    capturas_nomeadas: Vec<Vec<u16>>,
    referencias_nomeadas: Vec<Vec<u16>>,
}

type R<T> = Result<T, ()>;

impl Analisador<'_> {
    /// `ReadNext`.
    fn ler(&self) -> (u32, usize) {
        let mut pos = self.proxima;
        let c0 = self.entrada[pos] as u32;
        let mut c = c0;
        pos += 1;
        if self.unicode && pos < self.entrada.len() && lead(c0) {
            let c1 = self.entrada[pos] as u32;
            if trail(c1) {
                c = 0x10000 + ((c0 - 0xD800) << 10) + (c1 - 0xDC00);
                pos += 1;
            }
        }
        (c, pos)
    }
    fn tem_proximo(&self) -> bool {
        self.proxima < self.entrada.len()
    }
    fn proximo(&self) -> u32 {
        if self.tem_proximo() { self.ler().0 } else { FIM }
    }
    fn avancar(&mut self) {
        if self.tem_proximo() {
            let (c, pos) = self.ler();
            self.atual = c;
            self.proxima = pos;
        } else {
            self.atual = FIM;
            self.proxima = self.entrada.len() + 1;
            self.tem_mais = false;
        }
    }
    fn avancar_n(&mut self, n: usize) {
        self.proxima += n - 1;
        self.avancar();
    }
    fn reiniciar(&mut self, pos: usize) {
        self.proxima = pos;
        self.tem_mais = pos < self.entrada.len();
        self.avancar();
    }
    fn posicao(&self) -> usize {
        self.proxima - 1
    }

    /// `ParseDisjunction`.
    fn disjuncao(&mut self) -> R<()> {
        let mut pilha: Vec<Estado> =
            vec![Estado { grupo: Grupo::Inicial, lookbehind: false, indice_de_captura: 0, nome: None, construtor: Construtor::default() }];
        loop {
            let c = self.atual;
            let unicode = self.unicode;
            // `true`: o átomo admite quantificador (o `break` do `switch`).
            let quantificavel = if c == FIM {
                if pilha.len() > 1 {
                    return Err(()); // "Unterminated group"
                }
                return Ok(());
            } else if c == ch(')') {
                if pilha.len() == 1 {
                    return Err(()); // "Unmatched ')'"
                }
                self.avancar();
                let mut estado = pilha.pop().expect("grupo aberto");
                let corpo = estado.construtor.para_regexp();
                let termo = match estado.grupo {
                    Grupo::Captura => {
                        if let Some(nome) = estado.nome.take() {
                            // `CreateNamedCaptureAtIndex`.
                            if self.capturas_nomeadas.contains(&nome) {
                                return Err(()); // "Duplicate capture group name"
                            }
                            self.capturas_nomeadas.push(nome);
                        }
                        Some(Termo::Outro)
                    }
                    Grupo::Agrupamento => corpo,
                    _ => Some(if estado.lookbehind { Termo::Lookbehind } else { Termo::Lookahead }),
                };
                pilha.last_mut().expect("estado inicial").construtor.adicionar_atomo(termo);
                true
            } else if c == ch('|') {
                self.avancar();
                pilha.last_mut().expect("estado").construtor.nova_alternativa();
                false
            } else if c == ch('*') || c == ch('+') || c == ch('?') {
                return Err(()); // "Nothing to repeat"
            } else if c == ch('^') || c == ch('$') {
                self.avancar();
                pilha.last_mut().expect("estado").construtor.adicionar_termo(Termo::Assercao);
                false
            } else if c == ch('.') {
                self.avancar();
                pilha.last_mut().expect("estado").construtor.adicionar_termo(Termo::Texto);
                true
            } else if c == ch('(') {
                let novo = self.abrir_parentese(pilha.last().expect("estado"))?;
                pilha.push(novo);
                false
            } else if c == ch('[') {
                self.classe()?;
                pilha.last_mut().expect("estado").construtor.adicionar_termo(Termo::Texto);
                true
            } else if c == ch('\\') {
                self.escape(&mut pilha)?
            } else {
                if c == ch('{') && self.intervalo().is_some() {
                    return Err(()); // "Nothing to repeat"
                }
                if (c == ch('{') || c == ch('}') || c == ch(']')) && unicode {
                    return Err(()); // "Lone quantifier brackets"
                }
                let atual = self.atual;
                pilha.last_mut().expect("estado").construtor.adicionar_unicode(atual, unicode);
                self.avancar();
                true
            };
            if !quantificavel {
                continue;
            }
            let c = self.atual;
            let min;
            if c == ch('*') || c == ch('?') {
                min = 0;
                self.avancar();
            } else if c == ch('+') {
                min = 1;
                self.avancar();
            } else if c == ch('{') {
                match self.intervalo() {
                    Some((a, b)) => {
                        if b < a {
                            return Err(()); // "numbers out of order in {} quantifier."
                        }
                        min = a;
                    }
                    None => continue,
                }
            } else {
                continue;
            }
            if self.atual == ch('?') {
                self.avancar();
            }
            pilha.last_mut().expect("estado").construtor.quantificar(min, unicode)?; // "invalid quantifier."
        }
    }

    /// O `\` fora de classe; devolve se o átomo admite quantificador.
    fn escape(&mut self, pilha: &mut [Estado]) -> R<bool> {
        let unicode = self.unicode;
        let n = self.proximo();
        let topo = pilha.len() - 1;
        if n == FIM {
            return Err(()); // "\\ at end of pattern"
        }
        let Some(nc) = char::from_u32(n) else {
            return self.escape_de_identidade(pilha);
        };
        match nc {
            'b' | 'B' => {
                self.avancar_n(2);
                pilha[topo].construtor.adicionar_termo(Termo::Assercao);
                Ok(false)
            }
            'd' | 'D' | 's' | 'S' | 'w' | 'W' => {
                self.avancar_n(2);
                pilha[topo].construtor.adicionar_termo(Termo::Texto);
                Ok(true)
            }
            'p' | 'P' => {
                self.avancar_n(2);
                if unicode {
                    if !self.propriedade() {
                        return Err(()); // "Invalid property name"
                    }
                    pilha[topo].construtor.adicionar_termo(Termo::Texto);
                } else {
                    pilha[topo].construtor.adicionar_caractere();
                }
                Ok(true)
            }
            '1'..='9' => {
                if let Some(indice) = self.indice_de_referencia() {
                    if dentro_da_captura(pilha, indice) {
                        pilha[topo].construtor.adicionar_vazio();
                    } else {
                        pilha[topo].construtor.adicionar_atomo(Some(Termo::Outro));
                    }
                    return Ok(true);
                }
                if unicode {
                    return Err(()); // kUnicodeIdentity
                }
                if nc == '8' || nc == '9' {
                    pilha[topo].construtor.adicionar_caractere();
                    self.avancar_n(2);
                    return Ok(true);
                }
                self.escape_zero(pilha)
            }
            '0' => self.escape_zero(pilha),
            'f' | 'n' | 'r' | 't' | 'v' => {
                self.avancar_n(2);
                pilha[topo].construtor.adicionar_caractere();
                Ok(true)
            }
            'c' => {
                self.avancar();
                let controle = self.proximo();
                let letra = controle & !0x20;
                if letra < ch('A') || ch('Z') < letra {
                    if unicode {
                        return Err(()); // kUnicodeIdentity
                    }
                    // A barra vira caractere; o `c` é lido de novo.
                    pilha[topo].construtor.adicionar_caractere();
                } else {
                    self.avancar_n(2);
                    pilha[topo].construtor.adicionar_caractere();
                }
                Ok(true)
            }
            'x' => {
                self.avancar_n(2);
                if self.escape_hex(2).is_some() || !unicode {
                    pilha[topo].construtor.adicionar_caractere();
                    Ok(true)
                } else {
                    Err(()) // kUnicodeIdentity
                }
            }
            'u' => {
                self.avancar_n(2);
                if let Some(v) = self.escape_unicode() {
                    pilha[topo].construtor.adicionar_unicode_escapado(v, unicode);
                    Ok(true)
                } else if !unicode {
                    pilha[topo].construtor.adicionar_caractere();
                    Ok(true)
                } else {
                    Err(()) // kUnicodeIdentity
                }
            }
            'k' if unicode || self.tem_capturas_nomeadas() => {
                self.avancar_n(2);
                self.referencia_nomeada(pilha)?;
                Ok(true)
            }
            _ => self.escape_de_identidade(pilha),
        }
    }

    /// O `default:` do `\`: o escape de identidade.
    fn escape_de_identidade(&mut self, pilha: &mut [Estado]) -> R<bool> {
        self.avancar();
        if !self.unicode || sintatico_ou_barra(self.atual) {
            let topo = pilha.len() - 1;
            pilha[topo].construtor.adicionar_caractere();
            self.avancar();
            Ok(true)
        } else {
            Err(()) // kUnicodeIdentity
        }
    }

    /// `case '0':` (e a queda dos dígitos que não são referência).
    fn escape_zero(&mut self, pilha: &mut [Estado]) -> R<bool> {
        self.avancar();
        if self.unicode && digito(self.proximo()) {
            return Err(()); // "Invalid decimal escape"
        }
        self.octal();
        let topo = pilha.len() - 1;
        pilha[topo].construtor.adicionar_caractere();
        Ok(true)
    }

    /// `ParseOpenParenthesis`.
    fn abrir_parentese(&mut self, estado: &Estado) -> R<Estado> {
        let mut lookbehind = estado.lookbehind;
        let mut nomeada = false;
        let mut grupo = Grupo::Captura;
        self.avancar();
        if self.atual == ch('?') {
            let n = self.proximo();
            if n == ch(':') {
                self.avancar_n(2);
                grupo = Grupo::Agrupamento;
            } else if n == ch('=') || n == ch('!') {
                self.avancar_n(2);
                lookbehind = false;
                grupo = if n == ch('=') { Grupo::LookaroundPositivo } else { Grupo::LookaroundNegativo };
            } else if n == ch('<') {
                self.avancar();
                let n2 = self.proximo();
                if n2 == ch('=') || n2 == ch('!') {
                    self.avancar_n(2);
                    lookbehind = true;
                    grupo = if n2 == ch('=') { Grupo::LookaroundPositivo } else { Grupo::LookaroundNegativo };
                } else {
                    nomeada = true;
                    self.tem_nomeadas = true;
                    self.avancar();
                }
            } else {
                return Err(()); // "Invalid group"
            }
        }
        let mut nome = None;
        if grupo == Grupo::Captura {
            if self.capturas_iniciadas >= MAX_CAPTURAS {
                return Err(()); // "Too many captures"
            }
            self.capturas_iniciadas += 1;
            if nomeada {
                nome = Some(self.nome_de_grupo()?);
            }
        }
        Ok(Estado { grupo, lookbehind, indice_de_captura: self.capturas_iniciadas, nome, construtor: Construtor::default() })
    }

    /// `ScanForCaptures`.
    fn varrer_capturas(&mut self) {
        let salvo = self.posicao();
        let mut n = self.capturas_iniciadas;
        loop {
            let c = self.atual;
            if c == FIM {
                break;
            }
            self.avancar();
            if c == ch('\\') {
                self.avancar();
            } else if c == ch('[') {
                loop {
                    let c2 = self.atual;
                    if c2 == FIM {
                        break;
                    }
                    self.avancar();
                    if c2 == ch('\\') {
                        self.avancar();
                    } else if c2 == ch(']') {
                        break;
                    }
                }
            } else if c == ch('(') {
                if self.atual == ch('?') {
                    self.avancar();
                    if self.atual != ch('<') {
                        continue;
                    }
                    self.avancar();
                    if self.atual == ch('=') || self.atual == ch('!') {
                        continue;
                    }
                    self.tem_nomeadas = true;
                }
                n += 1;
            }
        }
        self.contagem_de_capturas = n;
        self.varrido = true;
        self.reiniciar(salvo);
    }

    /// `ParseBackReferenceIndex`.
    fn indice_de_referencia(&mut self) -> Option<i64> {
        let inicio = self.posicao();
        let mut valor = (self.proximo() - ch('0')) as i64;
        self.avancar_n(2);
        while digito(self.atual) {
            valor = 10 * valor + (self.atual - ch('0')) as i64;
            if valor > MAX_CAPTURAS {
                self.reiniciar(inicio);
                return None;
            }
            self.avancar();
        }
        if valor > self.capturas_iniciadas {
            if !self.varrido {
                self.varrer_capturas();
            }
            if valor > self.contagem_de_capturas {
                self.reiniciar(inicio);
                return None;
            }
        }
        Some(valor)
    }

    /// `HasNamedCaptures`.
    fn tem_capturas_nomeadas(&mut self) -> bool {
        if self.tem_nomeadas || self.varrido {
            return self.tem_nomeadas;
        }
        self.varrer_capturas();
        self.tem_nomeadas
    }

    /// `ParseCaptureGroupName`.
    fn nome_de_grupo(&mut self) -> R<Vec<u16>> {
        let mut nome: Vec<u16> = Vec::new();
        let empurrar = |nome: &mut Vec<u16>, c: u32| {
            if c <= 0xFFFF {
                nome.push(c as u16);
            } else {
                let v = c - 0x10000;
                nome.push((0xD800 + (v >> 10)) as u16);
                nome.push((0xDC00 + (v & 0x3FF)) as u16);
            }
        };
        let mut no_inicio = true;
        loop {
            let mut c = self.atual;
            self.avancar();
            if c == ch('\\') && self.atual == ch('u') {
                self.avancar();
                match self.escape_unicode() {
                    Some(v) => c = v,
                    None => return Err(()), // "Invalid Unicode escape sequence"
                }
            }
            // A barra passaria por `ID_Start` e `ID_Continue`.
            if c == ch('\\') {
                return Err(()); // "Invalid capture group name"
            }
            if no_inicio {
                if !inicio_de_identificador(c) {
                    return Err(());
                }
                empurrar(&mut nome, c);
                no_inicio = false;
            } else if c == ch('>') {
                break;
            } else if parte_de_identificador(c) {
                empurrar(&mut nome, c);
            } else {
                return Err(());
            }
        }
        Ok(nome)
    }

    /// `ParseNamedBackReference`.
    fn referencia_nomeada(&mut self, pilha: &mut [Estado]) -> R<()> {
        if self.atual != ch('<') {
            return Err(()); // "Invalid named reference"
        }
        self.avancar();
        let nome = self.nome_de_grupo()?;
        let topo = pilha.len() - 1;
        if pilha.iter().any(|s| s.nome.as_ref() == Some(&nome)) {
            pilha[topo].construtor.adicionar_vazio();
        } else {
            pilha[topo].construtor.adicionar_atomo(Some(Termo::Outro));
            self.referencias_nomeadas.push(nome);
        }
        Ok(())
    }

    /// Os dígitos de um limite de `{}`, saturados em `kInfinity`.
    fn numero_do_intervalo(&mut self) -> i64 {
        let mut n: i64 = 0;
        while digito(self.atual) {
            let d = (self.atual - ch('0')) as i64;
            if n > (INFINITO - d) / 10 {
                loop {
                    self.avancar();
                    if !digito(self.atual) {
                        break;
                    }
                }
                return INFINITO;
            }
            n = 10 * n + d;
            self.avancar();
        }
        n
    }

    /// `ParseIntervalQuantifier`: `(min, max)`, ou nada com a posição
    /// restaurada.
    fn intervalo(&mut self) -> Option<(i64, i64)> {
        let inicio = self.posicao();
        self.avancar();
        if !digito(self.atual) {
            self.reiniciar(inicio);
            return None;
        }
        let min = self.numero_do_intervalo();
        let max;
        if self.atual == ch('}') {
            max = min;
            self.avancar();
        } else if self.atual == ch(',') {
            self.avancar();
            if self.atual == ch('}') {
                max = INFINITO;
                self.avancar();
            } else {
                max = self.numero_do_intervalo();
                if self.atual != ch('}') {
                    self.reiniciar(inicio);
                    return None;
                }
                self.avancar();
            }
        } else {
            self.reiniciar(inicio);
            return None;
        }
        Some((min, max))
    }

    /// `ParseOctalLiteral`.
    fn octal(&mut self) -> u32 {
        let octal = |c: u32| (ch('0')..=ch('7')).contains(&c);
        let mut valor = self.atual.wrapping_sub(ch('0'));
        self.avancar();
        if octal(self.atual) {
            valor = valor.wrapping_mul(8).wrapping_add(self.atual - ch('0'));
            self.avancar();
            if valor < 32 && octal(self.atual) {
                valor = valor * 8 + self.atual - ch('0');
                self.avancar();
            }
        }
        valor
    }

    /// `ParseHexEscape`.
    fn escape_hex(&mut self, tamanho: usize) -> Option<u32> {
        let inicio = self.posicao();
        let mut valor: u32 = 0;
        for _ in 0..tamanho {
            let d = hex(self.atual);
            if d < 0 {
                self.reiniciar(inicio);
                return None;
            }
            valor = valor.wrapping_mul(16).wrapping_add(d as u32);
            self.avancar();
        }
        Some(valor)
    }

    /// `ParseUnlimitedLengthHexNumber`.
    fn numero_hex(&mut self, maximo: u32) -> Option<u32> {
        let mut x: u32 = 0;
        let mut d = hex(self.atual);
        if d < 0 {
            return None;
        }
        while d >= 0 {
            x = x * 16 + d as u32;
            if x > maximo {
                return None;
            }
            self.avancar();
            d = hex(self.atual);
        }
        Some(x)
    }

    /// `ParseUnicodeEscape` (o `\u` já lido).
    fn escape_unicode(&mut self) -> Option<u32> {
        if self.atual == ch('{') && self.unicode {
            let inicio = self.posicao();
            self.avancar();
            if let Some(v) = self.numero_hex(0x10FFFF)
                && self.atual == ch('}')
            {
                self.avancar();
                return Some(v);
            }
            self.reiniciar(inicio);
            return None;
        }
        let v = self.escape_hex(4)?;
        if self.unicode && lead(v) && self.atual == ch('\\') {
            let inicio = self.posicao();
            if self.proximo() == ch('u') {
                self.avancar_n(2);
                if let Some(t) = self.escape_hex(4)
                    && trail(t)
                {
                    return Some(0x10000 + ((v - 0xD800) << 10) + (t - 0xDC00));
                }
            }
            self.reiniciar(inicio);
        }
        Some(v)
    }

    /// `ParsePropertyClassName` + `AddPropertyClassRange` (o `\p`/`\P` já
    /// lido): `false` no nome que não vale. Os nomes são exatos (o ICU acha
    /// pelo casamento solto, e a VM confere o apelido exato).
    fn propriedade(&mut self) -> bool {
        let mut nome1 = String::new();
        let mut nome2: Option<String> = None;
        if self.atual != ch('{') {
            return false;
        }
        self.avancar();
        while self.atual != ch('}') && self.atual != ch('=') {
            if !caractere_de_propriedade(self.atual) || !self.tem_proximo() {
                return false;
            }
            nome1.push((self.atual & 0xFF) as u8 as char);
            self.avancar();
        }
        if self.atual == ch('=') {
            let mut v = String::new();
            self.avancar();
            while self.atual != ch('}') {
                if !caractere_de_propriedade(self.atual) || !self.tem_proximo() {
                    return false;
                }
                v.push((self.atual & 0xFF) as u8 as char);
                self.avancar();
            }
            nome2 = Some(v);
        }
        self.avancar();
        let em = |lista: &[&str], n: &str| lista.contains(&n);
        match &nome2 {
            // A categoria geral, `Any`/`ASCII`/`Assigned` e as binárias.
            None => {
                em(&tabelas::CATEGORIAS_GERAIS, &nome1)
                    || matches!(nome1.as_str(), "Any" | "ASCII" | "Assigned")
                    || em(&tabelas::PROPRIEDADES_BINARIAS, &nome1)
            }
            Some(valor) => match nome1.as_str() {
                "gc" | "General_Category" => em(&tabelas::CATEGORIAS_GERAIS, valor),
                "sc" | "Script" => em(&tabelas::ESCRITAS, valor),
                "scx" | "Script_Extensions" => em(&tabelas::ESCRITAS_ESTENDIDAS, valor),
                _ => false,
            },
        }
    }

    /// `ParseClassCharacterEscape` (no `\`).
    fn escape_de_classe_caractere(&mut self) -> R<u32> {
        self.avancar();
        let c = self.atual;
        let controle_simples = match char::from_u32(c) {
            Some('b') => Some(0x08),
            Some('f') => Some(0x0C),
            Some('n') => Some(0x0A),
            Some('r') => Some(0x0D),
            Some('t') => Some(0x09),
            Some('v') => Some(0x0B),
            _ => None,
        };
        if let Some(x) = controle_simples {
            self.avancar();
            return Ok(x);
        }
        if c == ch('c') {
            let controle = self.proximo();
            let letra = controle & !0x20;
            if (ch('A')..=ch('Z')).contains(&letra) {
                self.avancar_n(2);
                return Ok(controle & 0x1F);
            }
            if self.unicode {
                return Err(()); // "Invalid class escape"
            }
            if digito(controle) || controle == ch('_') {
                self.avancar_n(2);
                return Ok(controle & 0x1F);
            }
            return Ok(ch('\\'));
        }
        if c == ch('0') && self.unicode && !digito(self.proximo()) {
            self.avancar();
            return Ok(0);
        }
        if (ch('0')..=ch('7')).contains(&c) {
            if self.unicode {
                return Err(()); // "Invalid class escape"
            }
            return Ok(self.octal());
        }
        if c == ch('x') {
            self.avancar();
            if let Some(v) = self.escape_hex(2) {
                return Ok(v);
            }
            if self.unicode {
                return Err(()); // "Invalid escape"
            }
            return Ok(ch('x'));
        }
        if c == ch('u') {
            self.avancar();
            if let Some(v) = self.escape_unicode() {
                return Ok(v);
            }
            if self.unicode {
                return Err(()); // kUnicodeIdentity
            }
            return Ok(ch('u'));
        }
        if !self.unicode || sintatico_ou_barra(c) || c == ch('-') {
            self.avancar();
            return Ok(c);
        }
        Err(()) // kUnicodeIdentity
    }

    /// `ParseClassEscape`: `(é classe, caractere)`.
    fn escape_de_classe(&mut self) -> R<(bool, u32)> {
        let primeiro = self.atual;
        if primeiro == ch('\\') {
            let n = self.proximo();
            if [ch('w'), ch('W'), ch('d'), ch('D'), ch('s'), ch('S')].contains(&n) {
                self.avancar_n(2);
                return Ok((true, 0));
            }
            if (n == ch('p') || n == ch('P')) && self.unicode {
                self.avancar_n(2);
                if !self.propriedade() {
                    return Err(()); // "Invalid property name in character class"
                }
                return Ok((true, 0));
            }
            if n == FIM {
                return Err(()); // "\\ at end of pattern"
            }
            return Ok((false, self.escape_de_classe_caractere()?));
        }
        self.avancar();
        Ok((false, primeiro))
    }

    /// `ParseCharacterClass`.
    fn classe(&mut self) -> R<()> {
        self.avancar();
        if self.atual == ch('^') {
            self.avancar();
        }
        while self.tem_mais && self.atual != ch(']') {
            let (classe1, c1) = self.escape_de_classe()?;
            if self.atual == ch('-') {
                self.avancar();
                if self.atual == FIM || self.atual == ch(']') {
                    break;
                }
                let (classe2, c2) = self.escape_de_classe()?;
                if classe1 || classe2 {
                    if self.unicode {
                        return Err(()); // "Invalid character class"
                    }
                    continue;
                }
                if c1 > c2 {
                    return Err(()); // "Range out of order in character class"
                }
            }
        }
        if !self.tem_mais {
            return Err(()); // "Unterminated character class"
        }
        self.avancar();
        Ok(())
    }
}

/// `IsInsideCaptureGroup(index)`.
fn dentro_da_captura(pilha: &[Estado], indice: i64) -> bool {
    for s in pilha.iter().rev() {
        if s.grupo != Grupo::Captura {
            continue;
        }
        if indice == s.indice_de_captura {
            return true;
        }
        if indice > s.indice_de_captura {
            return false;
        }
    }
    false
}

/// `RegExp(fonte, unicode: unicode)` da VM não lança `FormatException`.
pub fn valida(fonte: &[u16], unicode: bool) -> bool {
    let mut a = Analisador {
        entrada: fonte,
        unicode,
        atual: FIM,
        proxima: 0,
        tem_mais: true,
        capturas_iniciadas: 0,
        contagem_de_capturas: 0,
        varrido: false,
        tem_nomeadas: false,
        capturas_nomeadas: Vec::new(),
        referencias_nomeadas: Vec::new(),
    };
    a.avancar();
    if a.disjuncao().is_err() {
        return false;
    }
    // `PatchNamedBackReferences`.
    a.referencias_nomeadas.iter().all(|r| a.capturas_nomeadas.contains(r))
}

#[cfg(test)]
mod testes {
    use super::valida;

    fn v(s: &str, u: bool) -> bool {
        valida(&s.encode_utf16().collect::<Vec<u16>>(), u)
    }

    #[test]
    fn erros_e_validos_da_vm() {
        assert!(v("a+b*", false));
        assert!(!v("(", false));
        assert!(!v(")", false));
        assert!(!v("*a", false));
        assert!(!v("a{2,1}", false));
        assert!(v("a{,1}", false));
        assert!(!v("a{,1}", true));
        assert!(!v("[b-a]", false));
        assert!(!v("[a", false));
        assert!(!v("\\", false));
        assert!(!v("(?<=a)*", false));
        assert!(v("(?=a)*", false));
        assert!(!v("(?=a)*", true));
        assert!(!v("(?:(?<=a))+", false));
        assert!(v("\\1(a)", false));
        assert!(v("\\2(a)", false));
        assert!(!v("\\2(a)", true));
        assert!(!v("(?<n>a)(?<n>b)", false));
        assert!(!v("\\k<m>(?<n>a)", false));
        assert!(v("\\k<m>", false));
        assert!(v("\\p{Lu}", true));
        assert!(v("\\p{Script=Latin}", true));
        assert!(!v("\\p{Script=Hrkt}", true));
        assert!(!v("\\p{lu}", true));
        assert!(!v("\\-", true));
        assert!(v("[\\-]", true));
        assert!(!v("[\\d-a]", true));
        assert!(v("[\\d-a]", false));
        assert!(!v("(?a)", false));
        assert!(v("(?<ação>x)\\k<ação>", false));
    }
}
