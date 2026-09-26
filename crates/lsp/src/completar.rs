//! `textDocument/completion`: membros pelo tipo estático do receptor, nomes
//! do escopo, palavras-chave e argumentos nomeados.
//!
//! O ponto de digitação quase nunca analisa: `a.` sem nome, `a.ca` sem `;`.
//! O parser recupera por comando (pula até o `;` ou a `}` do bloco), então o
//! comando incompleto some da árvore. Antes de analisar, o nome sob o cursor
//! é trocado por um identificador sentinela e, se preciso, um fecho curto
//! (`;`, `);` …) é acrescentado depois dele; vale a variante que põe o
//! sentinela numa expressão com o menor número de diagnósticos. O programa é
//! então carregado com esse texto e os corpos da biblioteca são inferidos
//! pela inferência comum (`crates/types`): o tipo do receptor é o
//! `get_type` do alvo, e o escopo léxico é capturado pela sonda da inferência
//! no identificador sentinela.
//!
//! A ordem dos itens é estável: grupo (argumentos nomeados, locais, membros
//! da classe, declarações da biblioteca, importados, prefixos, palavras-chave)
//! e depois o nome. O filtro é pelo prefixo digitado, sem distinguir
//! maiúsculas de minúsculas.

use crate::DocumentStore;
use crate::consulta::Consulta;
use crate::semantica::AnalisadorSemantico;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, LibraryId, Namespace,
};
use dartforge_frontend::LibraryFeatures;
use dartforge_frontend::ast::{self, ExprId, ExprKind, StmtKind};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::HashSet;

/// Identificador que ocupa o lugar do nome sob o cursor na análise.
const SENTINELA: &str = "dartforge__completar";

/// Fechos tentados depois do sentinela, na ordem de preferência.
const FECHOS: &[&str] = &["", ";", ")", ");", "))", "));", ")));", "]", "];", "});"];

/// `CompletionItemKind` do LSP.
pub(crate) mod especie {
    pub const METODO: u32 = 2;
    pub const FUNCAO: u32 = 3;
    pub const CONSTRUTOR: u32 = 4;
    pub const CAMPO: u32 = 5;
    pub const VARIAVEL: u32 = 6;
    pub const CLASSE: u32 = 7;
    pub const MODULO: u32 = 9;
    pub const PROPRIEDADE: u32 = 10;
    pub const ENUM: u32 = 13;
    pub const PALAVRA_CHAVE: u32 = 14;
    pub const MEMBRO_DE_ENUM: u32 = 20;
    pub const PARAMETRO_DE_TIPO: u32 = 25;
}

/// Um item oferecido ao editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemCompletar {
    /// O que a lista mostra (`met(…)`, `nome: `).
    pub rotulo: String,
    /// `CompletionItemKind`.
    pub especie: u32,
    /// Tipo ou assinatura.
    pub detalhe: Option<String>,
    /// Texto que substitui o prefixo digitado.
    pub inserir: String,
    /// Grupo de ordenação (menor vem antes).
    grupo: u8,
}

/// Resultado do completar: o intervalo do prefixo (bytes) e os itens em ordem.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completar {
    pub inicio: usize,
    pub fim: usize,
    pub itens: Vec<ItemCompletar>,
}

mod grupo {
    pub const NOMEADO: u8 = 0;
    pub const LOCAL: u8 = 1;
    pub const MEMBRO: u8 = 2;
    pub const BIBLIOTECA: u8 = 3;
    pub const IMPORTADO: u8 = 4;
    pub const PREFIXO: u8 = 5;
    pub const PALAVRA: u8 = 6;
}

