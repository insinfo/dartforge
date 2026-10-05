//! `workspace/symbol` como o servidor do Dart 3.6.2 (docs/LSP-ESPECIFICACAO.md
//! §5): o `WorkspaceSymbolHandler` sobre o `FindDeclarations` do analyzer.
//!
//! * as bibliotecas vêm na ordem do `OwnedFiles`: primeiro os arquivos
//!   adicionados (os do projeto), depois os conhecidos pelo
//!   `discoverAvailableFiles` (as bibliotecas do SDK na ordem do
//!   `libraries.dart` e os `lib/` dos pacotes do `package_config.json`); uma
//!   parte não é biblioteca e fica de fora (o conteúdo dela entra pela
//!   biblioteca dona);
//! * cada biblioteca passa pelo pré-filtro `ElementNameUnion.contains`;
//! * por unidade, na ordem do `_FindCompilationUnitDeclarations`: acessores
//!   de topo, classes (com os acessores, construtores, campos e métodos),
//!   enums, mixins, extensões, extension types, funções, typedefs e
//!   variáveis; só os elementos não sintéticos;
//! * o nome casa pelo `FuzzyMatcher` (`MatchStyle.TEXT`) com score ≥ 0, sem
//!   ordenar; a coleta para em 500;
//! * o item: o nome com `()`/`(…)` pelo `getDisplayString()` do executável,
//!   a espécie pelo `declarationKindToSymbolKind`, o code range do elemento
//!   e o `containerName` de classe ou mixin.
//!
//! O índice é só sintático (o parser, sem resolução) e fica em cache por
//! arquivo; o de uma biblioteca é refeito quando ela ou uma das partes muda.

use crate::casador::{Casador, Estilo};
use dartforge_frontend::ast::{self, Annotation, Ast, DeclKind, DirectiveKind, FunctionKind, MemberKind, TypeKind};
use dartforge_frontend::comentarios::Comentarios;
use dartforge_intern::Interner;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// O limite de resultados do handler.
pub(crate) const LIMITE: usize = 500;

// -- ElementNameUnion ---------------------------------------------------------

/// `ElementNameUnion`: para cada posição de letra (até 63), as letras que
/// algum nome da biblioteca tem nela; `mascara[0]` é o maior comprimento.
#[derive(Debug, Clone)]
struct UniaoDeNomes {
    mascara: [u32; 64],
}

impl UniaoDeNomes {
    const MAXIMO: u32 = 63;

    fn vazia() -> Self {
        Self { mascara: [0; 64] }
    }

    fn adicionar(&mut self, nome: &str) {
        if self.mascara[0] >= Self::MAXIMO {
            return;
        }
        let mut indice = 1usize;
        for c in nome.encode_utf16() {
            if (0x41..=0x5A).contains(&c) {
                self.mascara[indice] |= 1 << (c - 0x41);
                indice += 1;
            } else if (0x61..=0x7A).contains(&c) {
                self.mascara[indice] |= 1 << (c - 0x61);
                indice += 1;
            }
            if indice > Self::MAXIMO as usize {
                self.mascara[0] = Self::MAXIMO;
                return;
            }
        }
        let comprimento = (indice - 1) as u32;
        if self.mascara[0] < comprimento {
            self.mascara[0] = comprimento;
        }
    }

    fn contem(&self, padrao: &str) -> bool {
        let maximo = self.mascara[0];
        if maximo >= Self::MAXIMO {
            return true;
        }
        let mut indice = 1usize;
        for c in padrao.encode_utf16() {
            let m: u32 = if (0x41..=0x5A).contains(&c) {
                1 << (c - 0x41)
            } else if (0x61..=0x7A).contains(&c) {
                1 << (c - 0x61)
            } else {
                continue;
            };
            loop {
                if indice > maximo as usize {
                    return false;
                }
                let x = self.mascara[indice];
                indice += 1;
                if x & m != 0 {
                    break;
                }
            }
        }
        true
    }
}

// -- Resumo de tipos (para o `getDisplayString()`) -----------------------------

/// O que importa de um tipo escrito para saber onde fica o primeiro `(` do
/// `getDisplayString()` do executável.
#[derive(Debug, Clone)]
enum Resumo {
    /// `Nome<args>` (o último segmento de `p.Nome`).
    Nome { nome: String, args: Vec<Resumo> },
    /// `R Function(…)`, com o retorno escrito.
    Funcao(Option<Box<Resumo>>),
    /// `(int, String)`.
    Registro,
    Void,
    /// O parâmetro de tipo `i` do `typedef` que está sendo expandido.
    Parametro(usize),
}

/// Um `typedef`: o tipo apelidado (os parâmetros de tipo viram
/// [`Resumo::Parametro`]).
#[derive(Debug, Clone)]
struct Apelido {
    alvo: Resumo,
}

/// Onde o `(` aparece no texto exibido de um tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parentese {
    Nenhum,
    Contem,
    /// O texto começa com `(` (um registro).
    Comeca,
}

/// Os tipos declarados numa biblioteca: `Some` num `typedef`, `None` numa
/// classe, mixin, enum ou extension type (que esconde um `typedef` homônimo
/// de outra biblioteca).
type TiposLocais = HashMap<String, Option<Apelido>>;

