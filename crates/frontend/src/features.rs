//! Versão de linguagem e recursos ligados por biblioteca.
//!
//! Contrato em `docs/VERSOES-LINGUAGEM.md`. Cada biblioteca Dart tem uma
//! **versão de linguagem** (`accepted/2.8/language-versioning`): o marcador
//! `// @dart = x.y` da própria biblioteca, senão o `languageVersion` do pacote
//! no `package_config.json`, senão a versão corrente da ferramenta. A versão
//! decide quais recursos da linguagem valem ali — como o `LibraryFeatures`
//! do CFE (`front_end/lib/src/api_prototype/experimental_flags_generated.dart`).
//!
//! Custo: a versão é resolvida **uma vez** por biblioteca, no carregador,
//! antes do parse; o parser e as fases seguintes só consultam um conjunto de
//! bits ([`LibraryFeatures::tem`]). Nenhuma fase refaz a conta por nó.

use dartforge_diagnostics::Span;

/// Versão de linguagem `maior.menor` (a de patch não existe para a linguagem).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct LanguageVersion {
    pub major: u16,
    pub minor: u16,
}

impl LanguageVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        LanguageVersion { major, minor }
    }

    /// A menor versão que o Dart 3 aceita (`kernel_target.dart`,
    /// `leastSupportedVersion`): abaixo dela não há null safety.
    pub const MINIMA: LanguageVersion = LanguageVersion::new(2, 12);

    /// O **piso** do DartForge e a versão da plataforma: as bibliotecas
    /// `dart:*` são as do SDK 3.6.2 (`docs/VERSOES-LINGUAGEM.md`, D1) e são
    /// sempre analisadas nesta versão.
    pub const PISO: LanguageVersion = LanguageVersion::new(3, 6);

    /// A versão corrente da ferramenta (D2): a de um arquivo sem marcador e
    /// fora de qualquer pacote, e o teto dos marcadores aceitos.
    pub const ATUAL: LanguageVersion = LanguageVersion::new(3, 13);

    /// Lê `x.y` (o formato do `package_config.json` e de `--versao-linguagem`).
    /// Espaços em volta são tolerados; qualquer outra coisa é `None`.
    pub fn parse(texto: &str) -> Option<LanguageVersion> {
        let (a, b) = texto.trim().split_once('.')?;
        if a.is_empty()
            || b.is_empty()
            || !a.bytes().all(|c| c.is_ascii_digit())
            || !b.bytes().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        Some(LanguageVersion::new(a.parse().ok()?, b.parse().ok()?))
    }
}

/// A versão corrente ([`LanguageVersion::ATUAL`]).
impl Default for LanguageVersion {
    fn default() -> Self {
        LanguageVersion::ATUAL
    }
}

impl std::fmt::Display for LanguageVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Recurso de linguagem cuja presença depende da versão (ou de experimento).
///
/// Só entram aqui os recursos posteriores ao piso 3.6 e os experimentos que o
/// DartForge acompanha. Os nomes são os de `tools/experimental_features.yaml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Feature {
    /// 3.7 — `_` local/parâmetro/parâmetro de tipo não liga nome.
    WildcardVariables,
    /// 3.7 — inferência que combina restrições com os bounds declarados.
    InferenceUsingBounds,
    /// 3.8 — `?e` e `?k: ?v` em literais de coleção.
    NullAwareElements,
    /// 3.9 — promoção, alcançabilidade e atribuição definitiva supõem null safety.
    SoundFlowAnalysis,
    /// 3.9 — deixa de ser erro o getter e o setter de tipos incompatíveis.
    GetterSetterError,
    /// 3.10 — `.id`, `.new`, `const .id(…)` pelo tipo de contexto.
    DotShorthands,
    /// 3.12 — `{this._x}` nomeado, chamado como `x:`.
    PrivateNamedParameters,
    /// 3.13 — construtores primários, `new`/`factory` sem o nome da classe,
    /// corpo `;`; `var`/`final` só em parâmetro declarante.
    PrimaryConstructors,
    /// Experimento (sem `enabledIn`): augmentations escritas à mão.
    Augmentations,
    /// Experimento: *part* com imports próprios.
    EnhancedParts,
    /// Experimento: macros.
    Macros,
}

impl Feature {
    /// Todos, na ordem da declaração (o índice é o bit em [`LibraryFeatures`]).
    pub const TODOS: [Feature; 11] = [
        Feature::WildcardVariables,
        Feature::InferenceUsingBounds,
        Feature::NullAwareElements,
        Feature::SoundFlowAnalysis,
        Feature::GetterSetterError,
        Feature::DotShorthands,
        Feature::PrivateNamedParameters,
        Feature::PrimaryConstructors,
        Feature::Augmentations,
        Feature::EnhancedParts,
        Feature::Macros,
    ];