const PALAVRAS_DE_EXPRESSAO: &[&str] = &["const", "false", "new", "null", "true"];
const PALAVRAS_DE_COMANDO: &[&str] = &[
    "assert", "break", "const", "continue", "do", "false", "final", "for", "if", "late", "new",
    "null", "return", "switch", "throw", "true", "try", "var", "void", "while",
];
const PALAVRAS_DE_TOPO: &[&str] = &[
    "abstract",
    "base",
    "class",
    "const",
    "enum",
    "export",
    "extension",
    "external",
    "final",
    "import",
    "interface",
    "late",
    "library",
    "mixin",
    "part",
    "sealed",
    "typedef",
    "var",
    "void",
];
const PALAVRAS_DE_MEMBRO: &[&str] = &[
    "abstract",
    "const",
    "covariant",
    "external",
    "factory",
    "final",
    "get",
    "late",
    "operator",
    "set",
    "static",
    "var",
    "void",
];

fn eh_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Completa na posição `offset` (bytes) de `texto`, o conteúdo vigente de `uri`.
///
/// `None` quando o arquivo não pode ser carregado (sem SDK, URI que não é de
/// arquivo). Dentro de comentário, de texto de string ou de número, a lista
/// é vazia.
pub(crate) fn completar(
    semantico: &AnalisadorSemantico,
    documentos: &DocumentStore,
    uri: &str,
    texto: &str,
    offset: usize,
    features: LibraryFeatures,
) -> Option<Completar> {
    let bytes = texto.as_bytes();
    let offset = offset.min(texto.len());
    let mut inicio = offset;
    while inicio > 0 && eh_ident(bytes[inicio - 1]) {
        inicio -= 1;
    }
    let mut fim = offset;
    while fim < bytes.len() && eh_ident(bytes[fim]) {
        fim += 1;
    }
    let vazio = Completar {
        inicio,
        fim: offset,
        itens: Vec::new(),
    };
    if bytes.get(inicio).is_some_and(u8::is_ascii_digit) && inicio < offset
        || fora_de_codigo(texto, offset)
    {
        return Some(vazio);
    }
    let prefixo = texto[inicio..offset].to_ascii_lowercase();
    let preparado = preparar(texto, inicio, fim, features);
    let texto_analisado = preparado.as_deref().unwrap_or(texto);
    let (programa, nomes, unidade) = semantico.carregar(uri, texto_analisado, Some(documentos))?;
    let biblioteca = programa.unit(unidade).library;
    let sonda = preparado.as_ref().map(|_| (unidade, inicio));
    let mut consulta = Consulta::inferir(programa, nomes, &[biblioteca], false, sonda);
    let mut coletor = Coletor {
        itens: Vec::new(),
        biblioteca,
    };

    let sentinela = preparado
        .as_ref()
        .and_then(|_| achar_sentinela(&consulta.programa.unit(unidade).ast, inicio));
    match sentinela {
        Some((expr, Some(alvo))) => coletor.membros_do_alvo(&mut consulta, unidade, expr, alvo),
        Some((expr, None)) => {
            coletor.argumentos_nomeados(&consulta, unidade, expr);
            coletor.escopo(&mut consulta);
            let ast = &consulta.programa.unit(unidade).ast;
            let comando = ast
                .stmts
                .iter()
                .any(|s| matches!(s.kind, StmtKind::Expression(e) if e == expr));
            let estatico = consulta.escopo.as_ref().is_none_or(|e| e.estatico);
            let mut palavras: Vec<&str> = if comando {
                PALAVRAS_DE_COMANDO.to_vec()
            } else {
                PALAVRAS_DE_EXPRESSAO.to_vec()
            };
            if !estatico {
                palavras.extend(["this", "super"]);
            }
            coletor.palavras(&palavras);
        }
        None => {
            coletor.biblioteca(&consulta);
            let ast = &consulta.programa.unit(unidade).ast;
            let em_tipo = ast.decls.iter().any(|d| {
                d.span.start < inicio
                    && inicio < d.span.end
                    && matches!(
                        d.kind,
                        ast::DeclKind::Class(_)
                            | ast::DeclKind::Mixin(_)
                            | ast::DeclKind::Enum(_)
                            | ast::DeclKind::Extension(_)
                            | ast::DeclKind::ExtensionType(_)
                    )
            });
            let em_corpo = ast.functions.iter().any(|f| {
                let corpo = match f.body {
                    ast::FunctionBody::Block(s) => ast.stmt(s).span,
                    ast::FunctionBody::Expression(e) => ast.expr(e).span,
                    _ => return false,
                };
                corpo.start < inicio && inicio <= corpo.end
            });
            let palavras = if em_corpo {
                PALAVRAS_DE_COMANDO
            } else if em_tipo {
                PALAVRAS_DE_MEMBRO
            } else {
                PALAVRAS_DE_TOPO
            };
            coletor.palavras(palavras);
        }
    }

    let mut itens = coletor.itens;
    itens
        .retain(|i| i.inserir != SENTINELA && i.inserir.to_ascii_lowercase().starts_with(&prefixo));
    itens.sort_by(|a, b| {
        (a.grupo, a.inserir.to_ascii_lowercase(), &a.inserir).cmp(&(
            b.grupo,
            b.inserir.to_ascii_lowercase(),
            &b.inserir,
        ))
    });
    let mut vistos = HashSet::new();
    itens.retain(|i| vistos.insert(i.inserir.clone()));
    Some(Completar {
        inicio,
        fim: offset,
        itens,
    })
}