fn parentese(r: &Resumo, ctx: &[Parentese], locais: &TiposLocais, globais: &HashMap<String, Apelido>, profundidade: u32) -> Parentese {
    match r {
        Resumo::Void => Parentese::Nenhum,
        Resumo::Registro => Parentese::Comeca,
        Resumo::Parametro(i) => ctx.get(*i).copied().unwrap_or(Parentese::Nenhum),
        // `R Function(…)`: começa com `(` só quando o retorno começa.
        Resumo::Funcao(retorno) => match retorno {
            Some(x) if parentese(x, ctx, locais, globais, profundidade) == Parentese::Comeca => Parentese::Comeca,
            _ => Parentese::Contem,
        },
        Resumo::Nome { nome, args } => {
            let argumentos: Vec<Parentese> = args.iter().map(|a| parentese(a, ctx, locais, globais, profundidade)).collect();
            if profundidade < 16 {
                let apelido = match locais.get(nome) {
                    Some(local) => local.as_ref(),
                    None => globais.get(nome),
                };
                // O `getDisplayString()` sem `preferTypeAlias` mostra o tipo
                // apelidado.
                if let Some(a) = apelido {
                    return parentese(&a.alvo, &argumentos, locais, globais, profundidade + 1);
                }
            }
            if argumentos.iter().any(|p| *p != Parentese::Nenhum) { Parentese::Contem } else { Parentese::Nenhum }
        }
    }
}

fn resumir(ast: &Ast, nomes: &Interner, t: ast::TypeId, parametros: &[String]) -> Resumo {
    let ty = ast.ty(t);
    match &ty.kind {
        TypeKind::Void => Resumo::Void,
        TypeKind::Record { .. } => Resumo::Registro,
        TypeKind::Function { return_type, .. } => Resumo::Funcao(return_type.map(|r| Box::new(resumir(ast, nomes, r, parametros)))),
        TypeKind::Named { name, args } => {
            let nome = name.last().map(|n| nomes.resolve(n.sym).to_string()).unwrap_or_default();
            if name.len() == 1
                && args.is_empty()
                && let Some(i) = parametros.iter().position(|p| *p == nome)
            {
                return Resumo::Parametro(i);
            }
            Resumo::Nome { nome, args: args.iter().map(|a| resumir(ast, nomes, *a, parametros)).collect() }
        }
    }
}

// -- Declarações ----------------------------------------------------------------

/// `DeclarationKind` do analyzer (as usadas pelo `_getSearchElementKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Especie {
    Classe,
    AliasDeClasse,
    Enum,
    Mixin,
    Extensao,
    ExtensionType,
    Construtor,
    ConstanteDeEnum,
    Campo,
    Funcao,
    Metodo,
    Getter,
    Setter,
    Typedef,
    Variavel,
}

impl Especie {
    /// As preferências do `declarationKindToSymbolKind`.
    fn preferencias(self) -> &'static [u64] {
        match self {
            Especie::Classe | Especie::AliasDeClasse | Especie::Extensao | Especie::ExtensionType | Especie::Mixin | Especie::Typedef => &[5],
            Especie::Construtor => &[9],
            Especie::Enum => &[10],
            Especie::ConstanteDeEnum => &[22, 10],
            Especie::Campo => &[8],
            Especie::Funcao => &[12],
            Especie::Getter | Especie::Setter => &[7],
            Especie::Metodo => &[6],
            Especie::Variavel => &[13],
        }
    }
}

/// O que o `getDisplayString()` do elemento precisa para o sufixo.
#[derive(Debug, Clone)]
enum Assinatura {
    /// Não é executável.
    Nenhuma,
    /// `A A.n(…)`: o primeiro `(` é o da lista.
    Lista { vazia: bool },
    /// `R get x`: só o retorno pode ter `(`.
    Getter { retorno: Option<Resumo>, augment: bool },
    /// `R nome<T>(…)`.
    Executavel { retorno: Option<Resumo>, vazia: bool, augment: bool },
}

#[derive(Debug, Clone)]
struct Declaracao {
    nome: String,
    especie: Especie,
    /// `className ?? mixinName`.
    conteiner: Option<String>,
    assinatura: Assinatura,
    /// O code range em posições LSP do arquivo.
    inicio: (u32, u32),
    fim: (u32, u32),
}

#[derive(Debug)]
struct Unidade {
    uri: String,
    declaracoes: Vec<Declaracao>,
}

#[derive(Debug)]
struct Biblioteca {
    uniao: UniaoDeNomes,
    unidades: Vec<Unidade>,
    tipos: TiposLocais,
}

/// A impressão de um arquivo lido: o tamanho e a data no disco, ou o texto
/// aberto (pelo tamanho e um resumo).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Impressao {
    Disco(Option<SystemTime>, u64),
    Aberto(usize, u64),
}

#[derive(Debug)]
struct Entrada {
    /// O arquivo da biblioteca e as partes, com as impressões.
    arquivos: Vec<(PathBuf, Impressao)>,
    biblioteca: Option<Arc<Biblioteca>>,
}

/// O índice: as bibliotecas já lidas, pelo caminho.
#[derive(Debug, Default)]
pub(crate) struct Indice {
    entradas: HashMap<PathBuf, Entrada>,
    /// As bibliotecas do SDK na ordem do `libraries.dart`, pelo `lib/`.
    sdk: Option<(PathBuf, Vec<PathBuf>)>,
}