    /// Nome do experimento em `tools/experimental_features.yaml` (é também o
    /// valor de `--enable-experiment`).
    pub const fn nome(self) -> &'static str {
        match self {
            Feature::WildcardVariables => "wildcard-variables",
            Feature::InferenceUsingBounds => "inference-using-bounds",
            Feature::NullAwareElements => "null-aware-elements",
            Feature::SoundFlowAnalysis => "sound-flow-analysis",
            Feature::GetterSetterError => "getter-setter-error",
            Feature::DotShorthands => "dot-shorthands",
            Feature::PrivateNamedParameters => "private-named-parameters",
            Feature::PrimaryConstructors => "primary-constructors",
            Feature::Augmentations => "augmentations",
            Feature::EnhancedParts => "enhanced-parts",
            Feature::Macros => "macros",
        }
    }

    /// `enabledIn` do `experimental_features.yaml`: a versão de linguagem a
    /// partir da qual o recurso vale sem sinal nenhum. `None` = experimento.
    pub const fn habilitado_em(self) -> Option<LanguageVersion> {
        match self {
            Feature::WildcardVariables | Feature::InferenceUsingBounds => {
                Some(LanguageVersion::new(3, 7))
            }
            Feature::NullAwareElements => Some(LanguageVersion::new(3, 8)),
            Feature::SoundFlowAnalysis | Feature::GetterSetterError => {
                Some(LanguageVersion::new(3, 9))
            }
            Feature::DotShorthands => Some(LanguageVersion::new(3, 10)),
            Feature::PrivateNamedParameters => Some(LanguageVersion::new(3, 12)),
            Feature::PrimaryConstructors => Some(LanguageVersion::new(3, 13)),
            Feature::Augmentations | Feature::EnhancedParts | Feature::Macros => None,
        }
    }

    /// O recurso pelo nome do experimento.
    pub fn do_nome(nome: &str) -> Option<Feature> {
        Feature::TODOS.into_iter().find(|f| f.nome() == nome)
    }

    const fn bit(self) -> u32 {
        1 << (self as u8)
    }
}

/// Os recursos ligados numa biblioteca: a versão e um conjunto de bits.
///
/// `Copy` e de 8 bytes: o parser e as fases seguintes guardam o valor, não
/// uma referência, e a consulta é um `&` de bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LibraryFeatures {
    versao: LanguageVersion,
    bits: u32,
}

impl LibraryFeatures {
    /// Os recursos de uma biblioteca na versão `versao`, mais os
    /// `experimentos` pedidos.
    ///
    /// Quem chama decide se os experimentos valem para a biblioteca: pela
    /// regra da proposta de versionamento (`:159`), só para as que **não**
    /// têm marcador e cuja versão padrão é a corrente. Ver
    /// [`LibraryFeatures::para_biblioteca`].
    pub fn new(versao: LanguageVersion, experimentos: &[Feature]) -> Self {
        let mut bits = 0;
        for f in Feature::TODOS {
            if f.habilitado_em().is_some_and(|v| versao >= v) {
                bits |= f.bit();
            }
        }
        for f in experimentos {
            bits |= f.bit();
        }
        LibraryFeatures { versao, bits }
    }

    /// A regra inteira de uma biblioteca: `marcador` é a versão do
    /// `// @dart = x.y`, se houver; `padrao` a do pacote (ou a corrente, fora
    /// de pacote); `corrente` a da ferramenta; `experimentos`, os pedidos por
    /// `--enable-experiment`, que só valem sem marcador e com o padrão igual
    /// à corrente.
    pub fn para_biblioteca(
        marcador: Option<LanguageVersion>,
        padrao: LanguageVersion,
        corrente: LanguageVersion,
        experimentos: &[Feature],
    ) -> Self {
        match marcador {
            Some(v) => LibraryFeatures::new(v, &[]),
            None if padrao == corrente => LibraryFeatures::new(padrao, experimentos),
            None => LibraryFeatures::new(padrao, &[]),
        }
    }

    /// A versão corrente da ferramenta, sem experimentos.
    pub fn atual() -> Self {
        LibraryFeatures::new(LanguageVersion::ATUAL, &[])
    }

    /// O piso 3.6: bibliotecas `dart:*` e o corpus 3.6.
    pub fn piso() -> Self {
        LibraryFeatures::new(LanguageVersion::PISO, &[])
    }

    #[inline]
    pub fn tem(self, f: Feature) -> bool {
        self.bits & f.bit() != 0
    }

