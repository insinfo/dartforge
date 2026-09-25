//! Diagnósticos compartilhados entre compilador, analisador e servidor de linguagem.
//!
//! Um [`Diagnostic`] pode carregar um [`Codigo`] do `package:analyzer` oficial,
//! com severidade e argumentos. A mensagem de um diagnóstico com código é
//! **renderizada do molde oficial em inglês** ([`InfoCodigo::mensagem`]), com a
//! mesma regra do `formatList` do analyzer (`src/generated/java_core.dart`):
//! cada `{n}` vira o argumento `n`. A tabela de moldes ([`codigos`]) é gerada
//! do `analyzer-6.11.0` (o analyzer do SDK 3.6) por
//! `cargo run -p dartforge-paridade --example gerar_codigos`, e não é editada
//! à mão.

mod codigos_g;

/// Códigos do analyzer por classe de origem (`codigos::compile_time_error::X`,
/// `codigos::warning::X`, `codigos::parser::X`...), gerados.
pub mod codigos {
    pub use crate::codigos_g::modulos::*;
}

/// Intervalo semiaberto de bytes UTF-8 no arquivo de origem.
///
/// Adaptadores de editor devem converter esses offsets para a codificação do protocolo,
/// por exemplo unidades UTF-16 no LSP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Span {
    /// Primeiro byte incluído.
    pub start: usize,
    /// Primeiro byte depois do intervalo.
    pub end: usize,
}

/// Severidade de um diagnóstico, na nomenclatura do `ErrorSeverity` do analyzer.
///
/// A ordem da enumeração é a de relato do `dart analyze`: erros primeiro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Severidade {
    /// `ERROR`: erro de compilação ou de sintaxe.
    #[default]
    Error,
    /// `WARNING`: aviso (antigo "hint" promovido, `StaticWarningCode`, `WarningCode`).
    Warning,
    /// `INFO`: dicas, lints e TODOs.
    Info,
}

impl Severidade {
    /// O nome em `ErrorSeverity.name` (`"ERROR"`, `"WARNING"`, `"INFO"`).
    pub const fn nome(self) -> &'static str {
        match self {
            Severidade::Error => "ERROR",
            Severidade::Warning => "WARNING",
            Severidade::Info => "INFO",
        }
    }

    /// Lê o nome de [`Severidade::nome`].
    pub fn por_nome(nome: &str) -> Option<Self> {
        match nome {
            "ERROR" => Some(Severidade::Error),
            "WARNING" => Some(Severidade::Warning),
            "INFO" => Some(Severidade::Info),
            _ => None,
        }
    }
}

/// Tipo de um código, na nomenclatura do `ErrorType` do analyzer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TipoErro {
    /// `TODO`.
    Todo,
    /// `HINT`.
    Hint,
    /// `COMPILE_TIME_ERROR`.
    CompileTimeError,
    /// `CHECKED_MODE_COMPILE_TIME_ERROR`.
    CheckedModeCompileTimeError,
    /// `STATIC_WARNING` (também o dos `WarningCode`).
    StaticWarning,
    /// `SYNTACTIC_ERROR` (parser e scanner).
    SyntacticError,
    /// `LINT`.
    Lint,
}

impl TipoErro {
    /// O nome em `ErrorType.name`.
    pub const fn nome(self) -> &'static str {
        match self {
            TipoErro::Todo => "TODO",
            TipoErro::Hint => "HINT",
            TipoErro::CompileTimeError => "COMPILE_TIME_ERROR",
            TipoErro::CheckedModeCompileTimeError => "CHECKED_MODE_COMPILE_TIME_ERROR",
            TipoErro::StaticWarning => "STATIC_WARNING",
            TipoErro::SyntacticError => "SYNTACTIC_ERROR",
            TipoErro::Lint => "LINT",
        }
    }

    /// Lê o nome de [`TipoErro::nome`].
    pub fn por_nome(nome: &str) -> Option<Self> {
        [
            TipoErro::Todo,
            TipoErro::Hint,
            TipoErro::CompileTimeError,
            TipoErro::CheckedModeCompileTimeError,
            TipoErro::StaticWarning,
            TipoErro::SyntacticError,
            TipoErro::Lint,
        ]
        .into_iter()
        .find(|t| t.nome() == nome)
    }

    /// A severidade padrão do tipo (`ErrorType.severity`).
    pub const fn severidade(self) -> Severidade {
        match self {
            TipoErro::Todo | TipoErro::Hint | TipoErro::Lint => Severidade::Info,
            TipoErro::StaticWarning => Severidade::Warning,
            TipoErro::CompileTimeError | TipoErro::CheckedModeCompileTimeError | TipoErro::SyntacticError => {
                Severidade::Error
            }
        }
    }

    /// Erros de compilação e de sintaxe: o `compile-js` recusa o programa.
    pub const fn e_erro_de_compilacao(self) -> bool {
        matches!(
            self,
            TipoErro::CompileTimeError | TipoErro::CheckedModeCompileTimeError | TipoErro::SyntacticError
        )
    }
}