/// Lê um arquivo (aberto ou do disco) com a sua impressão.
type Leitor<'a> = dyn Fn(&Path) -> Option<(String, Impressao)> + 'a;

/// A impressão de um arquivo no disco, sem lê-lo.
fn impressao_no_disco(caminho: &Path) -> Option<Impressao> {
    let m = std::fs::metadata(caminho).ok()?;
    Some(Impressao::Disco(m.modified().ok(), m.len()))
}

/// A impressão de um texto aberto.
fn impressao_de_texto(texto: &str) -> Impressao {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    texto.hash(&mut h);
    Impressao::Aberto(texto.len(), h.finish())
}

/// A primeira posição de código a partir de `pos` (pula espaços e
/// comentários).
fn proximo_codigo(fonte: &str, mut pos: usize) -> usize {
    let b = fonte.as_bytes();
    loop {
        while pos < b.len() && b[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if b.get(pos) == Some(&b'/') && b.get(pos + 1) == Some(&b'/') {
            while pos < b.len() && b[pos] != b'\n' && b[pos] != b'\r' {
                pos += 1;
            }
            continue;
        }
        if b.get(pos) == Some(&b'/') && b.get(pos + 1) == Some(&b'*') {
            let mut nivel = 0usize;
            while pos < b.len() {
                if b[pos] == b'/' && b.get(pos + 1) == Some(&b'*') {
                    nivel += 1;
                    pos += 2;
                } else if b[pos] == b'*' && b.get(pos + 1) == Some(&b'/') {
                    nivel -= 1;
                    pos += 2;
                    if nivel == 0 {
                        break;
                    }
                } else {
                    pos += 1;
                }
            }
            continue;
        }
        return pos;
    }
}

/// O que lê uma unidade: o texto, as linhas e os comentários.
struct Leitura<'a> {
    fonte: &'a str,
    ast: &'a Ast,
    nomes: &'a Interner,
    linhas: crate::utf16::TabelaLinhas,
    comentarios: Comentarios,
}

impl Leitura<'_> {
    fn nome(&self, n: ast::Name) -> String {
        self.nomes.resolve(n.sym).to_string()
    }

    /// O `offset` de um `AnnotatedNode`: o comentário de documentação e as
    /// anotações (`AnnotatedNodeImpl.beginToken`).
    fn inicio_anotado(&self, inicio: usize, metadata: &[Annotation]) -> usize {
        let apos = match metadata.last() {
            Some(m) => proximo_codigo(self.fonte, m.span.end),
            None => inicio,
        };
        let mut doc = self.comentarios.dart_doc(self.fonte, apos);
        if doc.is_none() {
            for m in metadata.iter().rev() {
                doc = self.comentarios.dart_doc(self.fonte, m.span.start);
                if doc.is_some() {
                    break;
                }
            }
        }
        let mut comeco = inicio;
        if let Some(m) = metadata.first() {
            comeco = comeco.min(m.span.start);
        }
        if let Some(d) = doc {
            comeco = comeco.min(d.start);
        }
        comeco
    }

    fn posicao(&self, offset: usize) -> (u32, u32) {
        self.linhas.posicao_de_offset(self.fonte, offset)
    }

    fn declaracao(&self, nome: String, especie: Especie, conteiner: Option<String>, assinatura: Assinatura, inicio: usize, fim: usize) -> Declaracao {
        Declaracao { nome, especie, conteiner, assinatura, inicio: self.posicao(inicio), fim: self.posicao(fim) }
    }

    /// O fim de uma variável: o do inicializador, senão o do nome.
    fn fim_da_variavel(&self, v: &ast::Variable) -> usize {
        v.initializer.map_or(v.name.span.end, |e| self.ast.expr(e).span.end)
    }
}

/// O conteúdo de uma unidade para a lista de declarações e a união.
struct Coletor<'a, 'b> {
    l: &'b Leitura<'a>,
    saida: Vec<Declaracao>,
    uniao: &'b mut UniaoDeNomes,
    tipos: &'b mut TiposLocais,
}

impl Coletor<'_, '_> {
    /// `_addDeclaration` sem o casamento (que é feito na consulta).
    fn push(&mut self, d: Declaracao) {
        self.saida.push(d);
    }

    fn assinatura_de_funcao(&self, f: &ast::Function, augment: bool, parametros_de_tipo: &[String]) -> Assinatura {
        let retorno = f.return_type.map(|t| resumir(self.l.ast, self.l.nomes, t, parametros_de_tipo));
        match f.kind {
            FunctionKind::Getter => Assinatura::Getter { retorno, augment },
            FunctionKind::Setter => Assinatura::Lista { vazia: f.parameters.as_ref().is_none_or(|p| p.is_empty()) },
            FunctionKind::Function | FunctionKind::Operator => {
                Assinatura::Executavel { retorno, vazia: f.parameters.as_ref().is_none_or(|p| p.is_empty()), augment }
            }
        }
    }

