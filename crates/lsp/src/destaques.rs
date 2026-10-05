//! `textDocument/documentHighlight` como o `DartUnitOccurrencesComputer` do
//! servidor do Dart 3.6.2 (docs/LSP-ESPECIFICACAO.md §11.6): um visitante da
//! árvore no formato do analyzer (resolvida) junta, por **elemento
//! canônico** (o campo de um `this.x`, a variável de um getter/setter, a
//! declaração de um membro), os offsets em que ele aparece; a resposta são
//! os grupos com algum offset cobrindo o cursor (fim inclusivo), sem
//! família de sobrescrita, sem intervalos repetidos.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::arvore_analyzer::Marca;
use crate::projeto::{Alvo, Concreto, Projeto};
use crate::refatoracoes::Contexto;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, UnitId, VariableId};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;
use dartforge_types::{MemberRef, Resolved};

/// O elemento canônico (`_canonicalizeElement`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Chave {
    /// Local, parâmetro, função local, variável de padrão (pelo offset do
    /// nome declarado).
    Local(usize),
    Variavel(VariableId),
    /// O campo sintético de um getter/setter declarado (a dona e o nome).
    Acessores(Dono2, String),
    Funcao(FunctionElementId),
    Classe(ClassId),
    Extensao(ExtensionId),
    Typedef(dartforge_elements::model::TypedefId),
    ParametroDeTipo(usize),
    Rotulo(usize),
    Prefixo(LibraryId, SymbolId),
}

/// A dona de um par de acessores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dono2 {
    Classe(ClassId),
    Extensao(ExtensionId),
    Biblioteca(LibraryId),
}

impl Projeto {
    /// Os destaques de `offset` em `unidade`, na ordem dos grupos e dos
    /// offsets.
    pub(crate) fn destaques_do_dart(&self, unidade: UnitId, offset: usize) -> Vec<Span> {
        let cx = Contexto::novo(self, unidade);
        let mut grupos: Vec<(Chave, usize, Vec<usize>)> = Vec::new();
        let mut registrar = |chave: Chave, comprimento: usize, o: usize| match grupos.iter_mut().find(|(k, _, _)| *k == chave) {
            Some((_, _, v)) => v.push(o),
            None => grupos.push((chave, comprimento, vec![o])),
        };
        // Pré-ordem: a declaração registra o nome antes dos filhos.
        let mut pilha = vec![0usize];
        while let Some(n) = pilha.pop() {
            for (chave, comprimento, o) in registros(self, &cx, n) {
                registrar(chave, comprimento, o);
            }
            pilha.extend(cx.filhos(n).iter().rev().copied());
        }
        let mut saida: Vec<Span> = Vec::new();
        for (_, comprimento, offsets) in &grupos {
            if !offsets.iter().any(|&o| o <= offset && offset <= o + comprimento) {
                continue;
            }
            for &o in offsets {
                let s = Span { start: o, end: o + comprimento };
                if !saida.contains(&s) {
                    saida.push(s);
                }
            }
        }
        saida
    }
}

/// O nome declarado (token) e o offset dele.
fn nome_em(cx: &Contexto<'_>, inicio: usize, nome: &str) -> Option<usize> {
    let fim = cx.arvore.nos.iter().map(|k| k.fim).max().unwrap_or(cx.fonte.len());
    let resto = &cx.fonte[inicio..fim.min(cx.fonte.len())];
    let mut i = 0;
    while let Some(k) = resto[i..].find(nome) {
        let a = i + k;
        let antes = resto[..a].chars().next_back();
        let depois = resto[a + nome.len()..].chars().next();
        let limite = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$'));
        if limite(antes) && limite(depois) {
            return Some(inicio + a);
        }
        i = a + nome.len();
    }
    None
}