/// Uma entrada da tabela gerada: um `ErrorCode` do analyzer.
#[derive(Debug, PartialEq, Eq)]
pub struct InfoCodigo {
    /// O código como o `dart analyze` o relata: `ErrorCode.name` em minúsculas.
    /// Vários códigos podem compartilhar o nome (`uniqueName` diferente).
    pub nome: &'static str,
    /// `ErrorCode.uniqueName`, por exemplo `CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION`.
    pub unico: &'static str,
    /// Molde da mensagem, com `{0}`, `{1}`...
    pub mensagem: &'static str,
    /// Molde da correção, se houver.
    pub correcao: Option<&'static str>,
    /// `ErrorCode.type`.
    pub tipo: TipoErro,
    /// `ErrorCode.errorSeverity`.
    pub severidade: Severidade,
    /// `hasPublishedDocs`: o JSON do `dart analyze` traz o campo `documentation`.
    pub documentado: bool,
}

impl InfoCodigo {
    /// `ErrorCode.url`: o endereço da documentação publicada.
    pub fn url(&self) -> Option<String> {
        self.documentado.then(|| format!("https://dart.dev/diagnostics/{}", self.nome))
    }
}

/// Um código do analyzer: índice na tabela gerada (2 bytes por diagnóstico).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Codigo(u16);

impl Codigo {
    /// A entrada da tabela.
    pub fn info(self) -> &'static InfoCodigo {
        &codigos_g::TABELA[self.0 as usize]
    }

    /// Todos os códigos da tabela, na ordem gerada.
    pub fn todos() -> impl Iterator<Item = Codigo> {
        (0..codigos_g::TABELA.len() as u16).map(Codigo)
    }

    /// Busca por `uniqueName` (`"CompileTimeErrorCode.UNDEFINED_FUNCTION"`).
    pub fn por_unico(unico: &str) -> Option<Codigo> {
        codigos_g::POR_UNICO
            .binary_search_by(|(u, _)| (*u).cmp(unico))
            .ok()
            .map(|i| Codigo(codigos_g::POR_UNICO[i].1))
    }

    /// Busca pelo código relatado (`"undefined_function"`). Entre os que
    /// compartilham o nome, devolve aquele cujo `uniqueName` é o próprio nome.
    pub fn por_nome(nome: &str) -> Option<Codigo> {
        let mut achado = None;
        for c in Codigo::todos() {
            let info = c.info();
            if info.nome == nome {
                let proprio = info.unico.rsplit('.').next().is_some_and(|u| u.eq_ignore_ascii_case(nome));
                if proprio {
                    return Some(c);
                }
                achado.get_or_insert(c);
            }
        }
        achado
    }
}

impl serde::Serialize for Codigo {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.info().unico)
    }
}

impl<'de> serde::Deserialize<'de> for Codigo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Codigo::por_unico(&s).ok_or_else(|| serde::de::Error::custom(format!("código desconhecido: {s}")))
    }
}

impl serde::Serialize for Severidade {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nome())
    }
}

impl<'de> serde::Deserialize<'de> for Severidade {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Severidade::por_nome(&s).ok_or_else(|| serde::de::Error::custom(format!("severidade desconhecida: {s}")))
    }
}

/// Renderiza um molde como o `formatList` do analyzer: cada `{n}` vira
/// `args[n]`. Sem argumentos, o molde volta intacto (o analyzer também não
/// substitui nada). Um índice sem argumento fica como está.
pub fn formatar(molde: &str, args: &[Box<str>]) -> String {
    if args.is_empty() {
        return molde.to_string();
    }
    let mut out = String::with_capacity(molde.len() + 16);
    let mut resto = molde;
    while let Some(i) = resto.find('{') {
        out.push_str(&resto[..i]);
        let depois = &resto[i + 1..];
        let digitos = depois.bytes().take_while(u8::is_ascii_digit).count();
        if digitos > 0 && depois.as_bytes().get(digitos) == Some(&b'}') {
            let n: usize = depois[..digitos].parse().unwrap_or(usize::MAX);
            match args.get(n) {
                Some(a) => out.push_str(a),
                None => out.push_str(&resto[i..i + digitos + 2]),
            }
            resto = &depois[digitos + 1..];
        } else {
            out.push('{');
            resto = depois;
        }
    }
    out.push_str(resto);
    out
}

/// Erro acompanhado da localização correspondente no código de origem.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    /// Explicação legível do problema. Com [`Diagnostic::code`], é o molde
    /// oficial renderizado com [`Diagnostic::args`].
    pub message: String,
    /// Região do código associada ao problema.
    pub span: Span,
    /// Código do analyzer, quando o diagnóstico tem paridade com um.
    #[serde(default)]
    pub code: Option<Codigo>,
    /// Severidade efetiva (a do código, salvo reconfiguração pelo
    /// `analysis_options.yaml`). Sem código, `Error`.
    #[serde(default)]
    pub severity: Severidade,
    /// Argumentos do molde, na ordem dos `{n}`.
    #[serde(default)]
    pub args: Box<[Box<str>]>,
}