    /// Os membros de uma classe, mixin, enum, extensão ou extension type, na
    /// ordem do `_addClasses`/`_addExtensions`: acessores, construtores,
    /// campos (`primeiros` antes dos declarados) e métodos.
    fn membros(&mut self, membros: &[ast::MemberId], conteiner: Option<&str>, construtores: bool, primeiros_campos: Vec<Declaracao>, primeiros_construtores: Vec<Declaracao>) {
        let l = self.l;
        let ast = l.ast;
        let conteiner = conteiner.map(str::to_string);
        let mut acessores = Vec::new();
        let mut ctors = primeiros_construtores;
        let mut campos = primeiros_campos;
        let mut metodos = Vec::new();
        for &mid in membros {
            let m = ast.member(mid);
            let inicio = l.inicio_anotado(m.span.start, &m.metadata);
            match &m.kind {
                MemberKind::Field(lista) => {
                    for (i, v) in lista.variables.iter().enumerate() {
                        let nome = l.nome(v.name);
                        self.uniao.adicionar(&nome);
                        let comeco = if i == 0 { inicio } else { v.name.span.start };
                        campos.push(l.declaracao(nome, Especie::Campo, conteiner.clone(), Assinatura::Nenhuma, comeco, l.fim_da_variavel(v)));
                    }
                }
                MemberKind::Method(fid) => {
                    let f = ast.function(*fid);
                    let Some(n) = f.name else { continue };
                    let nome = l.nome(n);
                    self.uniao.adicionar(&nome);
                    let assinatura = self.assinatura_de_funcao(f, m.augment, &[]);
                    match f.kind {
                        FunctionKind::Getter => acessores.push(l.declaracao(nome, Especie::Getter, conteiner.clone(), assinatura, inicio, m.span.end)),
                        FunctionKind::Setter => acessores.push(l.declaracao(nome, Especie::Setter, conteiner.clone(), assinatura, inicio, m.span.end)),
                        _ => metodos.push(l.declaracao(nome, Especie::Metodo, conteiner.clone(), assinatura, inicio, m.span.end)),
                    }
                }
                MemberKind::Constructor(c) => {
                    if !construtores {
                        continue;
                    }
                    let nome = c.name.map(|n| l.nome(n)).filter(|n| n != "new").unwrap_or_default();
                    ctors.push(l.declaracao(nome, Especie::Construtor, conteiner.clone(), Assinatura::Lista { vazia: c.parameters.is_empty() }, inicio, m.span.end));
                }
            }
        }
        self.saida.extend(acessores);
        self.saida.extend(ctors);
        self.saida.extend(campos);
        self.saida.extend(metodos);
    }