/// O cursor está dentro de comentário ou do texto de uma string: não há o
/// que completar. Interpolações (`$nome`, `${…}`) são código.
fn fora_de_codigo(texto: &str, offset: usize) -> bool {
    let Ok(tokens) = dartforge_frontend::lexer::lex(texto) else {
        return false;
    };
    let mut fim_anterior = 0;
    for t in &tokens {
        if t.span.start >= offset {
            break;
        }
        if offset < t.span.end {
            // Dentro de um token: só strings são "fora de código".
            return !matches!(
                t.kind,
                dartforge_frontend::token::Kind::Ident
                    | dartforge_frontend::token::Kind::Keyword(_)
            );
        }
        fim_anterior = t.span.end;
    }
    let lacuna = &texto[fim_anterior.min(offset)..offset];
    if let Some(i) = lacuna.find("//") {
        return !lacuna[i..].contains('\n');
    }
    lacuna.contains("/*")
}

/// Texto com o nome sob o cursor trocado pelo sentinela (e o fecho que o
/// faz analisar); `None` quando nenhuma variante põe o sentinela numa
/// expressão.
fn preparar(texto: &str, inicio: usize, fim: usize, features: LibraryFeatures) -> Option<String> {
    let mut melhor: Option<(usize, String)> = None;
    for fecho in FECHOS {
        let candidato = format!("{}{SENTINELA}{fecho}{}", &texto[..inicio], &texto[fim..]);
        let mut nomes = Interner::new();
        let analisado = dartforge_frontend::parser::parse_com(&candidato, &mut nomes, features);
        if achar_sentinela(&analisado.ast, inicio).is_none() {
            continue;
        }
        let erros = analisado.diagnostics.len();
        if melhor.as_ref().is_none_or(|(e, _)| erros < *e) {
            let zero = erros == 0;
            melhor = Some((erros, candidato));
            if zero {
                break;
            }
        }
    }
    melhor.map(|(_, t)| t)
}

/// A expressão do sentinela em `inicio` e, se é acesso a membro, o alvo.
fn achar_sentinela(ast: &ast::Ast, inicio: usize) -> Option<(ExprId, Option<ExprId>)> {
    ast.exprs
        .iter()
        .enumerate()
        .find_map(|(i, e)| match &e.kind {
            ExprKind::Identifier(n)
                if n.span.start == inicio && n.span.end == inicio + SENTINELA.len() =>
            {
                Some((ExprId(i as u32), None))
            }
            ExprKind::Property { target, name, .. }
                if name.span.start == inicio && name.span.end == inicio + SENTINELA.len() =>
            {
                Some((ExprId(i as u32), Some(*target)))
            }
            _ => None,
        })
}