/// Os registros (`chave`, `nameLength`, offset) que o nó `n` faz.
fn registros(p: &Projeto, cx: &Contexto<'_>, n: usize) -> Vec<(Chave, usize, usize)> {
    let no = &cx.arvore.nos[n];
    let mut v = Vec::new();
    match no.especie {
        "ClassDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration" => {
            if let Marca::Decl(d) = no.marca
                && let Some(c) = cx.classe_da_declaracao(cx.unidade, d)
                && let Some(nome) = cx.nome_da_declaracao(d)
            {
                v.push((Chave::Classe(c), nome.span.end - nome.span.start, nome.span.start));
            }
        }
        "FunctionDeclaration" | "MethodDeclaration" => {
            if let Marca::Funcao(fid) = no.marca
                && let Some(nome) = cx.ast.function(fid).name
            {
                let local = no.especie == "FunctionDeclaration" && cx.pai(n).is_some_and(|x| cx.especie(x) == "FunctionDeclarationStatement");
                let chave = if local {
                    Some(Chave::Local(nome.span.start))
                } else {
                    p.funcao_do_no(cx.unidade, fid).and_then(|f| chave_de_funcao(p, f))
                };
                if let Some(k) = chave {
                    v.push((k, nome.span.end - nome.span.start, nome.span.start));
                }
            }
        }
        "VariableDeclaration" => {
            let nome = crate::refatoracoes_exec::nome_no_inicio_pub(cx.texto_do_no(n));
            let chave = if no.marca == Marca::VariavelLocal {
                Some(Chave::Local(no.inicio))
            } else {
                variavel_declarada_em(p, cx.unidade, no.inicio).map(Chave::Variavel)
            };
            if let Some(k) = chave {
                v.push((k, nome.len(), no.inicio));
            }
        }
        "DeclaredIdentifier" | "DeclaredVariablePattern" | "SuperFormalParameter" | "SimpleFormalParameter" | "FieldFormalParameter" | "EnumConstantDeclaration" => {
            // O nome: o último identificador antes de `=`/`:`/`(` (numa
            // constante de enum, o primeiro).
            let declarado = if no.especie == "EnumConstantDeclaration" { primeiro_identificador(cx, n) } else { nome_declarado(cx, n) };
            if let Some((o, nome)) = declarado {
                let chave = match no.especie {
                    "EnumConstantDeclaration" => variavel_declarada_em(p, cx.unidade, o).map(Chave::Variavel),
                    "FieldFormalParameter" => campo_do_this(p, cx, n, &nome).map(Chave::Variavel),
                    _ => Some(Chave::Local(o)),
                };
                if let Some(k) = chave {
                    v.push((k, nome.len(), o));
                }
            }
        }
        "ConstructorDeclaration" => {
            // Só o construtor com nome (o nome escrito depois do ponto).
            if let Some(f) = construtor_declarado_em(p, cx.unidade, no.inicio, no.fim)
                && !p.nome(p.programa().function(f).name).is_empty()
                && let Some((_, s)) = p.nome_da_funcao(f)
            {
                v.push((Chave::Funcao(f), s.end - s.start, s.start));
            }
        }
        "NamedType" => {
            if let Some((k, comprimento)) = elemento_do_tipo(p, cx, n) {
                v.push((k, comprimento, no.inicio));
            }
        }
        "SimpleIdentifier" => {
            if let Some(k) = chave_do_identificador(p, cx, n) {
                let comprimento = comprimento_do_nome(p, &k, no.fim - no.inicio);
                v.push((k, comprimento, no.inicio));
            }
        }
        _ => {}
    }
    v
}

/// O comprimento do nome do elemento (`element.nameLength`).
fn comprimento_do_nome(p: &Projeto, k: &Chave, padrao: usize) -> usize {
    match k {
        Chave::Funcao(f) => {
            let fe = p.programa().function(*f);
            if matches!(fe.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
                let nome = p.nome(fe.name);
                if nome.is_empty() {
                    // O construtor sem nome: o nome da classe escrito, ou 0.
                    return match (fe.kind, fe.class) {
                        (FunctionKind::Constructor, Some(c)) => p.nome(p.programa().class(c).name).len(),
                        _ => 0,
                    };
                }
                return nome.len();
            }
            p.nome(fe.name).trim_end_matches('=').len()
        }
        _ => padrao,
    }
}