    fn unidade(&mut self, unidade: &ast::CompilationUnit) {
        let l = self.l;
        let ast = l.ast;
        let mut acessores = Vec::new();
        let mut classes = Vec::new();
        let mut enums = Vec::new();
        let mut mixins = Vec::new();
        let mut extensoes = Vec::new();
        let mut tipos_de_extensao = Vec::new();
        let mut funcoes = Vec::new();
        let mut apelidos = Vec::new();
        let mut variaveis = Vec::new();
        for &did in &unidade.declarations {
            let d = ast.decl(did);
            match &d.kind {
                DeclKind::Class(_) => classes.push(did),
                DeclKind::Enum(_) => enums.push(did),
                DeclKind::Mixin(_) => mixins.push(did),
                DeclKind::Extension(_) => extensoes.push(did),
                DeclKind::ExtensionType(_) => tipos_de_extensao.push(did),
                DeclKind::Typedef(_) => apelidos.push(did),
                DeclKind::Variables(_) => variaveis.push(did),
                DeclKind::Function(fid) => match ast.function(*fid).kind {
                    FunctionKind::Getter | FunctionKind::Setter => acessores.push(did),
                    _ => funcoes.push(did),
                },
            }
        }
        for did in acessores {
            let d = ast.decl(did);
            let DeclKind::Function(fid) = d.kind else { continue };
            let f = ast.function(fid);
            let Some(n) = f.name else { continue };
            let nome = l.nome(n);
            self.uniao.adicionar(&nome);
            let especie = if f.kind == FunctionKind::Getter { Especie::Getter } else { Especie::Setter };
            let assinatura = self.assinatura_de_funcao(f, d.augment, &[]);
            let inicio = l.inicio_anotado(d.span.start, &d.metadata);
            self.push(l.declaracao(nome, especie, None, assinatura, inicio, d.span.end));
        }
        for did in classes.into_iter().chain(enums).chain(mixins) {
            let d = ast.decl(did);
            let inicio = l.inicio_anotado(d.span.start, &d.metadata);
            match &d.kind {
                DeclKind::Class(c) => {
                    let nome = l.nome(c.name);
                    self.uniao.adicionar(&nome);
                    self.tipos.insert(nome.clone(), None);
                    let especie = if c.mixin_application { Especie::AliasDeClasse } else { Especie::Classe };
                    self.push(l.declaracao(nome.clone(), especie, None, Assinatura::Nenhuma, inicio, d.span.end));
                    self.membros(&c.members, Some(&nome), true, Vec::new(), Vec::new());
                }
                DeclKind::Enum(e) => {
                    let nome = l.nome(e.name);
                    self.uniao.adicionar(&nome);
                    self.tipos.insert(nome.clone(), None);
                    self.push(l.declaracao(nome, Especie::Enum, None, Assinatura::Nenhuma, inicio, d.span.end));
                    // As constantes são campos; o `values` sintético só
                    // entra na união.
                    let mut constantes = Vec::new();
                    for k in &e.constants {
                        let n = l.nome(k.name);
                        self.uniao.adicionar(&n);
                        let ci = l.inicio_anotado(k.span.start, &k.metadata);
                        constantes.push(l.declaracao(n, Especie::ConstanteDeEnum, None, Assinatura::Nenhuma, ci, k.span.end));
                    }
                    self.uniao.adicionar("values");
                    self.membros(&e.members, None, true, constantes, Vec::new());
                }
                DeclKind::Mixin(m) => {
                    let nome = l.nome(m.name);
                    self.uniao.adicionar(&nome);
                    self.tipos.insert(nome.clone(), None);
                    self.push(l.declaracao(nome.clone(), Especie::Mixin, None, Assinatura::Nenhuma, inicio, d.span.end));
                    self.membros(&m.members, Some(&nome), true, Vec::new(), Vec::new());
                }
                _ => {}
            }
        }
        for did in extensoes {
            let d = ast.decl(did);
            let DeclKind::Extension(x) = &d.kind else { continue };
            if let Some(n) = x.name {
                let nome = l.nome(n);
                self.uniao.adicionar(&nome);
                let inicio = l.inicio_anotado(d.span.start, &d.metadata);
                self.push(l.declaracao(nome, Especie::Extensao, None, Assinatura::Nenhuma, inicio, d.span.end));
            }
            self.membros(&x.members, None, false, Vec::new(), Vec::new());
        }
        for did in tipos_de_extensao {
            let d = ast.decl(did);
            let DeclKind::ExtensionType(x) = &d.kind else { continue };
            let nome = l.nome(x.name);
            self.uniao.adicionar(&nome);
            self.tipos.insert(nome.clone(), None);
            let inicio = l.inicio_anotado(d.span.start, &d.metadata);
            self.push(l.declaracao(nome.clone(), Especie::ExtensionType, None, Assinatura::Nenhuma, inicio, d.span.end));
            // O campo de representação e o construtor primário
            // (`_builtRepresentationDeclaration`) vêm antes dos declarados.
            let campo_nome = Some(l.nome(x.representation_name)).filter(|n| !n.is_empty()).unwrap_or_else(|| "<empty>".to_string());
            self.uniao.adicionar(&campo_nome);
            let campo_inicio = x.representation_metadata.first().map_or(l.ast.ty(x.representation_type).span.start, |m| m.span.start);
            let campo = l.declaracao(campo_nome, Especie::Campo, Some(nome.clone()), Assinatura::Nenhuma, campo_inicio, x.representation_name.span.end);
            let (ctor_nome, ctor_inicio) = match x.constructor {
                Some(n) => {
                    let texto = l.nome(n);
                    // O nó começa no `.` do nome.
                    let antes = l.fonte[..n.span.start].trim_end();
                    let ponto = if antes.ends_with('.') { antes.len() - 1 } else { n.span.start };
                    (if texto == "new" { String::new() } else { texto }, ponto)
                }
                None => (String::new(), x.representation_span.start),
            };
            let ctor = l.declaracao(ctor_nome, Especie::Construtor, Some(nome.clone()), Assinatura::Lista { vazia: false }, ctor_inicio, x.representation_span.end);
            self.membros(&x.members, Some(&nome), true, vec![campo], vec![ctor]);
        }
        for did in funcoes {
            let d = ast.decl(did);
            let DeclKind::Function(fid) = d.kind else { continue };
            let f = ast.function(fid);
            let Some(n) = f.name else { continue };
            let nome = l.nome(n);
            self.uniao.adicionar(&nome);
            let parametros: Vec<String> = f.type_params.iter().map(|t| l.nome(t.name)).collect();
            let assinatura = self.assinatura_de_funcao(f, d.augment, &parametros);
            let inicio = l.inicio_anotado(d.span.start, &d.metadata);
            self.push(l.declaracao(nome, Especie::Funcao, None, assinatura, inicio, d.span.end));
        }
        for did in apelidos {
            let d = ast.decl(did);
            let DeclKind::Typedef(t) = &d.kind else { continue };
            let nome = l.nome(t.name);
            self.uniao.adicionar(&nome);
            let parametros: Vec<String> = t.type_params.iter().map(|p| l.nome(p.name)).collect();
            let alvo = match &t.kind {
                ast::TypedefKind::Alias(a) => resumir(ast, l.nomes, *a, &parametros),
                ast::TypedefKind::Legacy { return_type, .. } => Resumo::Funcao(return_type.map(|r| Box::new(resumir(ast, l.nomes, r, &parametros)))),
            };
            self.tipos.insert(nome.clone(), Some(Apelido { alvo }));
            let inicio = l.inicio_anotado(d.span.start, &d.metadata);
            self.push(l.declaracao(nome, Especie::Typedef, None, Assinatura::Nenhuma, inicio, d.span.end));
        }
        for did in variaveis {
            let d = ast.decl(did);
            let DeclKind::Variables(lista) = &d.kind else { continue };
            let inicio = l.inicio_anotado(d.span.start, &d.metadata);
            for (i, v) in lista.variables.iter().enumerate() {
                let nome = l.nome(v.name);
                self.uniao.adicionar(&nome);
                let comeco = if i == 0 { inicio } else { v.name.span.start };
                self.push(l.declaracao(nome, Especie::Variavel, None, Assinatura::Nenhuma, comeco, l.fim_da_variavel(v)));
            }
        }
    }
}