struct Coletor {
    itens: Vec<ItemCompletar>,
    biblioteca: LibraryId,
}

impl Coletor {
    fn empurrar(
        &mut self,
        grupo: u8,
        especie: u32,
        rotulo: String,
        inserir: String,
        detalhe: Option<String>,
    ) {
        self.itens.push(ItemCompletar {
            rotulo,
            especie,
            detalhe,
            inserir,
            grupo,
        });
    }

    fn palavras(&mut self, palavras: &[&str]) {
        for p in palavras {
            self.empurrar(
                grupo::PALAVRA,
                especie::PALAVRA_CHAVE,
                p.to_string(),
                p.to_string(),
                None,
            );
        }
    }

    /// Nome privado de outra biblioteca: invisível aqui.
    fn invisivel(&self, consulta: &Consulta, nome: &str, dona: LibraryId) -> bool {
        let _ = consulta;
        nome.starts_with('_') && dona != self.biblioteca
    }

    /// Item de uma função (método, getter, acessor) com o tipo `tipo` já
    /// visto pelo receptor.
    fn funcao(&mut self, consulta: &Consulta, grupo: u8, f: FunctionElementId, tipo: TypeId) {
        let fe = consulta.programa.function(f);
        let nome = consulta.nome(fe.name).trim_end_matches('=').to_string();
        if nome.is_empty()
            || !nome.as_bytes()[0].is_ascii_alphabetic() && !nome.starts_with(['_', '$'])
        {
            return;
        }
        if self.invisivel(consulta, &nome, fe.library) {
            return;
        }
        match fe.kind {
            FunctionKind::ImplicitAccessor => {
                let constante_de_enum = fe.variable.is_some_and(|v| {
                    let ve = consulta.programa.variable(v);
                    ve.class
                        .is_some_and(|c| consulta.programa.class(c).enum_constants.contains(&v))
                });
                let especie = if constante_de_enum {
                    especie::MEMBRO_DE_ENUM
                } else if fe.class.is_some() || fe.extension.is_some() {
                    especie::CAMPO
                } else {
                    especie::VARIAVEL
                };
                self.empurrar(
                    grupo,
                    especie,
                    nome.clone(),
                    nome,
                    Some(consulta.formatar(tipo)),
                );
            }
            FunctionKind::Getter | FunctionKind::Setter => {
                let especie = if fe.class.is_some() || fe.extension.is_some() {
                    especie::PROPRIEDADE
                } else {
                    especie::VARIAVEL
                };
                self.empurrar(
                    grupo,
                    especie,
                    nome.clone(),
                    nome,
                    Some(consulta.formatar(tipo)),
                );
            }
            FunctionKind::Function
            | FunctionKind::Constructor
            | FunctionKind::SyntheticConstructor => {
                let especie = match fe.kind {
                    FunctionKind::Function if fe.class.is_some() || fe.extension.is_some() => {
                        especie::METODO
                    }
                    FunctionKind::Function => especie::FUNCAO,
                    _ => especie::CONSTRUTOR,
                };
                let sem_parametros = matches!(consulta.tabela.get(tipo), Type::Function { positional, optional, named, .. }
                    if positional.is_empty() && optional.is_empty() && named.is_empty());
                let rotulo = if sem_parametros {
                    format!("{nome}()")
                } else {
                    format!("{nome}(…)")
                };
                self.empurrar(
                    grupo,
                    especie,
                    rotulo,
                    nome,
                    Some(consulta.detalhe_de_funcao(f, tipo)),
                );
            }
            FunctionKind::Operator => {}
        }
    }

