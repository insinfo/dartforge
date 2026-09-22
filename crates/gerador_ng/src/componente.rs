//! Lê os argumentos de `@Component` e `@Directive` direto da árvore.
//!
//! O compilador oficial faz isso pelo `package:analyzer`, resolvendo cada
//! argumento como constante. Aqui os argumentos que interessam ao gerador são
//! literais escritos na própria anotação (`selector: 'x'`, `templateUrl:
//! 'x.html'`), e os que não são — a lista de `directives`, os `providers` —
//! ficam guardados como texto da fonte, para as etapas que souberem usá-los.
use dartforge_frontend::ast;
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
    /// Nomes escritos em `directives:`, na ordem. Listas embutidas
    /// (`coreDirectives`) entram como o próprio nome e não resolvem para
    /// classe nenhuma — o que basta para o gerador recusar o que depende
    /// delas.
    pub diretivas: Vec<String>,
    /// Parâmetros do construtor, na ordem — o que a visão-hospedeira precisa
    /// para instanciar o componente.
    pub parametros: Vec<Parametro>,
    /// Ganchos de ciclo de vida que a classe implementa.
    pub ganchos: Ganchos,
    /// Anotação de membro que muda a visão e o gerador ainda não trata
    /// (`@HostListener`, `@ViewChild`…), ou argumento de `@Component` fora do
    /// que ele entende. Enquanto houver uma, o arquivo não é nosso: gerar
    /// ignorando isso dá saída **errada**, não incompleta.
    pub nao_entendido: Option<String>,
    /// Cada campo e getter da classe. O emissor precisa disto para escolher
    /// entre `interpolateString`, `interpolate` e `updateTextWithPrimitive`,
    /// que o oficial decide pelo tipo estático da expressão do template e por
    /// ela ser mutável ou não.
    pub membros: std::collections::HashMap<String, Membro>,
    /// Métodos da classe, separados dos campos: um método só pode aparecer
    /// como alvo de chamada (`titulo()`), e o seu tipo é o do retorno. Se
    /// entrassem no mesmo mapa, `{{ titulo }}` (tearoff) seria interpolado
    /// como se fosse o valor de retorno.
    pub metodos: std::collections::HashMap<String, String>,
}

/// Ganchos de ciclo de vida do ngdart implementados pelo componente. Cada um
/// tem o seu lugar fixo no `detectChangesInternal` da visão-hospedeira.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Ganchos {
    pub on_init: bool,
    pub on_destroy: bool,
    pub do_check: bool,
    pub after_content_init: bool,
    pub after_content_checked: bool,
    pub after_view_init: bool,
    pub after_view_checked: bool,
}

impl Ganchos {
    /// Algum gancho põe código na visão-hospedeira?
    pub fn algum(&self) -> bool {
        self.on_init
            || self.on_destroy
            || self.do_check
            || self.after_content_init
            || self.after_content_checked
            || self.after_view_init
            || self.after_view_checked
    }

    /// `firstCheck` só é declarado quando algum gancho o usa.
    pub fn usa_primeira_checagem(&self) -> bool {
        self.on_init || self.after_content_init || self.after_view_init
    }

    /// Há detecção de mudança na hospedeira? (`ngOnDestroy` sozinho só gera
    /// `destroyInternal`.)
    pub fn tem_deteccao(&self) -> bool {
        self.on_init
            || self.do_check
            || self.after_content_init
            || self.after_content_checked
            || self.after_view_init
            || self.after_view_checked
    }
}

/// Um campo ou getter da classe do componente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membro {
    /// Tipo como escrito.
    pub tipo: String,
    /// `final` (ou getter): conta como imutável na regra `isImmutable` do
    /// ngcompiler, que decide se o valor primitivo vai pelo caminho rápido.
    pub imutavel: bool,
}

/// Um parâmetro do construtor do componente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parametro {
    /// Tipo como escrito (`Element`, `RestConfig`, `List<int>`).
    pub tipo: Option<String>,
    pub nome: String,
    pub nomeado: bool,
    /// `@Optional`, `@Inject(...)`, `@Self`… mudam a injeção.
    pub anotado: bool,
}

/// Valor de um argumento nomeado, quando é uma string literal sem
/// interpolação.
fn texto_do_argumento(arvore: &ast::Ast, id: ast::ExprId) -> Option<String> {
    match &arvore.expr(id).kind {
        ast::ExprKind::String(lit) => lit.constant_value().map(|s| s.to_string_lossy()),
        ast::ExprKind::Parenthesized(inner) => texto_do_argumento(arvore, *inner),
        _ => None,
    }
}