/// As partes declaradas por uma unidade, resolvidas a partir do arquivo.
fn partes(unidade: &ast::CompilationUnit, caminho: &Path) -> Vec<PathBuf> {
    let mut saida = Vec::new();
    for d in &unidade.directives {
        if let DirectiveKind::Part { uri } = &d.kind
            && let Some(relativo) = dartforge_elements::load::string_lit_value(uri)
            && !relativo.contains(':')
            && let Some(pasta) = caminho.parent()
        {
            saida.push(dartforge_elements::gerado::chave(&pasta.join(relativo)));
        }
    }
    saida
}

fn eh_parte(unidade: &ast::CompilationUnit) -> bool {
    unidade.directives.iter().any(|d| matches!(d.kind, DirectiveKind::PartOf { .. }))
}

/// Indexa a biblioteca de `caminho`: `None` quando é parte ou não se lê.
fn indexar(caminho: &Path, ler: &Leitor<'_>, uri_de: &dyn Fn(&Path) -> String) -> (Vec<(PathBuf, Impressao)>, Option<Biblioteca>) {
    let mut arquivos = Vec::new();
    let mut uniao = UniaoDeNomes::vazia();
    let mut tipos = TiposLocais::new();
    let mut unidades = Vec::new();
    // As unidades em pré-ordem: a que define, cada parte e as partes dela.
    let mut pendentes = vec![caminho.to_path_buf()];
    let mut vistos: HashSet<PathBuf> = HashSet::new();
    let mut primeira = true;
    while let Some(arquivo) = pendentes.pop() {
        if !vistos.insert(arquivo.clone()) {
            continue;
        }
        let Some((fonte, impressao)) = ler(&arquivo) else {
            if primeira {
                return (vec![(arquivo, Impressao::Disco(None, 0))], None);
            }
            continue;
        };
        arquivos.push((arquivo.clone(), impressao));
        let mut nomes = Interner::new();
        let analisado = dartforge_frontend::parser::parse(&fonte, &mut nomes);
        if primeira && eh_parte(&analisado.unit) {
            return (arquivos, None);
        }
        primeira = false;
        let leitura = Leitura {
            fonte: &fonte,
            ast: &analisado.ast,
            nomes: &nomes,
            linhas: crate::utf16::TabelaLinhas::construir(&fonte),
            comentarios: Comentarios::de(&fonte),
        };
        let mut coletor = Coletor { l: &leitura, saida: Vec::new(), uniao: &mut uniao, tipos: &mut tipos };
        coletor.unidade(&analisado.unit);
        let declaracoes = coletor.saida;
        let uri = uri_de(&arquivo);
        unidades.push(Unidade { uri, declaracoes });
        for p in partes(&analisado.unit, &arquivo).into_iter().rev() {
            pendentes.push(p);
        }
    }
    (arquivos, Some(Biblioteca { uniao, unidades, tipos }))
}

impl Indice {
    /// A biblioteca de `caminho`, do cache quando nenhum dos arquivos mudou.
    fn biblioteca(&mut self, caminho: &Path, ler: &Leitor<'_>, abertos: &HashMap<PathBuf, Impressao>, uri_de: &dyn Fn(&Path) -> String) -> Option<Arc<Biblioteca>> {
        let atual = |p: &Path| abertos.get(&dartforge_elements::gerado::chave(p)).cloned().or_else(|| impressao_no_disco(p));
        if let Some(e) = self.entradas.get(caminho)
            && e.arquivos.iter().all(|(p, i)| atual(p).as_ref() == Some(i))
        {
            return e.biblioteca.clone();
        }
        let (arquivos, biblioteca) = indexar(caminho, ler, uri_de);
        let biblioteca = biblioteca.map(Arc::new);
        self.entradas.insert(caminho.to_path_buf(), Entrada { arquivos, biblioteca: biblioteca.clone() });
        biblioteca
    }

    /// As bibliotecas do SDK (`dartSdk.sdkLibraries`): as entradas do mapa
    /// `libraries` de `_internal/sdk_library_metadata/lib/libraries.dart`,
    /// na ordem escrita, com o caminho relativo a `lib/`.
    fn bibliotecas_do_sdk(&mut self, lib: &Path) -> Vec<PathBuf> {
        if let Some((raiz, lista)) = &self.sdk
            && raiz == lib
        {
            return lista.clone();
        }
        let lista = ler_libraries_dart(lib);
        self.sdk = Some((lib.to_path_buf(), lista.clone()));
        lista
    }