    /// Item de um elemento de topo (da biblioteca ou importado).
    fn elemento(&mut self, consulta: &Consulta, grupo: u8, elemento: Element) {
        let programa = &consulta.programa;
        match elemento {
            Element::Class(c) => {
                let classe = programa.class(c);
                let nome = consulta.nome(classe.name).to_string();
                if self.invisivel(consulta, &nome, classe.library)
                    || classe.kind == ClassKind::MixinApplication && classe.decl.is_none()
                {
                    return;
                }
                let especie = if classe.kind == ClassKind::Enum {
                    especie::ENUM
                } else {
                    especie::CLASSE
                };
                self.empurrar(grupo, especie, nome.clone(), nome, None);
            }
            Element::Typedef(t) => {
                let nome = consulta.nome(programa.typedef(t).name).to_string();
                if !self.invisivel(consulta, &nome, programa.typedef(t).library) {
                    self.empurrar(grupo, especie::CLASSE, nome.clone(), nome, None);
                }
            }
            Element::Extension(x) => {
                let Some(n) = programa.extension(x).name else {
                    return;
                };
                let nome = consulta.nome(n).to_string();
                if !self.invisivel(consulta, &nome, programa.extension(x).library) {
                    self.empurrar(grupo, especie::CLASSE, nome.clone(), nome, None);
                }
            }
            Element::Function(f) => {
                let dados = &consulta.outline.functions[f.0 as usize];
                let tipo = match programa.function(f).kind {
                    FunctionKind::Getter | FunctionKind::ImplicitAccessor => dados.return_type,
                    FunctionKind::Setter => dados
                        .parameters
                        .first()
                        .map_or(consulta.core.dynamic_, |p| p.ty),
                    _ => dados.signature,
                };
                let tipo = match programa
                    .function(f)
                    .variable
                    .and_then(|v| consulta.tipo_da_variavel(v))
                {
                    Some(t) if programa.function(f).kind == FunctionKind::ImplicitAccessor => t,
                    _ => tipo,
                };
                self.funcao(consulta, grupo, f, tipo);
            }
            Element::Variable(v) => {
                let ve = programa.variable(v);
                let nome = consulta.nome(ve.name).to_string();
                if !self.invisivel(consulta, &nome, ve.library) {
                    let detalhe = consulta.tipo_da_variavel(v).map(|t| consulta.formatar(t));
                    self.empurrar(grupo, especie::VARIAVEL, nome.clone(), nome, detalhe);
                }
            }
            Element::Prefix(_, p) => {
                let nome = consulta.nome(p).to_string();
                self.empurrar(grupo::PREFIXO, especie::MODULO, nome.clone(), nome, None);
            }
        }
    }

    fn espaco_de_nomes(
        &mut self,
        consulta: &Consulta,
        espaco: &Namespace,
        grupo_de: impl Fn(LibraryId) -> u8,
    ) {
        let mut entradas: Vec<(&SymbolId, &dartforge_elements::model::Binding)> =
            espaco.iter().collect();
        entradas.sort_by_key(|(s, _)| consulta.nome(**s).to_string());
        for (_, vinculo) in entradas {
            if vinculo.ambiguous {
                continue;
            }
            let Some(elemento) = vinculo.getter.or(vinculo.setter) else {
                continue;
            };
            let dona = biblioteca_do_elemento(consulta, elemento);
            self.elemento(consulta, grupo_de(dona), elemento);
        }
    }

    /// Declarações de topo, importados sem prefixo e os prefixos.
    fn biblioteca(&mut self, consulta: &Consulta) {
        let lib = consulta.programa.library(self.biblioteca);
        let propria = self.biblioteca;
        self.espaco_de_nomes(consulta, &lib.scope, |dona| {
            if dona == propria {
                grupo::BIBLIOTECA
            } else {
                grupo::IMPORTADO
            }
        });
        let mut prefixos: Vec<String> = lib
            .prefixes
            .keys()
            .map(|p| consulta.nome(*p).to_string())
            .collect();
        prefixos.sort();
        for p in prefixos {
            self.empurrar(grupo::PREFIXO, especie::MODULO, p.clone(), p, None);
        }
    }