    #[inline]
    pub fn versao(self) -> LanguageVersion {
        self.versao
    }

    /// Mensagem do diagnóstico de recurso desligado.
    pub fn mensagem_desligado(self, f: Feature) -> String {
        match f.habilitado_em() {
            Some(v) => format!(
                "o recurso '{}' exige a versão de linguagem {v} ou superior (esta biblioteca está na {})",
                f.nome(),
                self.versao
            ),
            None => format!(
                "o recurso '{}' é experimental: exige --enable-experiment={} (esta biblioteca está na {})",
                f.nome(),
                f.nome(),
                self.versao
            ),
        }
    }
}

impl Default for LibraryFeatures {
    fn default() -> Self {
        LibraryFeatures::atual()
    }
}

/// Procura o marcador `// @dart = x.y` no cabeçalho da fonte: antes do
/// primeiro token, depois de um `#!` e de outros comentários.
///
/// Segue o *scanner* do CFE
/// (`_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart`,
/// `tokenizeLanguageVersionOrSingleLineComment`): só `//` (não `///`),
/// espaços (não tabulações) em volta de `@dart`, `=` e da versão, e nada
/// depois da versão além de espaços até o fim da linha. Um marcador dentro de
/// comentário de bloco não conta, e só o primeiro vale. Varredura pura sobre
/// bytes, sem alocar: custa o tamanho do cabeçalho de comentários.
pub fn marcador_versao(fonte: &str) -> Option<(LanguageVersion, Span)> {
    let b = fonte.as_bytes();
    let mut i = 0;
    // BOM UTF-8.
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        i = 3;
    }
    // `#!` só na primeira linha.
    if b[i..].starts_with(b"#!") {
        while i < b.len() && b[i] != b'\n' {
            i += 1;
        }
    }
    loop {
        while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\r' | b'\n' | 0x0C) {
            i += 1;
        }
        if b[i..].starts_with(b"//") {
            let inicio = i;
            let mut fim = i;
            while fim < b.len() && b[fim] != b'\n' && b[fim] != b'\r' {
                fim += 1;
            }
            if let Some(v) = versao_do_comentario(&b[inicio..fim]) {
                return Some((
                    v,
                    Span {
                        start: inicio,
                        end: fim,
                    },
                ));
            }
            i = fim;
        } else if b[i..].starts_with(b"/*") {
            // Comentário de bloco, aninhado como no lexer.
            let mut prof = 0usize;
            loop {
                if i >= b.len() {
                    return None;
                }
                if b[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else {
            return None;
        }
    }
}

/// `// @dart = x.y` (sem o `\n`), com a gramática do CFE; senão `None`.
fn versao_do_comentario(linha: &[u8]) -> Option<LanguageVersion> {
    let mut r = linha.strip_prefix(b"//")?;
    if r.first() == Some(&b'/') {
        return None; // `///` é documentação
    }
    let espacos = |r: &mut &[u8]| {
        while r.first() == Some(&b' ') {
            *r = &r[1..];
        }
    };
    espacos(&mut r);
    r = r.strip_prefix(b"@dart")?;
    espacos(&mut r);
    r = r.strip_prefix(b"=")?;
    espacos(&mut r);
    let digitos = |r: &mut &[u8]| -> Option<u16> {
        let n = r.iter().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 {
            return None;
        }
        let v = std::str::from_utf8(&r[..n]).ok()?.parse().ok();
        *r = &r[n..];
        v
    };
    let major = digitos(&mut r)?;
    r = r.strip_prefix(b".")?;
    let minor = digitos(&mut r)?;
    espacos(&mut r);
    r.is_empty().then_some(LanguageVersion::new(major, minor))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn v(a: u16, b: u16) -> LanguageVersion {
        LanguageVersion::new(a, b)
    }

    #[test]
    fn marcador_simples_e_com_espacos() {
        assert_eq!(
            marcador_versao("// @dart=3.6\nvoid main() {}").map(|m| m.0),
            Some(v(3, 6))
        );
        assert_eq!(
            marcador_versao("//@dart = 2.12   \nx").map(|m| m.0),
            Some(v(2, 12))
        );
        assert_eq!(
            marcador_versao("// @dart = 3.13").map(|m| m.0),
            Some(v(3, 13))
        );
        let (_, span) = marcador_versao("\n\n// @dart=3.7\r\nmain(){}").unwrap();
        assert_eq!(span, Span { start: 2, end: 14 });
    }

    #[test]
    fn marcador_depois_de_script_e_comentarios() {
        let f = "#!/usr/bin/env dart\n// licença\n/* bloco\n// @dart=2.1 não conta */\n// @dart=3.8\nimport 'x.dart';";
        assert_eq!(marcador_versao(f).map(|m| m.0), Some(v(3, 8)));
        assert_eq!(
            marcador_versao("\u{FEFF}// @dart=3.7\n").map(|m| m.0),
            Some(v(3, 7))
        );
    }

    #[test]
    fn so_o_primeiro_vale_e_so_antes_do_primeiro_token() {
        assert_eq!(
            marcador_versao("// @dart=3.6\n// @dart=3.13\n").map(|m| m.0),
            Some(v(3, 6))
        );
        assert_eq!(marcador_versao("library a;\n// @dart=3.6\n"), None);
    }

    #[test]
    fn formas_que_nao_sao_marcador() {
        for f in [
            "/// @dart=3.6\n",      // documentação
            "// @dart=3.6 extra\n", // lixo depois da versão
            "//\t@dart=3.6\n",      // tabulação
            "// @dart=3\n",         // sem menor
            "// @dart=.6\n",        // sem maior
            "// @ dart=3.6\n",      // espaço dentro de `@dart`
            "/* // @dart=3.6 */ x", // dentro de bloco
            "// @dartx=3.6\n",
        ] {
            assert_eq!(marcador_versao(f), None, "{f:?}");
        }
        // Um comentário comum antes não impede o marcador seguinte.
        assert_eq!(
            marcador_versao("// @dart=3.6 extra\n// @dart=3.7\n").map(|m| m.0),
            Some(v(3, 7))
        );
    }

    #[test]
    fn recursos_pela_versao() {
        let f36 = LibraryFeatures::new(v(3, 6), &[]);
        assert!(Feature::TODOS.iter().all(|f| !f36.tem(*f)));
        let f310 = LibraryFeatures::new(v(3, 10), &[]);
        assert!(
            f310.tem(Feature::WildcardVariables)
                && f310.tem(Feature::NullAwareElements)
                && f310.tem(Feature::DotShorthands)
        );
        assert!(
            !f310.tem(Feature::PrivateNamedParameters) && !f310.tem(Feature::PrimaryConstructors)
        );
        let atual = LibraryFeatures::atual();
        assert!(atual.tem(Feature::PrimaryConstructors) && !atual.tem(Feature::Macros));
        assert!(LibraryFeatures::new(v(3, 6), &[Feature::Macros]).tem(Feature::Macros));
        assert_eq!(LanguageVersion::parse(" 3.13 "), Some(v(3, 13)));
        assert_eq!(LanguageVersion::parse("3"), None);
        assert_eq!(LanguageVersion::parse("3.x"), None);
        assert_eq!(
            Feature::do_nome("dot-shorthands"),
            Some(Feature::DotShorthands)
        );
    }

    #[test]
    fn experimentos_so_sem_marcador_e_na_versao_corrente() {
        let atual = LanguageVersion::ATUAL;
        let exp = [Feature::Macros];
        assert!(LibraryFeatures::para_biblioteca(None, atual, atual, &exp).tem(Feature::Macros));
        assert!(
            !LibraryFeatures::para_biblioteca(Some(atual), atual, atual, &exp).tem(Feature::Macros)
        );
        assert!(!LibraryFeatures::para_biblioteca(None, v(3, 6), atual, &exp).tem(Feature::Macros));
    }

    /// A tabela transcrita confere com o `experimental_features.yaml` do SDK,
    /// quando o clone de referência existe (`references/` fica fora do git).
    #[test]
    fn tabela_confere_com_o_yaml_do_sdk() {
        let yaml = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../references/dart-sdk/tools/experimental_features.yaml");
        let Ok(texto) = std::fs::read_to_string(&yaml) else {
            eprintln!("{} ausente; conferência pulada", yaml.display());
            return;
        };
        for f in Feature::TODOS {
            let cab = format!("\n  {}:\n", f.nome());
            let inicio = texto
                .find(&cab)
                .unwrap_or_else(|| panic!("{} fora do yaml", f.nome()))
                + cab.len();
            let bloco: String = texto[inicio..]
                .lines()
                .take_while(|l| l.starts_with("    ") || l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let habilitado = bloco
                .lines()
                .find_map(|l| l.trim().strip_prefix("enabledIn:"))
                .map(|s| {
                    let s = s.trim().trim_matches('\'');
                    let mut p = s.split('.');
                    LanguageVersion::new(
                        p.next().unwrap().parse().unwrap(),
                        p.next().unwrap().parse().unwrap(),
                    )
                });
            assert_eq!(habilitado, f.habilitado_em(), "{}", f.nome());
        }
    }
}