/// A chave de um elemento função (acessor implícito → a variável; getter
/// ou setter declarado → o par; o resto, a própria função).
fn chave_de_funcao(p: &Projeto, f: FunctionElementId) -> Option<Chave> {
    let prog = p.programa();
    let fe = prog.function(f);
    match fe.kind {
        FunctionKind::ImplicitAccessor => fe.variable.map(Chave::Variavel),
        FunctionKind::Getter | FunctionKind::Setter => {
            let dono = match (fe.class, fe.extension) {
                (_, Some(x)) => Dono2::Extensao(x),
                (Some(c), _) => Dono2::Classe(c),
                _ => Dono2::Biblioteca(fe.library),
            };
            Some(Chave::Acessores(dono, p.nome(fe.name).trim_end_matches('=').to_string()))
        }
        _ => Some(Chave::Funcao(f)),
    }
}

/// A chave de um `SimpleIdentifier`: pela resolução da expressão marcada,
/// ou pelo `identificar` (combinadores, anotações, documentação, rótulos de
/// argumento, o `returnType` do construtor), ou o rótulo de um comando.
fn chave_do_identificador(p: &Projeto, cx: &Contexto<'_>, n: usize) -> Option<Chave> {
    let no = &cx.arvore.nos[n];
    // Rótulos de comando e `break`/`continue`.
    if let Some(pai) = cx.pai(n) {
        if cx.especie(pai) == "Label" && cx.pai(pai).is_some_and(|g| matches!(cx.especie(g), "LabeledStatement" | "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")) {
            return Some(Chave::Rotulo(no.inicio));
        }
        if matches!(cx.especie(pai), "BreakStatement" | "ContinueStatement") {
            let nome = cx.texto_do_no(n);
            for k in cx.com_pais(pai) {
                for &l in cx.filhos(k) {
                    if cx.especie(l) == "Label"
                        && let Some(&i) = cx.filhos(l).first()
                        && cx.texto_do_no(i) == nome
                    {
                        return Some(Chave::Rotulo(cx.arvore.nos[i].inicio));
                    }
                }
            }
            return None;
        }
    }
    if let Marca::Expr(x) = no.marca {
        return match cx.corpos.get_resolved(x) {
            Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => {
                let d = cx.corpos.declaracao_local(x)?;
                // `this.x` → o campo.
                if let Some((q, _)) = crate::projeto::parametro_em(cx.ast, d)
                    && q.this_
                {
                    let nome = p.nome(q.name?.sym).to_string();
                    return campo_da_classe_em(p, cx, d, &nome).map(Chave::Variavel);
                }
                Some(Chave::Local(d))
            }
            Some(Resolved::Element(Element::Function(f)))
            | Some(Resolved::Member { member: MemberRef::Function(f), .. })
            | Some(Resolved::ExtensionMember { member: f, .. }) => {
                // Numa escrita, o setter (a mesma variável/par).
                chave_de_funcao(p, *f)
            }
            Some(Resolved::Element(Element::Variable(v))) | Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => Some(Chave::Variavel(*v)),
            Some(Resolved::Element(Element::Class(c))) => Some(Chave::Classe(*c)),
            Some(Resolved::Element(Element::Extension(e))) => Some(Chave::Extensao(*e)),
            Some(Resolved::Element(Element::Typedef(t))) => Some(Chave::Typedef(*t)),
            Some(Resolved::Element(Element::Prefix(l, s))) => Some(Chave::Prefixo(*l, *s)),
            Some(Resolved::Prefix(_)) => {
                let ast::ExprKind::Identifier(nome) = &cx.ast.expr(x).kind else { return None };
                Some(Chave::Prefixo(p.programa().unit(cx.unidade).library, nome.sym))
            }
            Some(Resolved::TypeParameter(_)) => {
                let ast::ExprKind::Identifier(nome) = &cx.ast.expr(x).kind else { return None };
                crate::projeto::declaracao_de_parametro_de_tipo(cx.ast, nome.span.start, nome.sym).map(Chave::ParametroDeTipo)
            }
            Some(Resolved::Constructor(f)) => Some(Chave::Funcao(*f)),
            None if cx.construtores_por_alvo.contains_key(&x) => {
                let f = cx.construtores_por_alvo[&x];
                Some(Chave::Funcao(f))
            }
            _ => None,
        };
    }
    // Pelo `identificar`.
    let d = p.identificar(cx.unidade, no.inicio).ok()??;
    if d.nome.start != no.inicio {
        return None;
    }
    match &d.alvo {
        Alvo::Local { declaracao, .. } => Some(Chave::Local(*declaracao)),
        Alvo::ParametroDeTipo { declaracao, .. } => Some(Chave::ParametroDeTipo(*declaracao)),
        Alvo::Prefixo { biblioteca, nome } => Some(Chave::Prefixo(*biblioteca, *nome)),
        Alvo::Topo(Element::Class(c)) => Some(Chave::Classe(*c)),
        Alvo::Topo(Element::Extension(e)) => Some(Chave::Extensao(*e)),
        Alvo::Topo(Element::Typedef(t)) => Some(Chave::Typedef(*t)),
        Alvo::Topo(Element::Variable(v)) => Some(Chave::Variavel(*v)),
        Alvo::Topo(Element::Function(f)) => chave_de_funcao(p, *f),
        Alvo::Topo(Element::Prefix(l, s)) => Some(Chave::Prefixo(*l, *s)),
        Alvo::Construtor(f) => Some(Chave::Funcao(*f)),
        Alvo::Membro { .. } => match d.concreto {
            Some(Concreto::Funcao(f)) => chave_de_funcao(p, f),
            Some(Concreto::Variavel(v)) => Some(Chave::Variavel(v)),
            None => None,
        },
    }
}

