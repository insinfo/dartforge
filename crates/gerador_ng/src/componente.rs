//! Lê os argumentos de `@Component` e `@Directive` direto da árvore.
//!
//! O compilador oficial faz isso pelo `package:analyzer`, resolvendo cada
//! argumento como constante. Aqui os argumentos que interessam ao gerador são
//! literais escritos na própria anotação (`selector: 'x'`, `templateUrl:
//! 'x.html'`), e os que não são — a lista de `directives`, os `providers` —
//! ficam guardados como texto da fonte, para as etapas que souberem usá-los.
use dartforge_frontend::ast;
use dartforge_frontend::parser::Parsed;
use dartforge_intern::Interner;

/// `@Component(...)` de uma classe, como está escrito.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Componente {
    pub classe: String,
    pub seletor: String,
    /// `template: '...'` quando o template está na própria anotação.
    pub template: Option<String>,
    /// `templateUrl: 'x.html'`.
    pub template_url: Option<String>,
    pub style_urls: Vec<String>,
    /// `styles: ['...']` — folhas escritas na anotação.
    pub styles: Vec<String>,
    /// `changeDetection: ChangeDetectionStrategy.OnPush`.
    pub on_push: bool,
    /// Texto-fonte de `directives:`, ainda sem resolver.
    pub diretivas: Option<String>,
    /// Parâmetros do construtor, na ordem — o que a visão-hospedeira precisa
    /// para instanciar o componente.
    pub parametros: Vec<Parametro>,
}

/// Um parâmetro do construtor do componente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parametro {
    /// Tipo como escrito (`Element`, `RestConfig`, `List<int>`).
    pub tipo: Option<String>,
    pub nome: String,
    pub nomeado: bool,
}

/// Valor de um argumento nomeado, quando é uma string literal sem
/// interpolação.
fn texto_do_argumento(analisada: &Parsed, id: ast::ExprId) -> Option<String> {
    match &analisada.ast.expr(id).kind {
        ast::ExprKind::String(lit) => lit.constant_value().map(|s| s.to_string_lossy()),
        ast::ExprKind::Parenthesized(inner) => texto_do_argumento(analisada, *inner),
        _ => None,
    }
}

/// Lista de strings literais (`styleUrls: ['a.css', 'b.css']`).
fn lista_de_textos(analisada: &Parsed, id: ast::ExprId) -> Vec<String> {
    let ast::ExprKind::List { elements, .. } = &analisada.ast.expr(id).kind else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|e| match e {
            ast::CollectionElement::Expression(x) => texto_do_argumento(analisada, *x),
            _ => None,
        })
        .collect()
}

/// `ChangeDetectionStrategy.OnPush` — o que muda o estado inicial da visão.
fn e_on_push(analisada: &Parsed, fonte: &str, id: ast::ExprId) -> bool {
    let span = analisada.ast.expr(id).span;
    fonte.get(span.start as usize..span.end as usize).is_some_and(|t| t.contains("OnPush"))
}

/// Extrai o `@Component` de uma classe anotada.
pub fn ler_componente(
    analisada: &Parsed,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
) -> Componente {
    let mut c = Componente {
        classe: interner.resolve(classe.name.sym).to_string(),
        ..Default::default()
    };
    if let Some(args) = &anotacao.arguments {
        for a in args.args.iter() {
            let Some(nome) = a.name.as_ref().map(|n| interner.resolve(n.sym)) else { continue };
            match nome {
                "selector" => c.seletor = texto_do_argumento(analisada, a.value).unwrap_or_default(),
                "template" => c.template = texto_do_argumento(analisada, a.value),
                "templateUrl" => c.template_url = texto_do_argumento(analisada, a.value),
                "styleUrls" => c.style_urls = lista_de_textos(analisada, a.value),
                "styles" => c.styles = lista_de_textos(analisada, a.value),
                "changeDetection" => c.on_push = e_on_push(analisada, fonte, a.value),
                "directives" => {
                    let s = analisada.ast.expr(a.value).span;
                    c.diretivas = fonte.get(s.start as usize..s.end as usize).map(str::to_string);
                }
                _ => {}
            }
        }
    }
    c.parametros = parametros_do_construtor(analisada, fonte, interner, classe);
    c
}

/// Parâmetros do construtor gerador (o sem nome). Sem construtor declarado, a
/// classe tem o construtor implícito sem parâmetros.
fn parametros_do_construtor(
    analisada: &Parsed,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Vec<Parametro> {
    for &id in &classe.members {
        let membro = analisada.ast.member(id);
        let ast::MemberKind::Constructor(ctor) = &membro.kind else { continue };
        if ctor.name.is_some() {
            continue; // construtor nomeado não é o que a visão usa
        }
        return ctor
            .parameters
            .iter()
            .map(|p| Parametro {
                tipo: p.ty.map(|t| {
                    let s = analisada.ast.ty(t).span;
                    fonte.get(s.start as usize..s.end as usize).unwrap_or("").to_string()
                }),
                nome: p.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(),
                nomeado: matches!(p.kind, ast::ParameterKind::Named),
            })
            .collect();
    }
    Vec::new()
}
