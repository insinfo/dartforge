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

/// Qual analyzer é a referência de nomes de código e de textos de um
/// arquivo (docs/ANALYZER-ESPECIFICACAO.md, T2): o 3.6.2, ou o 3.13.4 para
/// os arquivos de versão de linguagem acima da 3.6 e para os que usam
/// sintaxe que o 3.6.2 não conhece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Referencia {
    /// O analyzer 6.11.0, do SDK 3.6.2.
    #[default]
    V3_6,
    /// O analyzer do SDK 3.13.4.
    V3_13,
}

/// Como um código do catálogo 6.11 sai quando a referência é o 3.13.4.
#[derive(Debug, PartialEq, Eq)]
pub struct Variante313 {
    /// A entrada do catálogo 6.11.
    pub de: Codigo,
    /// A entrada com que o 3.13.4 relata (pode ser a mesma); `None` quando
    /// o 3.13.4 não relata.
    pub para: Option<Codigo>,
    /// Os índices, nos argumentos do diagnóstico, dos argumentos do molde de
    /// destino, na ordem dos `{n}` dele.
    pub args: &'static [u8],
    /// Quantos argumentos o diagnóstico tem de trazer para a linha valer.
    pub exige: u8,
}

impl Codigo {
    /// As linhas da tabela de variantes deste código, na ordem em que são
    /// tentadas.
    pub fn variantes_3_13(self) -> &'static [Variante313] {
        let t = &codigos_g::VARIANTES_3_13[..];
        let ini = t.partition_point(|v| v.de < self);
        let fim = t.partition_point(|v| v.de <= self);
        &t[ini..fim]
    }

    /// A variante que vale para um diagnóstico com `argumentos` argumentos:
    /// a primeira que cabe. O emissor que não passa os argumentos que só o
    /// 3.13 usa fica sem variante, e o diagnóstico sai como no 3.6.
    pub fn variante_3_13(self, argumentos: usize) -> Option<&'static Variante313> {
        self.variantes_3_13().iter().find(|v| argumentos >= v.exige as usize)
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

