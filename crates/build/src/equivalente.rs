//! Fábricas de builder reconhecidas pelo que fazem, não pelo nome.
//!
//! Um pacote pode registrar o seu próprio builder que só instancia um
//! builder conhecido com argumentos constantes — o `ngcomponents|scss_builder`
//! é
//!
//! ```dart
//! Builder scssBuilder(BuilderOptions options) =>
//!     SassBuilder(outputExtension: '.scss.css');
//! ```
//!
//! Aí não há o que executar em Dart: é o `SassBuilder` do `sass_builder`
//! com opções fixas, e o gerador nativo dele atende. O reconhecimento é
//! estrito: a fábrica tem de devolver direto a construção de `SassBuilder`
//! importado de `package:sass_builder/sass_builder.dart`, com argumentos
//! nomeados literais que o nativo conhece, e as extensões declaradas no
//! `build.yaml` têm de ser as que o objeto escreve. Qualquer outra forma
//! segue para o executor de builders.
use crate::pacotes::GrafoPacotes;
use crate::valor::{Mapa, Valor};
use dartforge_frontend::Interner;
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind, FunctionBody, StmtKind};

const SASS_BUILDER: &str = "package:sass_builder/sass_builder.dart";

/// Uma fábrica que é, na execução, um builder que o DartForge já tem.
#[derive(Debug, Clone, PartialEq)]
pub struct Equivalente {
    /// A chave do builder conhecido (`sass_builder:sass_builder`).
    pub chave: &'static str,
    /// As opções com que o gerador dele roda — as do construtor, não as do
    /// `build.yaml` (a fábrica as ignora).
    pub opcoes: Mapa,
}

/// A fábrica `fabrica` do `import` (`package:p/x.dart`) é um builder
/// conhecido? `declaradas`: as extensões do `build.yaml`.
pub fn equivalente(
    grafo: &GrafoPacotes,
    import: &str,
    fabrica: &str,
    declaradas: &[(String, Vec<String>)],
) -> Option<Equivalente> {
    let resto = import.strip_prefix("package:")?;
    let (pacote, caminho) = resto.split_once('/')?;
    let arquivo = grafo.no(pacote)?.raiz.join("lib").join(caminho);
    let fonte = std::fs::read_to_string(arquivo).ok()?;
    de_fonte(&fonte, fabrica, declaradas)
}