impl Diagnostic {
    /// Cria um diagnóstico sem código, sem alterar a mensagem ou o intervalo informado.
    ///
    /// # Exemplos
    ///
    /// ```
    /// use dartforge_diagnostics::{Diagnostic, Span};
    /// let erro = Diagnostic::new("Nome não encontrado", Span { start: 2, end: 5 });
    /// assert_eq!(erro.span.start, 2);
    /// assert!(erro.code.is_none());
    /// ```
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            code: None,
            severity: Severidade::Error,
            args: Box::default(),
        }
    }

    /// Cria um diagnóstico com código: a mensagem sai do molde oficial.
    ///
    /// ```
    /// use dartforge_diagnostics::{codigos, Diagnostic, Span};
    /// let d = Diagnostic::com_codigo(codigos::compile_time_error::UNDEFINED_FUNCTION, Span { start: 0, end: 1 }, ["h"]);
    /// assert_eq!(d.message, "The function 'h' isn't defined.");
    /// assert_eq!(d.code.unwrap().info().nome, "undefined_function");
    /// ```
    pub fn com_codigo<I, A>(code: Codigo, span: Span, args: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: Into<Box<str>>,
    {
        let args: Box<[Box<str>]> = args.into_iter().map(Into::into).collect();
        let info = code.info();
        Self {
            message: formatar(info.mensagem, &args),
            span,
            code: Some(code),
            severity: info.severidade,
            args,
        }
    }

    /// A correção renderizada do molde oficial, se o código tiver uma.
    pub fn correcao(&self) -> Option<String> {
        let molde = self.code?.info().correcao?;
        Some(formatar(molde, &self.args))
    }
}

impl std::fmt::Display for Diagnostic {
    /// Formata a mensagem com seu intervalo de bytes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(c) = self.code {
            write!(f, "{}: ", c.info().nome)?;
        }
        write!(f, "{} at bytes {}..{}", self.message, self.span.start, self.span.end)
    }
}
impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn formatar_como_format_list() {
        let a: Vec<Box<str>> = vec!["String".into(), "int".into(), "".into()];
        assert_eq!(
            formatar("The argument type '{0}' can't be assigned to the parameter type '{1}'. {2}", &a),
            "The argument type 'String' can't be assigned to the parameter type 'int'. "
        );
        assert_eq!(formatar("sem {0} args", &[]), "sem {0} args");
        assert_eq!(formatar("{x} {9}", &a), "{x} {9}");
    }

    /// A tabela do analyzer 6.11 mais o suplemento 3.13.4 do gerador
    /// (`SUPLEMENTO_3_13`: 4 `CompileTimeErrorCode` e 3 `ParserErrorCode`
    /// de construtores primários).
    #[test]
    fn tabela_tem_as_contagens_do_analyzer_6_11_e_do_suplemento() {
        let conta = |prefixo: &str| Codigo::todos().filter(|c| c.info().unico.starts_with(prefixo)).count();
        assert_eq!(conta("CompileTimeErrorCode."), 542 + 4);
        assert_eq!(conta("StaticWarningCode."), 7);
        assert_eq!(conta("WarningCode."), 144);
        assert_eq!(conta("ParserErrorCode."), 265 + 3);
        for c in Codigo::todos() {
            assert_eq!(Codigo::por_unico(c.info().unico), Some(c));
        }
    }

    #[test]
    fn por_nome_prefere_o_proprio() {
        let c = Codigo::por_nome("abstract_field_initializer").unwrap();
        assert_eq!(c.info().unico, "CompileTimeErrorCode.ABSTRACT_FIELD_INITIALIZER");
        assert_eq!(c.info().severidade, Severidade::Error);
        let w = Codigo::por_nome("unused_local_variable").unwrap();
        assert_eq!(w.info().tipo, TipoErro::StaticWarning);
        assert_eq!(w.info().severidade, Severidade::Warning);
    }

    #[test]
    fn serde_do_codigo_pelo_nome_unico() {
        let d = Diagnostic::com_codigo(codigos::compile_time_error::NON_BOOL_CONDITION, Span { start: 1, end: 2 }, Vec::<String>::new());
        let j = serde_json::to_string(&d).unwrap();
        assert!(j.contains("CompileTimeErrorCode.NON_BOOL_CONDITION"), "{j}");
        let volta: Diagnostic = serde_json::from_str(&j).unwrap();
        assert_eq!(volta, d);
        let antigo: Diagnostic = serde_json::from_str(r#"{"message":"x","span":{"start":0,"end":1}}"#).unwrap();
        assert_eq!(antigo, Diagnostic::new("x", Span { start: 0, end: 1 }));
    }
}