    /// Nomes visíveis no identificador sondado.
    fn escopo(&mut self, consulta: &mut Consulta) {
        if let Some(escopo) = consulta.escopo.clone() {
            for local in &escopo.locais {
                let nome = consulta.nome(local.nome).to_string();
                let especie = if local.funcao {
                    especie::FUNCAO
                } else {
                    especie::VARIAVEL
                };
                let detalhe = consulta.formatar(local.tipo);
                self.empurrar(grupo::LOCAL, especie, nome.clone(), nome, Some(detalhe));
            }
            for (nome, _) in &escopo.parametros_de_tipo {
                let nome = consulta.nome(*nome).to_string();
                self.empurrar(
                    grupo::LOCAL,
                    especie::PARAMETRO_DE_TIPO,
                    nome.clone(),
                    nome,
                    None,
                );
            }
            if let Some(classe) = escopo.classe {
                if !escopo.estatico
                    && let Some(this) = escopo.tipo_this
                {
                    self.membros_de_instancia(consulta, this);
                }
                self.estaticos_da_classe(consulta, classe, false);
            }
            if let Some(extensao) = escopo.extensao {
                let x = consulta.programa.extension(extensao);
                let mut membros: Vec<FunctionElementId> =
                    x.static_members.values().copied().collect();
                if !escopo.estatico {
                    membros.extend(x.instance_members.values().copied());
                }
                membros.sort();
                for f in membros {
                    let tipo = tipo_declarado(consulta, f);
                    self.funcao(consulta, grupo::MEMBRO, f, tipo);
                }
            }
        }
        self.biblioteca(consulta);
    }

    /// Membros estáticos (e, com `construtores`, os construtores nomeados)
    /// de uma classe.
    fn estaticos_da_classe(&mut self, consulta: &Consulta, classe: ClassId, construtores: bool) {
        let c = consulta.programa.class(classe);
        let mut membros: Vec<FunctionElementId> = c.static_members.values().copied().collect();
        membros.sort();
        for f in membros {
            let tipo = tipo_declarado(consulta, f);
            self.funcao(consulta, grupo::MEMBRO, f, tipo);
        }
        if construtores {
            for (nome, f) in c.construtores() {
                if consulta.nome(nome).is_empty() {
                    continue;
                }
                let tipo = consulta.outline.functions[f.0 as usize].signature;
                self.funcao(consulta, grupo::MEMBRO, f, tipo);
            }
        }
    }