/// Lista de strings literais (`styleUrls: ['a.css', 'b.css']`).
fn lista_de_textos(arvore: &ast::Ast, id: ast::ExprId) -> Vec<String> {
    let ast::ExprKind::List { elements, .. } = &arvore.expr(id).kind else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|e| match e {
            ast::CollectionElement::Expression(x) => texto_do_argumento(arvore, *x),
            _ => None,
        })
        .collect()
}

/// Identificadores escritos numa lista literal (`directives: [A, B]`).
fn nomes_da_lista(arvore: &ast::Ast, interner: &Interner, id: ast::ExprId) -> Vec<String> {
    let ast::ExprKind::List { elements, .. } = &arvore.expr(id).kind else { return Vec::new() };
    elements
        .iter()
        .filter_map(|e| match e {
            ast::CollectionElement::Expression(x) => match &arvore.expr(*x).kind {
                ast::ExprKind::Identifier(n) => Some(interner.resolve(n.sym).to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// `ChangeDetectionStrategy.OnPush` — o que muda o estado inicial da visão.
fn e_on_push(arvore: &ast::Ast, fonte: &str, id: ast::ExprId) -> bool {
    let span = arvore.expr(id).span;
    fonte.get(span.start as usize..span.end as usize).is_some_and(|t| t.contains("OnPush"))
}

/// Extrai o `@Component` de uma classe anotada.
pub fn ler_componente(
    arvore: &ast::Ast,
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
                "selector" => c.seletor = texto_do_argumento(arvore, a.value).unwrap_or_default(),
                "template" => c.template = texto_do_argumento(arvore, a.value),
                "templateUrl" => c.template_url = texto_do_argumento(arvore, a.value),
                "styleUrls" => c.style_urls = lista_de_textos(arvore, a.value),
                "styles" => c.styles = lista_de_textos(arvore, a.value),
                "changeDetection" => c.on_push = e_on_push(arvore, fonte, a.value),
                "directives" => c.diretivas = nomes_da_lista(arvore, interner, a.value),
                _ => {}
            }
        }
    }
    c.parametros = parametros_do_construtor(arvore, fonte, interner, classe);
    c.membros = tipos_dos_membros(arvore, fonte, interner, classe);
    c.metodos = tipos_dos_metodos(arvore, fonte, interner, classe);
    c.ganchos = ganchos_da_classe(arvore, fonte, classe);
    c.nao_entendido = o_que_nao_entendemos(arvore, fonte, interner, classe, anotacao);
    c
}

/// Argumentos de `@Component` cujo efeito o gerador conhece.
const ARGUMENTOS_CONHECIDOS: &[&str] = &[
    "selector",
    "template",
    "templateUrl",
    "styleUrls",
    "styles",
    "changeDetection",
    "directives",
    "exports",
];

/// Anotações de membro que mudam a visão gerada.
const ANOTACOES_DE_MEMBRO: &[&str] = &[
    "HostListener",
    "HostBinding",
    "ViewChild",
    "ViewChildren",
    "ContentChild",
    "ContentChildren",
];

/// Interfaces de ciclo de vida implementadas, pelas cláusulas `implements` e
/// `with`. (As do ngrouter — `OnActivate`, `CanDeactivate` — não mexem na
/// visão; `AfterChanges` tampouco aparece no arquivo do próprio componente.)
fn ganchos_da_classe(arvore: &ast::Ast, fonte: &str, classe: &ast::ClassDecl) -> Ganchos {
    let mut g = Ganchos::default();
    for t in classe.implements.iter().chain(classe.with.iter()) {
        let s = arvore.ty(*t).span;
        let texto = fonte.get(s.start as usize..s.end as usize).unwrap_or("");
        match texto.split(['<', '.']).next_back().unwrap_or(texto).trim() {
            "OnInit" => g.on_init = true,
            "OnDestroy" => g.on_destroy = true,
            "DoCheck" => g.do_check = true,
            "AfterContentInit" => g.after_content_init = true,
            "AfterContentChecked" => g.after_content_checked = true,
            "AfterViewInit" => g.after_view_init = true,
            "AfterViewChecked" => g.after_view_checked = true,
            _ => {}
        }
    }
    g
}

/// O que neste componente o gerador ainda não sabe traduzir. Recusar é
/// obrigatório: gerar sem isso produz um arquivo **errado**.
fn o_que_nao_entendemos(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
    anotacao: &ast::Annotation,
) -> Option<String> {
    if let Some(args) = &anotacao.arguments {
        for a in args.args.iter() {
            let nome = match a.name.as_ref() {
                Some(n) => interner.resolve(n.sym),
                None => return Some("argumento posicional em @Component".into()),
            };
            if !ARGUMENTOS_CONHECIDOS.contains(&nome) {
                return Some(format!("@Component(.., {nome}: ..)"));
            }
        }
    }
    for &id in &classe.members {
        let membro = arvore.member(id);
        for a in membro.metadata.iter() {
            let nome = crate::nome_da_anotacao(a, interner);
            if ANOTACOES_DE_MEMBRO.contains(&nome.as_str()) {
                return Some(format!("@{nome}"));
            }
        }
    }
    None
}

/// Parâmetros do construtor gerador (o sem nome). Sem construtor declarado, a
/// classe tem o construtor implícito sem parâmetros.
fn parametros_do_construtor(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Vec<Parametro> {
    for &id in &classe.members {
        let membro = arvore.member(id);
        let ast::MemberKind::Constructor(ctor) = &membro.kind else { continue };
        if ctor.name.is_some() {
            continue; // construtor nomeado não é o que a visão usa
        }
        let campos = tipos_dos_campos(arvore, fonte, interner, classe);
        return ctor
            .parameters
            .iter()
            .map(|p| {
                let nome =
                    p.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default();
                // `C(this.x)` não escreve tipo nenhum: o tipo do parâmetro é o
                // do campo. É assim que quase todo componente ngdart recebe as
                // suas dependências, e sem isto a injeção não sai.
                let tipo = match p.ty {
                    Some(t) => Some(texto_do_tipo(arvore, fonte, t)),
                    None if p.this_ || p.super_ => campos.get(&nome).cloned(),
                    None => None,
                };
                Parametro {
                    tipo,
                    nome,
                    nomeado: matches!(p.kind, ast::ParameterKind::Named),
                    anotado: !p.metadata.is_empty(),
                }
            })
            .collect();
    }
    Vec::new()
}

/// Texto-fonte de um tipo, como escrito.
fn texto_do_tipo(arvore: &ast::Ast, fonte: &str, t: ast::TypeId) -> String {
    let s = arvore.ty(t).span;
    fonte.get(s.start as usize..s.end as usize).unwrap_or("").to_string()
}

/// Tipo de cada campo e getter da classe, para o emissor saber o tipo
/// estático de `{{ nome }}`.
fn tipos_dos_membros(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> std::collections::HashMap<String, Membro> {
    let mut saida = std::collections::HashMap::new();
    for &id in &classe.members {
        match &arvore.member(id).kind {
            ast::MemberKind::Field(lista) => {
                let Some(t) = lista.ty else { continue };
                let tipo = texto_do_tipo(arvore, fonte, t);
                let imutavel = lista.final_ || lista.const_;
                for v in lista.variables.iter() {
                    saida.insert(
                        interner.resolve(v.name.sym).to_string(),
                        Membro { tipo: tipo.clone(), imutavel },
                    );
                }
            }
            ast::MemberKind::Method(f) => {
                let funcao = arvore.function(*f);
                if !matches!(funcao.kind, ast::FunctionKind::Getter) {
                    continue;
                }
                let (Some(nome), Some(t)) = (funcao.name, funcao.return_type) else { continue };
                saida.insert(
                    interner.resolve(nome.sym).to_string(),
                    Membro { tipo: texto_do_tipo(arvore, fonte, t), imutavel: true },
                );
            }
            _ => {}
        }
    }
    saida
}

/// Retorno de cada método da classe.
fn tipos_dos_metodos(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> std::collections::HashMap<String, String> {
    let mut saida = std::collections::HashMap::new();
    for &id in &classe.members {
        let ast::MemberKind::Method(f) = &arvore.member(id).kind else { continue };
        let funcao = arvore.function(*f);
        if !matches!(funcao.kind, ast::FunctionKind::Function) {
            continue;
        }
        let Some(nome) = funcao.name else { continue };
        let tipo = match funcao.return_type {
            Some(t) => texto_do_tipo(arvore, fonte, t),
            None => "dynamic".to_string(),
        };
        saida.insert(interner.resolve(nome.sym).to_string(), tipo);
    }
    saida
}

/// Tipo de cada campo da classe, para resolver os parâmetros `this.x`.
fn tipos_dos_campos(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> std::collections::HashMap<String, String> {
    let mut saida = std::collections::HashMap::new();
    for &id in &classe.members {
        let ast::MemberKind::Field(lista) = &arvore.member(id).kind else { continue };
        let Some(t) = lista.ty else { continue };
        let tipo = texto_do_tipo(arvore, fonte, t);
        for v in lista.variables.iter() {
            saida.insert(interner.resolve(v.name.sym).to_string(), tipo.clone());
        }
    }
    saida
}