    /// `FindDeclarations.compute` + o `_asSymbolInformation` do handler.
    /// `adicionados` são os arquivos do projeto; `pacotes`, os `lib/` dos
    /// pacotes (na ordem do `package_config.json`); `textos_abertos`, a URI
    /// e o texto dos documentos abertos pelo caminho normalizado
    /// (`gerado::chave`);
    /// `especies` os `SymbolKind` aceitos pelo cliente.
    pub(crate) fn buscar(
        &mut self,
        consulta: &str,
        adicionados: &[PathBuf],
        sdk: Option<&Path>,
        pacotes: &[PathBuf],
        textos_abertos: &HashMap<PathBuf, (String, String)>,
        especies: Option<&[u64]>,
    ) -> Vec<Value> {
        // Os documentos abertos valem pelo texto do editor (o overlay), com
        // a URI do cliente.
        let abertos: HashMap<PathBuf, Impressao> = textos_abertos.iter().map(|(k, (_, t))| (k.clone(), impressao_de_texto(t))).collect();
        let ler = |p: &Path| -> Option<(String, Impressao)> {
            let k = dartforge_elements::gerado::chave(p);
            if let (Some((_, t)), Some(i)) = (textos_abertos.get(&k), abertos.get(&k)) {
                return Some((t.clone(), i.clone()));
            }
            let i = impressao_no_disco(p)?;
            let t = std::fs::read_to_string(p).ok()?;
            Some((t, i))
        };
        let ler: &Leitor<'_> = &ler;
        let abertos = &abertos;
        let uri_de = |p: &Path| -> String {
            match textos_abertos.get(&dartforge_elements::gerado::chave(p)) {
                Some((uri, _)) => uri.clone(),
                None => url::Url::from_file_path(p).map(|u| u.to_string()).unwrap_or_default(),
            }
        };
        // Os arquivos na ordem do `OwnedFiles` (adicionados, depois os
        // conhecidos que não foram adicionados).
        let mut caminhos: Vec<PathBuf> = Vec::new();
        let mut vistos: HashSet<PathBuf> = HashSet::new();
        let mut incluir = |p: PathBuf, caminhos: &mut Vec<PathBuf>| {
            if vistos.insert(dartforge_elements::gerado::chave(&p)) {
                caminhos.push(p);
            }
        };
        for p in adicionados {
            incluir(p.clone(), &mut caminhos);
        }
        if let Some(lib) = sdk {
            for p in self.bibliotecas_do_sdk(lib) {
                incluir(p, &mut caminhos);
            }
        }
        for pasta in pacotes {
            let mut arquivos = Vec::new();
            descobrir(pasta, &mut arquivos);
            for p in arquivos {
                incluir(p, &mut caminhos);
            }
        }
        let bibliotecas: Vec<Arc<Biblioteca>> = caminhos.iter().filter_map(|c| self.biblioteca(c, ler, abertos, &uri_de)).collect();
        // Os `typedef` de todas as bibliotecas, para expandir os apelidos de
        // outra biblioteca (o primeiro de cada nome).
        let mut globais: HashMap<String, Apelido> = HashMap::new();
        for b in &bibliotecas {
            for (nome, a) in &b.tipos {
                if let Some(a) = a {
                    globais.entry(nome.clone()).or_insert_with(|| a.clone());
                }
            }
        }
        let mut casador = Casador::novo(consulta, Estilo::Texto);
        let mut saida = Vec::new();
        for b in &bibliotecas {
            if !b.uniao.contem(consulta) {
                continue;
            }
            for u in &b.unidades {
                for d in &u.declaracoes {
                    if saida.len() >= LIMITE {
                        return saida;
                    }
                    if casador.score(&d.nome) < 0.0 {
                        continue;
                    }
                    saida.push(simbolo(d, &u.uri, &b.tipos, &globais, especies));
                }
            }
        }
        saida
    }
}

/// O `SymbolInformation` de uma declaração.
fn simbolo(d: &Declaracao, uri: &str, locais: &TiposLocais, globais: &HashMap<String, Apelido>, especies: Option<&[u64]>) -> Value {
    let pelo_retorno = |retorno: &Option<Resumo>, augment: bool| {
        let p = retorno.as_ref().map_or(Parentese::Nenhum, |r| parentese(r, &[], locais, globais, 0));
        // `augment ` antes do retorno: o `(` nunca fica no começo.
        if augment && p == Parentese::Comeca { Parentese::Contem } else { p }
    };
    // `parameters`: o texto exibido a partir do primeiro `(` (que não pode
    // ser o primeiro caractere); `()` quando é só a lista vazia.
    let sufixo = match &d.assinatura {
        Assinatura::Nenhuma => "",
        Assinatura::Lista { vazia } => {
            if *vazia {
                "()"
            } else {
                "(…)"
            }
        }
        Assinatura::Getter { retorno, augment } => match pelo_retorno(retorno, *augment) {
            Parentese::Contem => "(…)",
            _ => "",
        },
        Assinatura::Executavel { retorno, vazia, augment } => match pelo_retorno(retorno, *augment) {
            Parentese::Comeca => "",
            Parentese::Contem => "(…)",
            Parentese::Nenhum if *vazia => "()",
            Parentese::Nenhum => "(…)",
        },
    };
    let aceita = |k: u64| especies.map_or(k <= 18, |l| l.contains(&k));
    let kind = d.especie.preferencias().iter().copied().find(|k| aceita(*k)).unwrap_or(19);
    let mut v = json!({
        "name": format!("{}{sufixo}", d.nome),
        "kind": kind,
        "location": {
            "uri": uri,
            "range": {
                "start": {"line": d.inicio.0, "character": d.inicio.1},
                "end": {"line": d.fim.0, "character": d.fim.1},
            },
        },
    });
    if let Some(c) = &d.conteiner {
        v["containerName"] = json!(c);
    }
    v
}