/// Uma mensagem de contexto de um diagnóstico (`DiagnosticMessage` do
/// analyzer: "The first definition of this name", a declaração de um tipo
/// homônimo…), que a saída mostra abaixo dele
/// (docs/ANALYZER-ESPECIFICACAO-INFRA.md §3.3, §5.4, §5.5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Contexto {
    /// O arquivo da mensagem, quando não é o do diagnóstico (caminho
    /// absoluto).
    #[serde(default)]
    pub arquivo: Option<Box<str>>,
    pub span: Span,
    pub mensagem: Box<str>,
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
    /// As mensagens de contexto (`contextMessages`); quase sempre vazio.
    #[serde(default)]
    pub contexto: Vec<Contexto>,
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
            contexto: Vec::new(),
        }
    }

    /// Acrescenta uma mensagem de contexto no mesmo arquivo do diagnóstico.
    pub fn com_contexto(mut self, span: Span, mensagem: impl Into<Box<str>>) -> Self {
        self.contexto.push(Contexto { arquivo: None, span, mensagem: mensagem.into() });
        self
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
            contexto: Vec::new(),
        }
    }

    /// O diagnóstico como o analyzer da referência `r` o relata: no 3.6,
    /// ele mesmo; no 3.13.4, com o código, o texto e os argumentos da
    /// variante ([`Codigo::variante_3_13`]), ou `None` quando o 3.13.4 não
    /// o relata. A severidade acompanha a do código de destino, salvo se já
    /// tiver sido reconfigurada.
    pub fn na_referencia(mut self, r: Referencia) -> Option<Diagnostic> {
        if r == Referencia::V3_6 {
            return Some(self);
        }
        let Some(de) = self.code else { return Some(self) };
        let Some(v) = de.variante_3_13(self.args.len()) else { return Some(self) };
        let para = v.para?;
        let args: Box<[Box<str>]> = v.args.iter().map(|&i| self.args[i as usize].clone()).collect();
        let info = para.info();
        if self.severity == de.info().severidade {
            self.severity = info.severidade;
        }
        self.message = formatar(info.mensagem, &args);
        self.code = Some(para);
        self.args = args;
        Some(self)
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
    /// (`SUPLEMENTO_3_13`): 4 `CompileTimeErrorCode` e 3 `ParserErrorCode`
    /// de construtores primários, 4 `CompileTimeErrorCode` de atalhos de
    /// ponto (com o dos argumentos de tipo no construtor), os 10 códigos que só o 3.13.4 tem (5, 3 e 2 `WarningCode`) e
    /// as 14 formas novas de códigos da 6.11 (12, 1 e 1).
    #[test]
    fn tabela_tem_as_contagens_do_analyzer_6_11_e_do_suplemento() {
        let conta = |prefixo: &str| Codigo::todos().filter(|c| c.info().unico.starts_with(prefixo)).count();
        assert_eq!(conta("CompileTimeErrorCode."), 542 + 4 + 4 + 5 + 12);
        assert_eq!(conta("StaticWarningCode."), 7);
        assert_eq!(conta("WarningCode."), 144 + 2 + 1);
        assert_eq!(conta("ParserErrorCode."), 265 + 3 + 3 + 1);
        for c in Codigo::todos() {
            assert_eq!(Codigo::por_unico(c.info().unico), Some(c));
        }
    }

    /// A tabela de variantes está ordenada, e cada linha cabe nos moldes.
    #[test]
    fn variantes_3_13_ordenadas_e_coerentes() {
        let t = &codigos_g::VARIANTES_3_13;
        assert!(t.windows(2).all(|w| w[0].de <= w[1].de));
        for v in t.iter() {
            assert!(v.args.iter().all(|&i| i < v.exige));
            assert_eq!(v.de.variantes_3_13().first().map(|p| p.de), Some(v.de));
        }
        assert!(codigos::compile_time_error::UNDEFINED_FUNCTION.variantes_3_13().is_empty());
    }

    /// Os casos c02, c03, c04, c05, c13 e c16 de `corpus/especificacao/t2/v`.
    #[test]
    fn na_referencia_troca_nome_texto_e_argumentos() {
        use codigos::{compile_time_error as c, parser as p, warning as w};
        let s = Span { start: 0, end: 1 };
        let igual = Diagnostic::com_codigo(c::ENUM_WITHOUT_CONSTANTS, s, Vec::<String>::new());
        assert_eq!(igual.clone().na_referencia(Referencia::V3_6), Some(igual.clone()));
        let novo = igual.na_referencia(Referencia::V3_13).unwrap();
        assert_eq!(novo.message, "The enum must have at least one enum constant.");
        assert_eq!(novo.correcao().as_deref(), Some("Try declaring an enum constant."));
        assert_eq!(novo.code.unwrap().info().nome, "enum_without_constants");

        let campo = Diagnostic::com_codigo(p::REPRESENTATION_FIELD_MODIFIER, s, ["var"]);
        assert_eq!(campo.message, "Representation fields can't have modifiers.");
        assert_eq!(campo.na_referencia(Referencia::V3_13).unwrap().message, "Representation fields can't have the modifier 'var'.");

        let parametro = Diagnostic::com_codigo(w::UNUSED_ELEMENT_PARAMETER, s, ["p"]);
        assert_eq!(parametro.code.unwrap().info().nome, "unused_element");
        let parametro = parametro.na_referencia(Referencia::V3_13).unwrap();
        assert_eq!(parametro.code.unwrap().info().nome, "unused_element_parameter");
        assert_eq!(parametro.message, "A value for optional parameter 'p' isn't ever given.");
        assert_eq!(parametro.severity, Severidade::Warning);

        let superior = Diagnostic::com_codigo(c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER, s, ["B", "A"]);
        assert!(superior.message.ends_with("of 'B'."));
        assert!(superior.na_referencia(Referencia::V3_13).unwrap().message.ends_with("of 'A'."));
        // Sem o argumento que só o 3.13 usa, o diagnóstico não muda.
        let antigo = Diagnostic::com_codigo(c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER, s, ["B"]);
        assert_eq!(antigo.clone().na_referencia(Referencia::V3_13), Some(antigo));

        let duas = Diagnostic::com_codigo(
            c::AMBIGUOUS_EXTENSION_MEMBER_ACCESS,
            s,
            ["m", "extension 'E1' and extension 'E2'", "extension E1 on int", "extension E2 on int"],
        );
        assert_eq!(
            duas.na_referencia(Referencia::V3_13).unwrap().message,
            "A member named 'm' is defined in 'extension E1 on int' and 'extension E2 on int', and neither is more specific."
        );
        // Três ou mais: o texto do 3.13.4 é o do 3.6.2 (caso c25).
        let tres = Diagnostic::com_codigo(c::AMBIGUOUS_EXTENSION_MEMBER_ACCESS, s, ["m", "extension 'E1', extension 'E2', and extension 'E3'"]);
        assert_eq!(tres.clone().na_referencia(Referencia::V3_13), Some(tres));

        let privado = Diagnostic::com_codigo(c::NON_EXHAUSTIVE_SWITCH_EXPRESSION, s, ["P", "P._b", "P._b", ""]);
        let privado = privado.na_referencia(Referencia::V3_13).unwrap();
        assert_eq!(privado.message, "The enum 'P' isn't exhaustively matched by the switch cases because some of the enum constants are private.");
        assert_eq!(privado.correcao().as_deref(), Some("Try adding a wildcard pattern."));

        let fora = Diagnostic::com_codigo(c::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR, s, Vec::<String>::new());
        assert_eq!(fora.na_referencia(Referencia::V3_13), None);
        let do_parser = Diagnostic::com_codigo(p::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR, s, Vec::<String>::new());
        assert!(do_parser.na_referencia(Referencia::V3_13).is_some());
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