    /// Membros de instância de um receptor com tipo estático `receptor`,
    /// com os tipos vistos por ele (substituídos), pela busca de membros de
    /// `crates/types`. Inclui os de extensões aplicáveis.
    fn membros_de_instancia(&mut self, consulta: &mut Consulta, receptor: TypeId) {
        let mut busca = receptor;
        // Parâmetro de tipo: os membros do limite.
        for _ in 0..16 {
            match consulta.tabela.get(busca).clone() {
                Type::TypeParameter { param, .. } => busca = consulta.tabela.param(param).bound,
                Type::Intersection { bound, .. } => busca = bound,
                Type::Dynamic | Type::Never | Type::Void | Type::FutureOr { .. } | Type::Null => {
                    busca = consulta.core.object;
                    break;
                }
                _ => break,
            }
        }
        let mut classes = Vec::new();
        match consulta.tabela.get(busca).clone() {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                classes.push(class)
            }
            Type::Record {
                positional, named, ..
            } => {
                for (i, t) in positional.iter().enumerate() {
                    let nome = format!("${}", i + 1);
                    self.empurrar(
                        grupo::MEMBRO,
                        especie::CAMPO,
                        nome.clone(),
                        nome,
                        Some(consulta.formatar(*t)),
                    );
                }
                for (n, t) in named.iter() {
                    let nome = consulta.nome(*n).to_string();
                    self.empurrar(
                        grupo::MEMBRO,
                        especie::CAMPO,
                        nome.clone(),
                        nome,
                        Some(consulta.formatar(*t)),
                    );
                }
                classes.extend(consulta.core.record_class);
            }
            _ => classes.extend(consulta.core.object_class),
        }
        let mut nomes: Vec<SymbolId> = Vec::new();
        let mut vistos_nomes = HashSet::new();
        let mut vistas = HashSet::new();
        while let Some(c) = classes.pop() {
            if !vistas.insert(c) {
                continue;
            }
            let classe = consulta.programa.class(c);
            let mut proprios: Vec<SymbolId> = classe.instance_members.keys().copied().collect();
            proprios.sort_by_key(|s| s.as_u32());
            for n in proprios {
                if vistos_nomes.insert(n) {
                    nomes.push(n);
                }
            }
            classes.extend(classe.supertype_class);
            classes.extend(classe.mixin_classes.iter().copied());
            classes.extend(classe.interface_classes.iter().copied());
            classes.extend(classe.on_classes.iter().copied());
        }
        // Extensões visíveis na biblioteca (as mesmas que a busca considera).
        let lib = consulta.programa.library(self.biblioteca);
        let mut extensoes: Vec<_> = lib
            .scope
            .values()
            .filter_map(|b| match b.getter {
                Some(Element::Extension(x)) => Some(x),
                _ => None,
            })
            .collect();
        extensoes.sort();
        for x in extensoes {
            let mut proprios: Vec<SymbolId> = consulta
                .programa
                .extension(x)
                .instance_members
                .keys()
                .copied()
                .collect();
            proprios.sort_by_key(|s| s.as_u32());
            for n in proprios {
                if vistos_nomes.insert(n) {
                    nomes.push(n);
                }
            }
        }
        for nome in nomes {
            let texto = consulta.nome(nome);
            if texto.ends_with('=')
                || !texto.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
            {
                continue;
            }
            let biblioteca = self.biblioteca;
            let achado = consulta
                .resolvedor()
                .lookup_member(busca, nome, false, biblioteca);
            let Some((resolvido, tipo)) = achado else {
                continue;
            };
            let funcao = match resolvido {
                Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                } => f,
                Resolved::ExtensionMember { member, .. } => member,
                Resolved::Member {
                    member: MemberRef::Variable(v),
                    ..
                } => match consulta.programa.variable(v).getter {
                    Some(g) => g,
                    None => continue,
                },
                _ => continue,
            };
            self.funcao(consulta, grupo::MEMBRO, funcao, tipo);
        }
    }

    /// `alvo.▮`: membros de instância pelo tipo do alvo, estáticos e
    /// construtores de uma classe, ou os nomes de um prefixo de import.
    fn membros_do_alvo(
        &mut self,
        consulta: &mut Consulta,
        unidade: dartforge_elements::model::UnitId,
        _expr: ExprId,
        alvo: ExprId,
    ) {
        let corpos = &consulta.corpos.units[unidade.0 as usize];
        let resolvido = corpos.get_resolved(alvo).cloned();
        let ast = &consulta.programa.unit(unidade).ast;
        if let (Some(Resolved::Prefix(_)), ExprKind::Identifier(p)) =
            (&resolvido, &ast.expr(alvo).kind)
        {
            let lib = consulta.programa.library(self.biblioteca);
            if let Some(espaco) = lib.prefixes.get(&p.sym) {
                let propria = self.biblioteca;
                self.espaco_de_nomes(consulta, espaco, |dona| {
                    if dona == propria {
                        grupo::BIBLIOTECA
                    } else {
                        grupo::IMPORTADO
                    }
                });
            }
            return;
        }
        match resolvido {
            Some(Resolved::Element(Element::Class(c))) => {
                self.estaticos_da_classe(consulta, c, true);
                return;
            }
            Some(Resolved::Element(Element::Extension(x))) => {
                let mut membros: Vec<FunctionElementId> = consulta
                    .programa
                    .extension(x)
                    .static_members
                    .values()
                    .copied()
                    .collect();
                membros.sort();
                for f in membros {
                    let tipo = tipo_declarado(consulta, f);
                    self.funcao(consulta, grupo::MEMBRO, f, tipo);
                }
                return;
            }
            _ => {}
        }
        let Some(tipo) = consulta.corpos.units[unidade.0 as usize].get_type(alvo) else {
            return;
        };
        self.membros_de_instancia(consulta, tipo);
    }

    /// Argumento posicional sendo digitado numa chamada: os parâmetros
    /// nomeados ainda não passados.
    fn argumentos_nomeados(
        &mut self,
        consulta: &Consulta,
        unidade: dartforge_elements::model::UnitId,
        expr: ExprId,
    ) {
        let ast = &consulta.programa.unit(unidade).ast;
        let corpos = &consulta.corpos.units[unidade.0 as usize];
        for (i, e) in ast.exprs.iter().enumerate() {
            let (argumentos, alvo) = match &e.kind {
                ExprKind::Call { target, arguments } => (arguments, Some(*target)),
                ExprKind::InstanceCreation { arguments, .. } => (arguments, None),
                _ => continue,
            };
            if !argumentos
                .args
                .iter()
                .any(|a| a.name.is_none() && a.value == expr)
            {
                continue;
            }
            let passados: HashSet<SymbolId> = argumentos
                .args
                .iter()
                .filter_map(|a| a.name.map(|n| n.sym))
                .collect();
            let mut nomeados: Vec<(String, TypeId)> = Vec::new();
            let construtor = match corpos.get_resolved(ExprId(i as u32)) {
                Some(Resolved::Constructor(f)) => Some(*f),
                _ => alvo.and_then(|a| match corpos.get_resolved(a) {
                    Some(Resolved::Constructor(f)) => Some(*f),
                    _ => None,
                }),
            };
            if let Some(f) = construtor {
                for p in consulta.outline.functions[f.0 as usize].parameters.iter() {
                    if p.kind == ast::ParameterKind::Named
                        && let Some(n) = p.externo.or(p.name).filter(|n| !passados.contains(n))
                    {
                        nomeados.push((consulta.nome(n).to_string(), p.ty));
                    }
                }
            } else if let Some(t) = alvo.and_then(|a| corpos.get_type(a))
                && let Type::Function { named, .. } = consulta.tabela.get(t)
            {
                for (n, t, _) in named.iter() {
                    if !passados.contains(n) {
                        nomeados.push((consulta.nome(*n).to_string(), *t));
                    }
                }
            }
            for (nome, tipo) in nomeados {
                self.empurrar(
                    grupo::NOMEADO,
                    especie::VARIAVEL,
                    format!("{nome}: "),
                    format!("{nome}: "),
                    Some(consulta.formatar(tipo)),
                );
            }
            return;
        }
    }
}

/// Tipo declarado de um membro, sem receptor (estáticos e extensões).
fn tipo_declarado(consulta: &Consulta, f: FunctionElementId) -> TypeId {
    let fe = consulta.programa.function(f);
    let dados = &consulta.outline.functions[f.0 as usize];
    match fe.kind {
        FunctionKind::ImplicitAccessor => fe
            .variable
            .and_then(|v| consulta.tipo_da_variavel(v))
            .unwrap_or(dados.return_type),
        FunctionKind::Getter => dados.return_type,
        FunctionKind::Setter => dados
            .parameters
            .first()
            .map_or(consulta.core.dynamic_, |p| p.ty),
        _ => dados.signature,
    }
}

/// Biblioteca que declara um elemento de topo.
fn biblioteca_do_elemento(consulta: &Consulta, elemento: Element) -> LibraryId {
    let p = &consulta.programa;
    match elemento {
        Element::Class(c) => p.class(c).library,
        Element::Extension(x) => p.extension(x).library,
        Element::Typedef(t) => p.typedef(t).library,
        Element::Function(f) => p.function(f).library,
        Element::Variable(v) => p.variable(v).library,
        Element::Prefix(l, _) => l,
    }
}
