//! YAML com posições, para os validadores de arquivos não-Dart
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, III.2.4 item 1 e etapa 7): a
//! árvore do documento com o intervalo (bytes da fonte) de cada nó, montada
//! sobre os eventos marcados do `yaml-rust2`.
//!
//! O analisador de eventos dá só o **início** de cada nó. O fim é calculado
//! aqui: de um escalar, pelo texto dele na fonte (aspas casadas, bloco pela
//! indentação); de uma lista ou mapa em fluxo, pelo `]` ou `}`; em bloco,
//! pelo fim do último filho (como o `package:yaml`).
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_diagnostics::Span;
use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

/// O valor de um nó.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    /// Vazio, `~` ou `null`.
    Nulo,
    /// Uma string (escalar entre aspas, de bloco, ou simples que não é
    /// número, booleano nem nulo).
    Texto(String),
    /// Número ou booleano, com o texto como escrito.
    Outro(String),
    Lista(Vec<No>),
    /// Os pares na ordem do arquivo.
    Mapa(Vec<(No, No)>),
}

/// Um nó com o intervalo dele na fonte.
#[derive(Debug, Clone, PartialEq)]
pub struct No {
    pub span: Span,
    pub valor: Valor,
}

impl No {
    /// O texto de um escalar string.
    pub fn texto(&self) -> Option<&str> {
        match &self.valor {
            Valor::Texto(t) => Some(t),
            _ => None,
        }
    }

    pub fn mapa(&self) -> Option<&[(No, No)]> {
        match &self.valor {
            Valor::Mapa(m) => Some(m),
            _ => None,
        }
    }

    pub fn lista(&self) -> Option<&[No]> {
        match &self.valor {
            Valor::Lista(l) => Some(l),
            _ => None,
        }
    }

    /// Escalar (`YamlScalar`): tudo o que não é lista nem mapa.
    pub fn escalar(&self) -> bool {
        !matches!(self.valor, Valor::Lista(_) | Valor::Mapa(_))
    }

    pub fn nulo(&self) -> bool {
        matches!(self.valor, Valor::Nulo)
    }

    /// O par de chave string `chave` de um mapa (`nodes[chave]`): a chave e
    /// o valor. A última ocorrência vale, como no mapa do `package:yaml`.
    pub fn par(&self, chave: &str) -> Option<&(No, No)> {
        self.mapa()?.iter().rev().find(|(k, _)| k.texto() == Some(chave))
    }

    pub fn campo(&self, chave: &str) -> Option<&No> {
        self.par(chave).map(|(_, v)| v)
    }
}

enum Aberto {
    Lista(usize, Vec<No>),
    Mapa(usize, Vec<(No, No)>, Option<No>),
}

struct Construtor<'a> {
    fonte: &'a str,
    /// O offset em bytes de cada caractere (o marcador conta caracteres).
    bytes: Vec<usize>,
    pilha: Vec<Aberto>,
    raiz: Option<No>,
}

impl Construtor<'_> {
    fn offset(&self, m: &Marker) -> usize {
        self.bytes.get(m.index()).copied().unwrap_or(self.fonte.len())
    }

    fn entregar(&mut self, no: No) {
        match self.pilha.last_mut() {
            Some(Aberto::Lista(_, itens)) => itens.push(no),
            Some(Aberto::Mapa(_, pares, chave)) => match chave.take() {
                Some(k) => pares.push((k, no)),
                None => *chave = Some(no),
            },
            None => {
                if self.raiz.is_none() {
                    self.raiz = Some(no);
                }
            }
        }
    }

    /// O fim de um escalar entre aspas que começa em `inicio`.
    fn fim_das_aspas(&self, inicio: usize) -> usize {
        let b = self.fonte.as_bytes();
        let Some(&aspa) = b.get(inicio) else { return inicio };
        let mut i = inicio + 1;
        while i < b.len() {
            if aspa == b'"' && b[i] == b'\\' {
                i += 2;
            } else if b[i] == aspa {
                // `''` dentro de aspas simples é uma aspa escapada.
                if aspa == b'\'' && b.get(i + 1) == Some(&b'\'') {
                    i += 2;
                } else {
                    return i + 1;
                }
            } else {
                i += 1;
            }
        }
        b.len()
    }

    /// O fim de um escalar de bloco (`|` ou `>`): a última linha de conteúdo
    /// com a indentação do bloco.
    fn fim_do_bloco(&self, inicio: usize) -> usize {
        let resto = &self.fonte[inicio..];
        let Some(primeira_quebra) = resto.find('\n') else { return self.fonte.len() };
        let mut fim = inicio + primeira_quebra;
        let mut posicao = fim + 1;
        let mut recuo: Option<usize> = None;
        for linha in self.fonte[posicao..].split_inclusive('\n') {
            let conteudo = linha.trim_end_matches(['\n', '\r']);
            if !conteudo.trim().is_empty() {
                let r = conteudo.len() - conteudo.trim_start().len();
                match recuo {
                    None if r == 0 => break,
                    None => recuo = Some(r),
                    Some(base) if r < base => break,
                    _ => {}
                }
                fim = posicao + conteudo.len();
            }
            posicao += linha.len();
        }
        fim
    }

    fn escalar(&self, valor: String, estilo: TScalarStyle, inicio: usize) -> No {
        match estilo {
            TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted => {
                No { span: Span { start: inicio, end: self.fim_das_aspas(inicio) }, valor: Valor::Texto(valor) }
            }
            TScalarStyle::Literal | TScalarStyle::Folded => {
                No { span: Span { start: inicio, end: self.fim_do_bloco(inicio) }, valor: Valor::Texto(valor) }
            }
            TScalarStyle::Plain => {
                let resto = &self.fonte[inicio.min(self.fonte.len())..];
                // O nó vazio chega como `~` sem o `~` estar na fonte.
                if valor == "~" && !resto.starts_with('~') {
                    return No { span: Span { start: inicio, end: inicio }, valor: Valor::Nulo };
                }
                let comprimento = if resto.starts_with(valor.as_str()) {
                    valor.len()
                } else {
                    // Um escalar simples de várias linhas: até o fim da
                    // primeira, sem o comentário.
                    let linha = resto.split('\n').next().unwrap_or("");
                    linha.find(" #").map_or(linha.len(), |k| k).min(linha.trim_end().len())
                };
                let span = Span { start: inicio, end: inicio + comprimento };
                let minusculo = valor.to_ascii_lowercase();
                let valor = if valor.is_empty() || valor == "~" || minusculo == "null" {
                    Valor::Nulo
                } else if minusculo == "true" || minusculo == "false" || valor.parse::<f64>().is_ok() || valor.starts_with("0x") {
                    Valor::Outro(valor)
                } else {
                    Valor::Texto(valor)
                };
                No { span, valor }
            }
        }
    }

    fn fechar(&mut self, marca: usize) {
        let Some(aberto) = self.pilha.pop() else { return };
        let (inicio, ultimo_fim, valor) = match aberto {
            Aberto::Lista(inicio, itens) => (inicio, itens.last().map(|n| n.span.end), Valor::Lista(itens)),
            Aberto::Mapa(inicio, pares, _) => (inicio, pares.last().map(|(_, v)| v.span.end), Valor::Mapa(pares)),
        };
        let em_fluxo = self.fonte[inicio.min(self.fonte.len())..].starts_with(['[', '{']);
        let fim = if em_fluxo { (marca + 1).min(self.fonte.len()) } else { ultimo_fim.unwrap_or(inicio).max(inicio) };
        self.entregar(No { span: Span { start: inicio, end: fim }, valor });
    }
}