/// `discoverRecursively`: os `.dart` da pasta e das subpastas, na ordem do
/// sistema de arquivos (a mesma do `Directory.listSync` do Dart).
fn descobrir(pasta: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(pasta) else { return };
    for e in entradas.flatten() {
        let caminho = e.path();
        let Ok(tipo) = e.file_type() else { continue };
        if tipo.is_file() {
            if caminho.extension().is_some_and(|x| x == "dart") {
                saida.push(caminho);
            }
        } else if tipo.is_dir() {
            descobrir(&caminho, saida);
        }
    }
}

/// As entradas do mapa `libraries` do `libraries.dart` do SDK, na ordem
/// escrita: `'nome': const LibraryInfo('caminho', …)`.
fn ler_libraries_dart(lib: &Path) -> Vec<PathBuf> {
    let arquivo = lib.join("_internal").join("sdk_library_metadata").join("lib").join("libraries.dart");
    let Ok(texto) = std::fs::read_to_string(&arquivo) else { return Vec::new() };
    let Some(inicio) = texto.find("libraries = const {") else { return Vec::new() };
    let corpo = &texto[inicio..];
    let fim = corpo.find("\n};").unwrap_or(corpo.len());
    let corpo = &corpo[..fim];
    let mut saida = Vec::new();
    let mut resto = corpo;
    while let Some(i) = resto.find("LibraryInfo(") {
        resto = &resto[i + "LibraryInfo(".len()..];
        let texto_da_entrada = resto.trim_start();
        let Some(aspas) = texto_da_entrada.chars().next().filter(|c| *c == '\'' || *c == '"') else { continue };
        let conteudo = &texto_da_entrada[1..];
        let Some(f) = conteudo.find(aspas) else { continue };
        let relativo = &conteudo[..f];
        saida.push(dartforge_elements::gerado::chave(&lib.join(relativo)));
    }
    saida
}

/// Os diretórios `lib/` (o `packageUri`) dos pacotes do
/// `.dart_tool/package_config.json` de `raiz`, na ordem do arquivo.
pub(crate) fn pastas_dos_pacotes(raiz: &Path) -> Vec<PathBuf> {
    let arquivo = raiz.join(".dart_tool").join("package_config.json");
    let Ok(texto) = std::fs::read_to_string(&arquivo) else { return Vec::new() };
    let Ok(json) = serde_json::from_str::<Value>(&texto) else { return Vec::new() };
    let Some(pacotes) = json.get("packages").and_then(Value::as_array) else { return Vec::new() };
    let Some(base) = arquivo.parent() else { return Vec::new() };
    let resolver = |uri: &str, relativo_a: &Path| -> Option<PathBuf> {
        if let Ok(u) = url::Url::parse(uri) {
            return u.to_file_path().ok();
        }
        Some(dartforge_elements::gerado::chave(&relativo_a.join(uri.trim_end_matches('/'))))
    };
    let mut saida = Vec::new();
    for p in pacotes {
        let Some(raiz_uri) = p.get("rootUri").and_then(Value::as_str) else { continue };
        let Some(raiz_do_pacote) = resolver(raiz_uri, base) else { continue };
        let pacote_uri = p.get("packageUri").and_then(Value::as_str).unwrap_or("lib/");
        if let Some(lib) = resolver(pacote_uri, &raiz_do_pacote) {
            saida.push(lib);
        }
    }
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniao_como_o_analyzer() {
        let mut u = UniaoDeNomes::vazia();
        u.adicionar("parseArgs");
        assert!(u.contem("pa"));
        assert!(u.contem("PARSE"));
        assert!(u.contem("p_a"));
        assert!(!u.contem("xyz"));
        assert!(!u.contem("parseArgsX"));
    }

    #[test]
    fn sufixos_pelo_retorno() {
        let locais = TiposLocais::new();
        let globais = HashMap::new();
        let f = Declaracao {
            nome: "f".into(),
            especie: Especie::Funcao,
            conteiner: None,
            assinatura: Assinatura::Executavel { retorno: Some(Resumo::Registro), vazia: true, augment: false },
            inicio: (0, 0),
            fim: (0, 1),
        };
        assert_eq!(simbolo(&f, "file:///a.dart", &locais, &globais, None)["name"], "f");
        let g = Declaracao { assinatura: Assinatura::Executavel { retorno: Some(Resumo::Funcao(None)), vazia: true, augment: false }, ..f.clone() };
        assert_eq!(simbolo(&g, "file:///a.dart", &locais, &globais, None)["name"], "f(…)");
        let h = Declaracao { assinatura: Assinatura::Executavel { retorno: None, vazia: true, augment: false }, ..f.clone() };
        assert_eq!(simbolo(&h, "file:///a.dart", &locais, &globais, None)["name"], "f()");
        let k = Declaracao { especie: Especie::ConstanteDeEnum, assinatura: Assinatura::Nenhuma, ..f };
        assert_eq!(simbolo(&k, "file:///a.dart", &locais, &globais, None)["kind"], 10);
    }
}