/// O elemento de um `NamedType` (pelo nome escrito, no escopo da unidade ou
/// de um parâmetro de tipo), com o comprimento do nome do elemento.
fn elemento_do_tipo(p: &Projeto, cx: &Contexto<'_>, n: usize) -> Option<(Chave, usize)> {
    let prog = p.programa();
    let no = &cx.arvore.nos[n];
    let prefixo = cx.filhos(n).iter().copied().find(|&k| cx.especie(k) == "ImportPrefixReference");
    let inicio_do_nome = prefixo.map_or(no.inicio, |k| cx.arvore.nos[k].fim);
    let resto = cx.fonte[inicio_do_nome..no.fim].trim_start();
    let fim = resto.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(resto.len());
    let nome = &resto[..fim];
    if nome.is_empty() || nome == "dynamic" || nome == "void" {
        return None;
    }
    let s = p.consulta.nomes.lookup(nome)?;
    let vinculo = match prefixo {
        Some(k) => {
            let t = cx.texto_do_no(k);
            let pf = t.trim_end_matches('.').trim();
            let ps = p.consulta.nomes.lookup(pf)?;
            prog.lookup_prefixed_na_unidade(cx.unidade, ps, s)
        }
        None => {
            // Um parâmetro de tipo em escopo vem antes do escopo da unidade.
            if let Some(d) = crate::projeto::declaracao_de_parametro_de_tipo(cx.ast, no.inicio, s) {
                return Some((Chave::ParametroDeTipo(d), nome.len()));
            }
            prog.lookup_na_unidade(cx.unidade, s)
        }
    }?;
    let chave = match vinculo.getter? {
        Element::Class(c) => Chave::Classe(c),
        Element::Typedef(t) => Chave::Typedef(t),
        Element::Extension(e) => Chave::Extensao(e),
        _ => return None,
    };
    Some((chave, nome.len()))
}