impl MarkedEventReceiver for Construtor<'_> {
    fn on_event(&mut self, ev: Event, marca: Marker) {
        let offset = self.offset(&marca);
        match ev {
            Event::Scalar(valor, estilo, ..) => {
                let no = self.escalar(valor, estilo, offset);
                self.entregar(no);
            }
            Event::SequenceStart(..) => self.pilha.push(Aberto::Lista(offset, Vec::new())),
            Event::MappingStart(..) => self.pilha.push(Aberto::Mapa(offset, Vec::new(), None)),
            Event::SequenceEnd | Event::MappingEnd => self.fechar(offset),
            // Uma referência a âncora: sem o valor resolvido, um nulo.
            Event::Alias(_) => self.entregar(No { span: Span { start: offset, end: offset }, valor: Valor::Nulo }),
            _ => {}
        }
    }
}

/// Um erro de sintaxe do YAML: a mensagem e o offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroDeYaml {
    pub mensagem: String,
    pub offset: usize,
}

/// A árvore do primeiro documento de `fonte`; `None` para um arquivo sem
/// documento (vazio ou só comentários).
pub fn ler(fonte: &str) -> Result<Option<No>, ErroDeYaml> {
    let mut bytes: Vec<usize> = fonte.char_indices().map(|(i, _)| i).collect();
    bytes.push(fonte.len());
    let mut construtor = Construtor { fonte, bytes, pilha: Vec::new(), raiz: None };
    let mut analisador = Parser::new_from_str(fonte);
    match analisador.load(&mut construtor, false) {
        Ok(()) => Ok(construtor.raiz),
        Err(e) => {
            let offset = construtor.bytes.get(e.marker().index()).copied().unwrap_or(fonte.len());
            Err(ErroDeYaml { mensagem: e.info().to_string(), offset })
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn trecho<'a>(fonte: &'a str, n: &No) -> &'a str {
        &fonte[n.span.start..n.span.end]
    }

    #[test]
    fn posicoes_de_chaves_e_valores() {
        let fonte = "name: app\ndependencies:\n  a: ^1.0.0\n  b:\n    path: '../b' # perto\nflutter:\n";
        let raiz = ler(fonte).unwrap().unwrap();
        let (chave, valor) = raiz.par("name").unwrap();
        assert_eq!((trecho(fonte, chave), trecho(fonte, valor)), ("name", "app"));
        let b = raiz.campo("dependencies").unwrap().campo("b").unwrap();
        let (chave_do_caminho, caminho) = b.par("path").unwrap();
        assert_eq!(trecho(fonte, chave_do_caminho), "path");
        assert_eq!(trecho(fonte, caminho), "'../b'");
        assert_eq!(caminho.texto(), Some("../b"));
        // O valor vazio é nulo, de comprimento zero.
        let flutter = raiz.campo("flutter").unwrap();
        assert!(flutter.nulo() && flutter.span.start == flutter.span.end);
    }

    #[test]
    fn fluxo_e_tipos() {
        let fonte = "a: [1, x, true]\nb: {k: v}\n";
        let raiz = ler(fonte).unwrap().unwrap();
        let a = raiz.campo("a").unwrap();
        assert_eq!(trecho(fonte, a), "[1, x, true]");
        let itens = a.lista().unwrap();
        assert!(matches!(itens[0].valor, Valor::Outro(_)) && itens[1].texto() == Some("x") && matches!(itens[2].valor, Valor::Outro(_)));
        assert_eq!(trecho(fonte, raiz.campo("b").unwrap()), "{k: v}");
    }

    #[test]
    fn erro_de_sintaxe() {
        assert!(ler("a: [\n").is_err());
        assert_eq!(ler("# só comentário\n").unwrap(), None);
    }
}