/// [`equivalente`] sobre o texto da biblioteca.
pub fn de_fonte(
    fonte: &str,
    fabrica: &str,
    declaradas: &[(String, Vec<String>)],
) -> Option<Equivalente> {
    let mut interner = Interner::default();
    let p = dartforge_frontend::parser::parse(fonte, &mut interner);
    if !p.diagnostics.is_empty() {
        return None;
    }
    let (unit, arvore) = (&p.unit, &p.ast);
    let nome = |n: &ast::Name| interner.resolve(n.sym).to_string();
    // O import do `sass_builder` (sem `show`/`hide`, que o nome teria de
    // atravessar) e o prefixo dele.
    let mut prefixos: Vec<Option<String>> = Vec::new();
    for d in &unit.directives {
        if let DirectiveKind::Import {
            uri,
            configurations,
            deferred,
            prefix,
            combinators,
        } = &d.kind
        {
            let alvo = uri.constant_value()?.to_string_lossy();
            if alvo == SASS_BUILDER
                && configurations.is_empty()
                && !deferred
                && combinators.is_empty()
            {
                prefixos.push(prefix.as_ref().map(nome));
            }
        }
    }
    if prefixos.is_empty() {
        return None;
    }
    // Nenhuma declaração local com o nome (sombrearia o importado).
    let mut funcao = None;
    for &id in &unit.declarations {
        let decl = arvore.decl(id);
        let n = match &decl.kind {
            DeclKind::Class(c) => Some(nome(&c.name)),
            DeclKind::Function(f) => arvore.function(*f).name.as_ref().map(nome),
            _ => None,
        };
        if n.as_deref() == Some("SassBuilder") {
            return None;
        }
        if let DeclKind::Function(f) = &decl.kind
            && n.as_deref() == Some(fabrica)
        {
            funcao = Some(*f);
        }
    }
    let f = arvore.function(funcao?);
    if f.parameters.as_ref().map_or(0, |p| p.len()) != 1
        || !matches!(f.modifier, ast::AsyncModifier::None)
    {
        return None;
    }
    let corpo = match f.body {
        FunctionBody::Expression(e) => e,
        FunctionBody::Block(b) => match &arvore.stmt(b).kind {
            StmtKind::Block(s) if s.len() == 1 => match &arvore.stmt(s[0]).kind {
                StmtKind::Return(Some(e)) => *e,
                _ => return None,
            },
            _ => return None,
        },
        _ => return None,
    };
    // `SassBuilder(..)` ou `p.SassBuilder(..)` (a chamada sem `new`), ou a
    // criação explícita.
    let (e_sass, argumentos) = match &arvore.expr(corpo).kind {
        ExprKind::Call { target, arguments } => {
            let e = match &arvore.expr(*target).kind {
                ExprKind::Identifier(n) => nome(n) == "SassBuilder" && prefixos.contains(&None),
                ExprKind::Property {
                    target: t,
                    name,
                    null_aware: false,
                } => {
                    nome(name) == "SassBuilder"
                        && matches!(&arvore.expr(*t).kind,
                            ExprKind::Identifier(p) if prefixos.contains(&Some(nome(p))))
                }
                _ => false,
            };
            (e, arguments)
        }
        ExprKind::InstanceCreation {
            ty,
            constructor: None,
            arguments,
            ..
        } => {
            let e = match &arvore.ty(*ty).kind {
                ast::TypeKind::Named { name, args } if args.is_empty() => match &name[..] {
                    [n] => nome(n) == "SassBuilder" && prefixos.contains(&None),
                    [p, n] => nome(n) == "SassBuilder" && prefixos.contains(&Some(nome(p))),
                    _ => false,
                },
                _ => false,
            };
            (e, arguments)
        }
        _ => return None,
    };
    if !e_sass || !argumentos.type_args.is_empty() {
        return None;
    }
    // `SassBuilder({outputExtension = '.css', outputStyle, generateSourceMaps = false})`.
    let mut extensao = ".css".to_string();
    let mut estilo = Valor::Nulo;
    let mut mapas = false;
    for a in argumentos.args.iter() {
        let n = nome(a.name.as_ref()?);
        let v = &arvore.expr(a.value).kind;
        match (n.as_str(), v) {
            ("outputExtension", ExprKind::String(s)) => {
                extensao = s.constant_value()?.to_string_lossy()
            }
            ("outputStyle", ExprKind::String(s)) => {
                estilo = Valor::Texto(s.constant_value()?.to_string_lossy())
            }
            ("outputStyle", ExprKind::Null) => estilo = Valor::Nulo,
            ("generateSourceMaps", ExprKind::Bool(b)) => mapas = *b,
            _ => return None,
        }
    }
    // O objeto escreve `x<outputExtension>` (e `x.css.map`) para `x.scss` e
    // `x.sass`: as extensões declaradas têm de dizer o mesmo, senão o grafo
    // esperaria outras saídas.
    let mut esperadas = vec![extensao];
    if mapas {
        esperadas.push(".css.map".into());
    }
    if declaradas.is_empty()
        || declaradas.iter().any(|(entrada, saidas)| {
            !matches!(entrada.as_str(), ".scss" | ".sass") || *saidas != esperadas
        })
    {
        return None;
    }
    let mut opcoes = Mapa::default();
    opcoes.inserir(Valor::Texto("outputStyle".into()), estilo);
    opcoes.inserir(Valor::Texto("sourceMaps".into()), Valor::Bool(mapas));
    Some(Equivalente {
        chave: "sass_builder:sass_builder",
        opcoes,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ext(pares: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
        pares
            .iter()
            .map(|(e, s)| (e.to_string(), s.iter().map(|x| x.to_string()).collect()))
            .collect()
    }

    const NGCOMPONENTS: &str = "import 'package:build/build.dart';\n\
        import 'package:sass_builder/sass_builder.dart';\n\
        Builder scssBuilder(BuilderOptions options) =>\n    SassBuilder(outputExtension: '.scss.css');\n";

    #[test]
    fn scss_builder_do_ngcomponents() {
        let e = de_fonte(
            NGCOMPONENTS,
            "scssBuilder",
            &ext(&[(".scss", &[".scss.css"]), (".sass", &[".scss.css"])]),
        )
        .expect("reconhecido");
        assert_eq!(e.chave, "sass_builder:sass_builder");
        assert_eq!(e.opcoes.obter("outputStyle"), Some(&Valor::Nulo));
        assert_eq!(e.opcoes.obter("sourceMaps"), Some(&Valor::Bool(false)));
    }

    #[test]
    fn formas_que_nao_sao_reconhecidas() {
        let decl = ext(&[(".scss", &[".scss.css"])]);
        // Extensão declarada diferente da que o objeto escreve.
        assert!(de_fonte(NGCOMPONENTS, "scssBuilder", &ext(&[(".scss", &[".css"])])).is_none());
        // Outra fábrica.
        assert!(de_fonte(NGCOMPONENTS, "outra", &decl).is_none());
        // Opção vinda da configuração (não constante).
        let cfg = "import 'package:sass_builder/sass_builder.dart';\n\
            Builder b(BuilderOptions o) => SassBuilder(outputExtension: '.scss.css', outputStyle: o.config['x']);\n";
        assert!(de_fonte(cfg, "b", &decl).is_none());
        // Sem o import do `sass_builder`, ou com um `SassBuilder` local.
        let sem = "Builder b(BuilderOptions o) => SassBuilder(outputExtension: '.scss.css');\n";
        assert!(de_fonte(sem, "b", &decl).is_none());
        let local = format!("{NGCOMPONENTS}class SassBuilder {{}}\n");
        assert!(de_fonte(&local, "scssBuilder", &decl).is_none());
        // Prefixado e em bloco, com estilo e mapa.
        let pref = "import 'package:sass_builder/sass_builder.dart' as s;\n\
            Builder b(BuilderOptions o) { return s.SassBuilder(outputExtension: '.x.css', outputStyle: 'compressed', generateSourceMaps: true); }\n";
        let e =
            de_fonte(pref, "b", &ext(&[(".scss", &[".x.css", ".css.map"])])).expect("prefixado");
        assert_eq!(
            e.opcoes.obter("outputStyle"),
            Some(&Valor::Texto("compressed".into()))
        );
        assert_eq!(e.opcoes.obter("sourceMaps"), Some(&Valor::Bool(true)));
    }
}