/// O nome declarado de um parâmetro ou de uma declaração simples: o offset
/// e o texto.
fn nome_declarado(cx: &Contexto<'_>, n: usize) -> Option<(usize, String)> {
    let no = &cx.arvore.nos[n];
    let t = cx.texto_do_no(n);
    // Sem os filhos (tipo, anotações, valor padrão): o primeiro trecho de
    // identificador fora deles, o último antes de `=`, `:`, `(`.
    let mut melhor: Option<(usize, String)> = None;
    let mut i = 0;
    let b = t.as_bytes();
    while i < b.len() {
        let c = b[i];
        let dentro_de_filho = cx.filhos(n).iter().any(|&f| {
            let s = &cx.arvore.nos[f];
            s.inicio <= no.inicio + i && no.inicio + i < s.fim
        });
        if dentro_de_filho {
            i += 1;
            continue;
        }
        if c == b'=' || c == b':' || c == b'(' {
            break;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let ini = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            let palavra = &t[ini..i];
            if !matches!(palavra, "final" | "var" | "const" | "late" | "covariant" | "required" | "this" | "super") {
                melhor = Some((no.inicio + ini, palavra.to_string()));
            }
            continue;
        }
        i += 1;
    }
    melhor
}

/// O primeiro identificador fora dos filhos (o nome de uma constante de
/// enum, depois das anotações).
fn primeiro_identificador(cx: &Contexto<'_>, n: usize) -> Option<(usize, String)> {
    let no = &cx.arvore.nos[n];
    let t = cx.texto_do_no(n);
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let dentro = cx.filhos(n).iter().any(|&f| {
            let s = &cx.arvore.nos[f];
            s.inicio <= no.inicio + i && no.inicio + i < s.fim
        });
        if !dentro && (b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] == b'$') {
            let ini = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            return Some((no.inicio + ini, t[ini..i].to_string()));
        }
        i += 1;
    }
    None
}

/// O campo de `this.x` (o `FieldFormalParameter` em `n`).
fn campo_do_this(p: &Projeto, cx: &Contexto<'_>, n: usize, nome: &str) -> Option<VariableId> {
    campo_da_classe_em(p, cx, cx.arvore.nos[n].inicio, nome)
}

/// O campo `nome` da classe que contém o offset.
fn campo_da_classe_em(p: &Projeto, cx: &Contexto<'_>, offset: usize, nome: &str) -> Option<VariableId> {
    let prog = p.programa();
    let c = prog.classes.iter().position(|c| {
        c.decl.is_some_and(|d| {
            d.unit == cx.unidade && {
                let s = cx.ast.decl(d.decl).span;
                s.start <= offset && offset <= s.end
            }
        })
    })?;
    prog.class(ClassId(c as u32)).fields.iter().copied().find(|v| p.nome(prog.variable(*v).name) == nome)
}

/// A variável (de topo, campo ou constante de enum) cujo nome começa em
/// `offset` na unidade.
pub(crate) fn variavel_declarada_em(p: &Projeto, u: UnitId, offset: usize) -> Option<VariableId> {
    let prog = p.programa();
    (0..prog.variables.len()).map(|i| VariableId(i as u32)).find(|&v| p.nome_da_variavel(v).is_some_and(|(uu, s)| uu == u && s.start == offset))
}

/// O construtor declarado no intervalo.
fn construtor_declarado_em(p: &Projeto, u: UnitId, ini: usize, fim: usize) -> Option<FunctionElementId> {
    let prog = p.programa();
    let unidade = prog.unit(u);
    let (mi, _) = unidade.ast.members.iter().enumerate().find(|(_, m)| matches!(m.kind, ast::MemberKind::Constructor(_)) && m.span.start >= ini && m.span.end <= fim)?;
    p.construtor_do_no(u, ast::MemberId(mi as u32))
}
