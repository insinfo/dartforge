//! A árvore no formato do analyzer 6.11 (a unidade só parseada) e o
//! `NodeLocator` (`AN:src/dart/ast/utilities.dart:1733-1870`), construídos
//! sobre a árvore do parser do DartForge e os tokens do texto
//! (docs/LSP-ESPECIFICACAO.md §7.2 a §7.6).
//!
//! Cada nó tem o intervalo do analyzer (`beginToken.offset` a
//! `endToken.end`) e os filhos na ordem do `visitChildren` (a que decide o
//! "primeiro que cobre"). Os nomes declarados são tokens, não nós; as
//! referências e os identificadores que o analyzer guarda como nó
//! (`returnType` de construtor, prefixo de import, nomes de `show`/`hide`,
//! nome de anotação, rótulos) são `SimpleIdentifier`. Os nós que a árvore do
//! parser não tem (`FormalParameterList`, `VariableDeclarationList`,
//! `TypeArgumentList`, corpos de função, cláusulas, partes de `for`,
//! elementos de coleção, combinadores, `Comment`…) são sintetizados pelos
//! tokens. A forma é a não resolvida: `A()` é `MethodInvocation`.
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Annotation, Arguments, Ast, AsyncModifier, CollectionElement, Combinator, CompilationUnit, DeclKind, DirectiveKind, ExprId,
    ExprKind, ForInTarget, ForInit, FunctionBody, Initializer, ListPatternElement, MemberKind, Parameter, ParameterKind, PatternId,
    PatternKind, StmtId, StmtKind, StringLit, TypeId, TypeKind, TypeParameter, UnaryOp,
};
use dartforge_frontend::comentarios::Comentarios;
use dartforge_frontend::token::{Kind, Op, Token};
use std::collections::HashMap;

/// De onde o nó veio na árvore do parser (para achar o elemento dele).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marca {
    Nenhuma,
    /// Um `SimpleIdentifier` cujo elemento é o resolvido nesta expressão
    /// (o próprio identificador, a propriedade de que é o nome, o alvo da
    /// invocação de que é o `methodName`).
    Expr(ExprId),
    /// `FunctionDeclaration`, `MethodDeclaration` ou `FunctionExpression`
    /// desta função.
    Funcao(ast::FunctionId),
    /// `VariableDeclaration` de uma variável local.
    VariavelLocal,
    /// Declaração de topo.
    Decl(ast::DeclId),
    /// `SimpleIdentifier` em contexto de declaração (`inDeclarationContext`:
    /// o prefixo `as p` e o rótulo de um comando).
    ContextoDeDeclaracao,
    /// `PrefixExpression` desta expressão (o `staticElement` é o operador).
    Operador(ExprId),
}

/// A entidade do AST do DartForge que o nó representa (o completar lê as
/// partes dela). As que não têm id na árvore do parser vão pelo endereço
/// (só comparado, nunca seguido).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ligacao {
    Nenhuma,
    Expr(ExprId),
    Stmt(StmtId),
    Decl(ast::DeclId),
    Membro(ast::MemberId),
    Funcao(ast::FunctionId),
    Tipo(TypeId),
    Padrao(PatternId),
    /// `ast::Parameter`.
    Parametro(usize),
    /// `ast::Initializer`.
    Inicializador(usize),
    /// `ast::EnumConstant`.
    ConstanteDeEnum(usize),
    /// `ast::SwitchCase`.
    Caso(usize),
    /// `ast::Directive`.
    Diretiva(usize),
    /// `ast::CatchClause`.
    Catch(usize),
    /// `ast::Arguments`.
    Argumentos(usize),
    /// `ast::CollectionElement`.
    Elemento(usize),
    /// `ast::Combinator`.
    Combinador(usize),
    /// `ast::Annotation`.
    Anotacao(usize),
    /// `ast::TypeParameter`.
    ParametroDeTipo(usize),
    /// O `SwitchExpressionCase` de índice `.1` da expressão `switch` `.0`.
    CasoDeSwitchExpr(ExprId, usize),
}

/// O endereço de uma entidade do AST, para a [`Ligacao`].
pub fn endereco<T>(x: &T) -> usize {
    x as *const T as usize
}

/// Um nó do analyzer.
#[derive(Debug, Clone)]
pub struct No {
    pub especie: &'static str,
    pub inicio: usize,
    pub fim: usize,
    pub filhos: Vec<usize>,
    pub pai: Option<usize>,
    /// A posição do fim do nome que o `NodeLocator` sobrescreve (classe,
    /// função, método, construtor).
    pub sobrescrita: Option<usize>,
    pub marca: Marca,
    pub ligacao: Ligacao,
}

/// As partes de um `ForParts` (`initialization`/`variables`, `condition`,
/// `updaters`), pelos nós.
#[derive(Debug, Clone, Default)]
pub struct PartesDeFor {
    pub inicio: Option<usize>,
    pub condicao: Option<usize>,
    pub atualizacoes: Vec<usize>,
}

/// A árvore; o nó 0 é a `CompilationUnit`.
pub struct Arvore {
    pub nos: Vec<No>,
    /// As partes de cada `ForPartsWith*`, pelo nó.
    pub partes_de_for: HashMap<usize, PartesDeFor>,
}

impl Arvore {
    /// `NodeLocator(ini, fim).searchWithin(unit)`.
    pub fn localizar(&self, ini: usize, fim: usize) -> Option<usize> {
        let mut achado = None;
        self.visitar(0, ini, fim, &mut achado);
        achado
    }

    fn visitar(&self, n: usize, ini: usize, fim: usize, achado: &mut Option<usize>) {
        if achado.is_some() {
            return;
        }
        let no = &self.nos[n];
        if let Some(s) = no.sobrescrita
            && ini == fim
            && ini == s
        {
            *achado = Some(n);
            return;
        }
        if no.fim < ini || no.inicio > fim {
            return;
        }
        for &f in &no.filhos {
            self.visitar(f, ini, fim, achado);
            if achado.is_some() {
                return;
            }
        }
        if no.inicio <= ini && fim <= no.fim {
            *achado = Some(n);
        }
    }

    /// `NodeLocator2` (fim exclusivo): o nó mais profundo com
    /// `inicio <= pos < fim`, primeiro filho que cobre, com as sobrescritas de
    /// `name.end`.
    pub fn localizar_exclusivo(&self, pos: usize) -> Option<usize> {
        let mut achado = None;
        self.visitar_exclusivo(0, pos, &mut achado);
        achado
    }

    fn visitar_exclusivo(&self, n: usize, pos: usize, achado: &mut Option<usize>) {
        if achado.is_some() {
            return;
        }
        let no = &self.nos[n];
        if let Some(s) = no.sobrescrita
            && pos == s
        {
            *achado = Some(n);
            return;
        }
        if no.fim <= pos || no.inicio > pos {
            return;
        }
        for &f in &no.filhos {
            self.visitar_exclusivo(f, pos, achado);
            if achado.is_some() {
                return;
            }
        }
        if no.inicio <= pos && pos < no.fim {
            *achado = Some(n);
        }
    }

    /// `NodeLocator2(ini, fim)`: o nó mais profundo com
    /// `inicio <= ini` e `fim < no.fim` (o `coveringNode` das correções).
    pub fn localizar2(&self, ini: usize, fim: usize) -> Option<usize> {
        let mut achado = None;
        self.visitar2(0, ini, fim, &mut achado);
        achado
    }

    fn visitar2(&self, n: usize, ini: usize, fim: usize, achado: &mut Option<usize>) {
        if achado.is_some() {
            return;
        }
        let no = &self.nos[n];
        if no.fim <= ini || no.inicio > fim {
            return;
        }
        for &f in &no.filhos {
            self.visitar2(f, ini, fim, achado);
            if achado.is_some() {
                return;
            }
        }
        if no.inicio <= ini && fim < no.fim {
            *achado = Some(n);
        }
    }

    /// O nó e os ancestrais, sem a `CompilationUnit`.
    pub fn cadeia(&self, n: usize) -> Vec<usize> {
        let mut v = Vec::new();
        let mut atual = Some(n);
        while let Some(k) = atual {
            if k == 0 {
                break;
            }
            v.push(k);
            atual = self.nos[k].pai;
        }
        v
    }

    pub fn span(&self, n: usize) -> Span {
        Span { start: self.nos[n].inicio, end: self.nos[n].fim }
    }
}

/// Os tokens do texto, com o par de `( ) [ ] { }`.
struct Toks {
    lista: Vec<Token>,
    par: HashMap<usize, usize>,
}

impl Toks {
    fn novos(fonte: &str) -> Toks {
        let lista = dartforge_frontend::lexer::lex(fonte).unwrap_or_default();
        let mut par = HashMap::new();
        let mut pilha: Vec<(Op, usize)> = Vec::new();
        for (i, t) in lista.iter().enumerate() {
            if let Kind::Op(op) = t.kind {
                match op {
                    Op::LParen | Op::LBracket | Op::LBrace => pilha.push((op, i)),
                    Op::RParen | Op::RBracket | Op::RBrace => {
                        let abre = match op {
                            Op::RParen => Op::LParen,
                            Op::RBracket => Op::LBracket,
                            _ => Op::LBrace,
                        };
                        if let Some(pos) = pilha.iter().rposition(|(o, _)| *o == abre) {
                            let (_, j) = pilha[pos];
                            pilha.truncate(pos);
                            par.insert(j, i);
                            par.insert(i, j);
                        }
                    }
                    _ => {}
                }
            }
        }
        Toks { lista, par }
    }

    /// O primeiro token que começa em `off` ou depois.
    fn indice(&self, off: usize) -> usize {
        self.lista.partition_point(|t| t.span.start < off)
    }

    /// O último token que termina até `off`.
    fn antes(&self, off: usize) -> Option<usize> {
        let k = self.lista.partition_point(|t| t.span.end <= off);
        k.checked_sub(1)
    }

    /// O primeiro token que começa em `off` ou depois (fora o EOF).
    fn depois(&self, off: usize) -> Option<usize> {
        let i = self.indice(off);
        self.lista.get(i).filter(|t| t.kind != Kind::Eof).map(|_| i)
    }

    fn span(&self, i: usize) -> Span {
        self.lista[i].span
    }

    fn e_op(&self, i: usize, op: Op) -> bool {
        self.lista.get(i).is_some_and(|t| t.kind == Kind::Op(op))
    }

    fn e_palavra(&self, i: usize, fonte: &str, palavra: &str) -> bool {
        self.lista.get(i).is_some_and(|t| matches!(t.kind, Kind::Ident | Kind::Keyword(_)) && t.text(fonte) == palavra)
    }

    /// O token imediatamente antes de `off`, se for `op`.
    fn op_antes(&self, off: usize, op: Op) -> Option<usize> {
        self.antes(off).filter(|&i| self.e_op(i, op))
    }

    /// O token imediatamente depois de `off`, se for `op`.
    fn op_depois(&self, off: usize, op: Op) -> Option<usize> {
        self.depois(off).filter(|&i| self.e_op(i, op))
    }

    /// O último token `op` que termina até `ate` e começa em `de` ou depois.
    fn ultimo_op(&self, op: Op, de: usize, ate: usize) -> Option<usize> {
        let i0 = self.indice(de);
        let i1 = self.lista.partition_point(|t| t.span.end <= ate);
        (i0..i1).rev().find(|&i| self.e_op(i, op))
    }

    /// O primeiro token `op` em `[de, ate)`.
    fn primeiro_op(&self, op: Op, de: usize, ate: usize) -> Option<usize> {
        let mut i = self.indice(de);
        while let Some(t) = self.lista.get(i) {
            if t.span.start >= ate || t.kind == Kind::Eof {
                return None;
            }
            if t.kind == Kind::Op(op) {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// O par do token `i`.
    fn par_de(&self, i: usize) -> Option<usize> {
        self.par.get(&i).copied()
    }
}

struct Construtor<'a> {
    a: &'a Ast,
    fonte: &'a str,
    t: Toks,
    comentarios: Comentarios,
    nos: Vec<No>,
    /// A versão de linguagem é anterior à 3.0 (o `case e:` é `SwitchCase`).
    antes_de_3: bool,
    /// Os metadados dos comandos (variáveis e funções locais).
    metadados_locais: HashMap<u32, &'a [Annotation]>,
    /// A forma resolvida (a das refatorações): as chamadas sem `new` que
    /// invocam construtor (`A()`, `A.n()`, `p.A()`, `p.A.n()`) viram
    /// `InstanceCreationExpression`, como no `AstRewriter.methodInvocation`.
    construtores: Option<&'a std::collections::HashSet<ExprId>>,
    /// Os identificadores que são prefixo de import (forma resolvida).
    prefixos: Option<&'a std::collections::HashSet<ExprId>>,
    partes_de_for: HashMap<usize, PartesDeFor>,
}

/// Monta a árvore do texto já analisado.
pub fn construir(fonte: &str, a: &Ast, unidade: &CompilationUnit, antes_de_3: bool) -> Arvore {
    construir_com(fonte, a, unidade, antes_de_3, None, None)
}

/// A árvore na forma resolvida: `construtores` são as chamadas (`Call` ou
/// `InstanceCreation` sem palavra-chave) que invocam construtor, e
/// `prefixos` os identificadores que denotam prefixo de import.
pub fn construir_resolvida(
    fonte: &str,
    a: &Ast,
    unidade: &CompilationUnit,
    antes_de_3: bool,
    construtores: &std::collections::HashSet<ExprId>,
    prefixos: &std::collections::HashSet<ExprId>,
) -> Arvore {
    construir_com(fonte, a, unidade, antes_de_3, Some(construtores), Some(prefixos))
}

fn construir_com<'a>(
    fonte: &'a str,
    a: &'a Ast,
    unidade: &'a CompilationUnit,
    antes_de_3: bool,
    construtores: Option<&'a std::collections::HashSet<ExprId>>,
    prefixos: Option<&'a std::collections::HashSet<ExprId>>,
) -> Arvore {
    let mut c = Construtor {
        a,
        fonte,
        t: Toks::novos(fonte),
        comentarios: Comentarios::de(fonte),
        nos: Vec::new(),
        antes_de_3,
        metadados_locais: a.metadados_locais.iter().map(|(s, m)| (s.0, &m[..])).collect(),
        construtores,
        prefixos,
        partes_de_for: HashMap::new(),
    };
    // A raiz primeiro (índice 0), os filhos depois.
    c.nos.push(No { especie: "CompilationUnit", inicio: 0, fim: fonte.len(), filhos: Vec::new(), pai: None, sobrescrita: None, marca: Marca::Nenhuma, ligacao: Ligacao::Nenhuma });
    let mut filhos = Vec::new();
    if let Some(s) = unidade.script_tag {
        filhos.extend(c.folha("ScriptTag", s));
    }
    // Diretivas e declarações em ordem de fonte.
    let mut itens: Vec<(usize, bool, usize)> = Vec::new();
    for (i, d) in unidade.directives.iter().enumerate() {
        itens.push((d.span.start, true, i));
    }
    for (i, d) in unidade.declarations.iter().enumerate() {
        itens.push((a.decl(*d).span.start, false, i));
    }
    itens.sort();
    for (_, diretiva, i) in itens {
        let n = if diretiva { c.diretiva(&unidade.directives[i]) } else { c.declaracao(unidade.declarations[i]) };
        filhos.extend(n);
    }
    c.ligar(0, filhos);
    Arvore { nos: c.nos, partes_de_for: c.partes_de_for }
}

impl<'a> Construtor<'a> {
    // -- Infraestrutura --------------------------------------------------------

    /// Um nó com os filhos dados; nenhum nó vazio.
    fn no(&mut self, especie: &'static str, inicio: usize, fim: usize, filhos: Vec<usize>) -> Option<usize> {
        if fim <= inicio {
            return None;
        }
        let id = self.nos.len();
        self.nos.push(No { especie, inicio, fim, filhos: Vec::new(), pai: None, sobrescrita: None, marca: Marca::Nenhuma, ligacao: Ligacao::Nenhuma });
        self.ligar(id, filhos);
        Some(id)
    }

    fn ligar(&mut self, pai: usize, filhos: Vec<usize>) {
        for &f in &filhos {
            self.nos[f].pai = Some(pai);
        }
        self.nos[pai].filhos = filhos;
    }

    fn folha(&mut self, especie: &'static str, s: Span) -> Option<usize> {
        self.no(especie, s.start, s.end, Vec::new())
    }

    fn identificador(&mut self, n: ast::Name) -> Option<usize> {
        // O identificador sintético da recuperação (`b.` antes de `}`) fica
        // na árvore com comprimento zero, como no analyzer.
        if n.span.start == n.span.end {
            let id = self.nos.len();
            self.nos.push(No {
                especie: "SimpleIdentifier",
                inicio: n.span.start,
                fim: n.span.end,
                filhos: Vec::new(),
                pai: None,
                sobrescrita: None,
                marca: Marca::Nenhuma,
                ligacao: Ligacao::Nenhuma,
            });
            return Some(id);
        }
        self.folha("SimpleIdentifier", n.span)
    }

    fn ligar_a(&mut self, n: Option<usize>, l: Ligacao) -> Option<usize> {
        if let Some(k) = n {
            self.nos[k].ligacao = l;
        }
        n
    }

    fn marcar(&mut self, n: Option<usize>, m: Marca) -> Option<usize> {
        if let Some(k) = n {
            self.nos[k].marca = m;
        }
        n
    }

    fn com_sobrescrita(&mut self, n: Option<usize>, pos: usize) -> Option<usize> {
        if let Some(k) = n {
            self.nos[k].sobrescrita = Some(pos);
        }
        n
    }

    // -- Comentário de documentação e anotações ---------------------------------

    /// O `Comment` de documentação antes de `pos` (o `findDartDoc`): o `/**`,
    /// ou a sequência de `///` a partir do primeiro.
    fn doc(&mut self, pos: usize) -> Option<(usize, usize)> {
        let inicio = self.comentarios.dart_doc(self.fonte, pos)?;
        let todos = self.comentarios.antes_de(self.fonte, pos);
        let texto = &self.fonte[inicio.start..inicio.end];
        let mut fim = inicio.end;
        if texto.starts_with("///") {
            let mut dentro = false;
            for s in todos {
                if s.start == inicio.start {
                    dentro = true;
                }
                if dentro {
                    if self.fonte[s.start..s.end].starts_with("///") {
                        fim = s.end;
                    } else {
                        break;
                    }
                }
            }
        }
        Some((inicio.start, fim))
    }

    /// O nó `Comment` com as `CommentReference`.
    fn no_de_comentario(&mut self, ini: usize, fim: usize) -> Option<usize> {
        let fonte = self.fonte;
        let mut refs = Vec::new();
        for (a, b) in referencias_de_doc(&fonte[ini..fim]) {
            let (a, b) = (ini + a, ini + b);
            let conteudo = &fonte[a..b];
            // `new A.b`: a expressão começa depois do `new`.
            let (inicio_expr, partes) = match conteudo.strip_prefix("new ") {
                Some(resto) => (b - resto.trim_start().len(), resto.trim_start()),
                None => (a, conteudo),
            };
            let mut nomes: Vec<Span> = Vec::new();
            let mut pos = inicio_expr;
            for p in partes.split('.') {
                nomes.push(Span { start: pos, end: pos + p.len() });
                pos += p.len() + 1;
            }
            let expr = match nomes.as_slice() {
                [x] => self.folha("SimpleIdentifier", *x),
                [x, y] => {
                    let fx = self.folha("SimpleIdentifier", *x);
                    let fy = self.folha("SimpleIdentifier", *y);
                    self.no("PrefixedIdentifier", x.start, y.end, fx.into_iter().chain(fy).collect())
                }
                [x, y, z] => {
                    let fx = self.folha("SimpleIdentifier", *x);
                    let fy = self.folha("SimpleIdentifier", *y);
                    let p = self.no("PrefixedIdentifier", x.start, y.end, fx.into_iter().chain(fy).collect());
                    let fz = self.folha("SimpleIdentifier", *z);
                    self.no("PropertyAccess", x.start, z.end, p.into_iter().chain(fz).collect())
                }
                _ => None,
            };
            refs.extend(self.no("CommentReference", a, b, expr.into_iter().collect()));
        }
        self.no("Comment", ini, fim, refs)
    }

    /// Os filhos `[doc/anot]` (em ordem de offset) e o início do nó: o
    /// `_findComment` do `AstBuilder` (antes do token depois das anotações;
    /// senão antes de cada anotação, da última à primeira).
    fn doc_e_anotacoes(&mut self, inicio: usize, metadata: &'a [Annotation]) -> (usize, Vec<usize>) {
        // O token depois das anotações (o da declaração).
        let apos = match metadata.last() {
            Some(m) => self.t.depois(m.span.end).map_or(inicio, |i| self.t.span(i).start),
            None => inicio,
        };
        let mut doc = self.doc(apos);
        if doc.is_none() {
            for m in metadata.iter().rev() {
                doc = self.doc(m.span.start);
                if doc.is_some() {
                    break;
                }
            }
        }
        let mut comeco = inicio;
        if let Some(m) = metadata.first() {
            comeco = comeco.min(m.span.start);
        }
        let mut itens: Vec<(usize, Option<usize>)> = Vec::new();
        if let Some((a, b)) = doc {
            comeco = comeco.min(a);
            itens.push((a, self.no_de_comentario(a, b)));
        }
        for m in metadata {
            itens.push((m.span.start, self.anotacao(m)));
        }
        itens.sort_by_key(|(p, _)| *p);
        (comeco, itens.into_iter().filter_map(|(_, n)| n).collect())
    }

    /// `Annotation`: `@` → argumentos ?? nome do construtor ?? nome.
    fn anotacao(&mut self, m: &'a Annotation) -> Option<usize> {
        let n = self.anotacao_bruta(m);
        self.ligar_a(n, Ligacao::Anotacao(endereco(m)))
    }

    fn anotacao_bruta(&mut self, m: &'a Annotation) -> Option<usize> {
        let mut filhos = Vec::new();
        // `@A<T>.b(…)`: o nome é `A` e o construtor `b`; `@p.A<T>(…)`: o nome
        // é `p.A`.
        let tipos_antes_do_segundo = m.name.len() == 2 && m.type_args.first().is_some_and(|t| self.a.ty(*t).span.start < m.name[1].span.start);
        let (nome, construtor): (&[ast::Name], Option<ast::Name>) = match m.name.len() {
            3 => (&m.name[..2], Some(m.name[2])),
            2 if tipos_antes_do_segundo => (&m.name[..1], Some(m.name[1])),
            _ => (&m.name[..], None),
        };
        match nome {
            [x] => filhos.extend(self.identificador(*x)),
            [x, y] => {
                let fx = self.identificador(*x);
                let fy = self.identificador(*y);
                filhos.extend(self.no("PrefixedIdentifier", x.span.start, y.span.end, fx.into_iter().chain(fy).collect()));
            }
            _ => {}
        }
        filhos.extend(self.argumentos_de_tipo(&m.type_args));
        if let Some(k) = construtor {
            filhos.extend(self.identificador(k));
        }
        if let Some(args) = &m.arguments {
            filhos.extend(self.lista_de_argumentos(args));
        }
        self.no("Annotation", m.span.start, m.span.end, filhos)
    }

    // -- Diretivas -------------------------------------------------------------

    fn literal_de_uri(&mut self, s: &'a StringLit) -> Option<usize> {
        self.string(s)
    }

    fn nomes_pontuados(&mut self, especie: &'static str, nomes: &[ast::Name]) -> Option<usize> {
        let (Some(p), Some(u)) = (nomes.first(), nomes.last()) else { return None };
        let filhos: Vec<usize> = nomes.iter().filter_map(|n| self.identificador(*n)).collect();
        self.no(especie, p.span.start, u.span.end, filhos)
    }

    fn configuracao(&mut self, c: &'a ast::Configuration) -> Option<usize> {
        let mut filhos = Vec::new();
        filhos.extend(self.nomes_pontuados("DottedName", &c.test));
        if let Some(v) = &c.value {
            filhos.extend(self.string(v));
        }
        filhos.extend(self.string(&c.uri));
        self.no("Configuration", c.span.start, c.span.end, filhos)
    }

    fn combinador(&mut self, c: &'a Combinator) -> Option<usize> {
        let n = self.combinador_bruta(c);
        self.ligar_a(n, Ligacao::Combinador(endereco(c)))
    }

    fn combinador_bruta(&mut self, c: &'a Combinator) -> Option<usize> {
        let (especie, nomes) = match c {
            Combinator::Show(n) => ("ShowCombinator", n),
            Combinator::Hide(n) => ("HideCombinator", n),
        };
        let (Some(p), Some(u)) = (nomes.first(), nomes.last()) else { return None };
        let palavra = self.t.antes(p.span.start).map_or(p.span.start, |i| self.t.span(i).start);
        let filhos: Vec<usize> = nomes.iter().filter_map(|n| self.identificador(*n)).collect();
        self.no(especie, palavra, u.span.end, filhos)
    }

    fn diretiva(&mut self, d: &'a ast::Directive) -> Option<usize> {
        let n = self.diretiva_bruta(d);
        self.ligar_a(n, Ligacao::Diretiva(endereco(d)))
    }

    fn diretiva_bruta(&mut self, d: &'a ast::Directive) -> Option<usize> {
        let (inicio, doc) = self.doc_e_anotacoes(d.span.start, &d.metadata);
        let mut filhos = Vec::new();
        let especie = match &d.kind {
            DirectiveKind::Library { name } => {
                filhos.extend(doc);
                filhos.extend(self.nomes_pontuados("LibraryIdentifier", name));
                "LibraryDirective"
            }
            DirectiveKind::Import { uri, configurations, prefix, combinators, .. } => {
                filhos.extend(doc);
                filhos.extend(self.literal_de_uri(uri));
                for c in configurations {
                    filhos.extend(self.configuracao(c));
                }
                if let Some(p) = prefix {
                    let id = self.identificador(*p);
                    filhos.extend(self.marcar(id, Marca::ContextoDeDeclaracao));
                }
                for c in combinators {
                    filhos.extend(self.combinador(c));
                }
                "ImportDirective"
            }
            DirectiveKind::Export { uri, configurations, combinators } => {
                for c in configurations {
                    filhos.extend(self.configuracao(c));
                }
                filhos.extend(doc);
                filhos.extend(self.literal_de_uri(uri));
                for c in combinators {
                    filhos.extend(self.combinador(c));
                }
                "ExportDirective"
            }
            DirectiveKind::Part { uri } => {
                filhos.extend(doc);
                filhos.extend(self.literal_de_uri(uri));
                "PartDirective"
            }
            DirectiveKind::PartOf { uri, name } => {
                filhos.extend(doc);
                filhos.extend(self.nomes_pontuados("LibraryIdentifier", name));
                if let Some(u) = uri {
                    filhos.extend(self.literal_de_uri(u));
                }
                "PartOfDirective"
            }
            DirectiveKind::ImportAugment { uri } | DirectiveKind::AugmentLibrary { uri } => {
                filhos.extend(doc);
                filhos.extend(self.literal_de_uri(uri));
                "AugmentationImportDirective"
            }
        };
        self.no(especie, inicio, d.span.end, filhos)
    }

    // -- Declarações -----------------------------------------------------------

    /// O token da palavra-chave antes do primeiro tipo de uma cláusula.
    fn clausula(&mut self, especie: &'static str, tipos: &[TypeId]) -> Option<usize> {
        let (Some(p), Some(u)) = (tipos.first(), tipos.last()) else { return None };
        let ps = self.a.ty(*p).span;
        let us = self.a.ty(*u).span;
        let palavra = self.t.antes(ps.start).map_or(ps.start, |i| self.t.span(i).start);
        let filhos: Vec<usize> = tipos.iter().filter_map(|t| self.tipo(*t)).collect();
        self.no(especie, palavra, us.end, filhos)
    }

    fn declaracao(&mut self, did: ast::DeclId) -> Option<usize> {
        let n = self.declaracao_sem_marca(did);
        let n = self.ligar_a(n, Ligacao::Decl(did));
        // A função de topo guarda a marca da função.
        match n {
            Some(k) if self.nos[k].marca == Marca::Nenhuma => self.marcar(n, Marca::Decl(did)),
            _ => n,
        }
    }

    fn declaracao_sem_marca(&mut self, did: ast::DeclId) -> Option<usize> {
        let a = self.a;
        let d = a.decl(did);
        let (inicio, mut filhos) = self.doc_e_anotacoes(d.span.start, &d.metadata);
        match &d.kind {
            DeclKind::Class(x) => {
                filhos.extend(self.parametros_de_tipo(&x.type_params));
                if x.mixin_application {
                    if let Some(s) = x.extends {
                        filhos.extend(self.tipo(s));
                    }
                    filhos.extend(self.clausula("WithClause", &x.with));
                    filhos.extend(self.clausula("ImplementsClause", &x.implements));
                    return self.no("ClassTypeAlias", inicio, d.span.end, filhos);
                }
                if let Some(s) = x.extends {
                    filhos.extend(self.clausula("ExtendsClause", &[s]));
                }
                filhos.extend(self.clausula("WithClause", &x.with));
                filhos.extend(self.clausula("ImplementsClause", &x.implements));
                for &m in &x.members {
                    filhos.extend(self.membro(m));
                }
                let n = self.no("ClassDeclaration", inicio, d.span.end, filhos);
                self.com_sobrescrita(n, x.name.span.end)
            }
            DeclKind::Mixin(x) => {
                filhos.extend(self.parametros_de_tipo(&x.type_params));
                filhos.extend(self.clausula("MixinOnClause", &x.on));
                filhos.extend(self.clausula("ImplementsClause", &x.implements));
                for &m in &x.members {
                    filhos.extend(self.membro(m));
                }
                self.no("MixinDeclaration", inicio, d.span.end, filhos)
            }
            DeclKind::Enum(x) => {
                filhos.extend(self.parametros_de_tipo(&x.type_params));
                filhos.extend(self.clausula("WithClause", &x.with));
                filhos.extend(self.clausula("ImplementsClause", &x.implements));
                for k in &x.constants {
                    filhos.extend(self.constante_de_enum(k));
                }
                for &m in &x.members {
                    filhos.extend(self.membro(m));
                }
                self.no("EnumDeclaration", inicio, d.span.end, filhos)
            }
            DeclKind::Extension(x) => {
                filhos.extend(self.parametros_de_tipo(&x.type_params));
                filhos.extend(self.clausula("ExtensionOnClause", &[x.on]));
                for &m in &x.members {
                    filhos.extend(self.membro(m));
                }
                self.no("ExtensionDeclaration", inicio, d.span.end, filhos)
            }
            DeclKind::ExtensionType(x) => {
                filhos.extend(self.parametros_de_tipo(&x.type_params));
                // `RepresentationDeclaration`: `.nome` ?? `(` → `)`.
                let mut rep = Vec::new();
                if let Some(c) = x.constructor {
                    let ponto = self.t.op_antes(c.span.start, Op::Dot).map_or(c.span.start, |i| self.t.span(i).start);
                    rep.extend(self.no("RepresentationConstructorName", ponto, c.span.end, Vec::new()));
                }
                for m in x.representation_metadata.iter() {
                    rep.extend(self.anotacao(m));
                }
                rep.extend(self.tipo(x.representation_type));
                filhos.extend(self.no("RepresentationDeclaration", x.representation_span.start, x.representation_span.end, rep));
                filhos.extend(self.clausula("ImplementsClause", &x.implements));
                for &m in &x.members {
                    filhos.extend(self.membro(m));
                }
                self.no("ExtensionTypeDeclaration", inicio, d.span.end, filhos)
            }
            DeclKind::Typedef(x) => match &x.kind {
                ast::TypedefKind::Alias(t) => {
                    filhos.extend(self.parametros_de_tipo(&x.type_params));
                    filhos.extend(self.tipo(*t));
                    self.no("GenericTypeAlias", inicio, d.span.end, filhos)
                }
                ast::TypedefKind::Legacy { return_type, parameters } => {
                    if let Some(r) = return_type {
                        filhos.extend(self.tipo(*r));
                    }
                    filhos.extend(self.parametros_de_tipo(&x.type_params));
                    filhos.extend(self.lista_de_parametros(parameters, x.name.span.end));
                    self.no("FunctionTypeAlias", inicio, d.span.end, filhos)
                }
            },
            DeclKind::Function(f) => {
                let n = self.declaracao_de_funcao(*f, inicio, filhos, true);
                let nome = a.function(*f).name.map(|n| n.span.end);
                match nome {
                    Some(p) => self.com_sobrescrita(n, p),
                    None => n,
                }
            }
            DeclKind::Variables(l) => {
                filhos.extend(self.lista_de_variaveis(l, d.span.start, &[]));
                self.no("TopLevelVariableDeclaration", inicio, d.span.end, filhos)
            }
        }
    }

    /// `FunctionDeclaration` (de topo ou local): `returnType`,
    /// `FunctionExpression`.
    fn declaracao_de_funcao(&mut self, fid: ast::FunctionId, inicio: usize, mut filhos: Vec<usize>, de_declaracao: bool) -> Option<usize> {
        let f = self.a.function(fid);
        if let Some(r) = f.return_type {
            filhos.extend(self.tipo(r));
        }
        filhos.extend(self.expressao_de_funcao(fid, de_declaracao));
        let n = self.no("FunctionDeclaration", inicio, f.span.end, filhos);
        let n = self.ligar_a(n, Ligacao::Funcao(fid));
        self.marcar(n, Marca::Funcao(fid))
    }

    /// `FunctionExpression`: parâmetros de tipo ?? parâmetros ?? corpo → fim
    /// do corpo.
    fn expressao_de_funcao(&mut self, fid: ast::FunctionId, de_declaracao: bool) -> Option<usize> {
        let f = self.a.function(fid);
        let depois_do_nome = f.name.map_or(f.span.start, |n| n.span.end);
        let mut filhos = Vec::new();
        let tps = self.parametros_de_tipo(&f.type_params);
        let depois_dos_tipos = tps.map_or(depois_do_nome, |n| self.nos[n].fim);
        filhos.extend(tps);
        let params = f.parameters.as_ref().and_then(|ps| self.lista_de_parametros(ps, depois_dos_tipos));
        filhos.extend(params);
        let corpo = self.corpo(&f.body, f.modifier, f.span.end, de_declaracao);
        filhos.extend(corpo);
        let inicio = filhos.first().map_or(f.span.start, |&n| self.nos[n].inicio);
        let fim = corpo.map_or(f.span.end, |n| self.nos[n].fim);
        let n = self.no("FunctionExpression", inicio, fim, filhos);
        let n = self.ligar_a(n, Ligacao::Funcao(fid));
        self.marcar(n, Marca::Funcao(fid))
    }

    /// O corpo de uma função.
    fn corpo(&mut self, b: &'a FunctionBody, modificador: AsyncModifier, fim_da_funcao: usize, de_declaracao: bool) -> Option<usize> {
        match b {
            FunctionBody::Block(s) => {
                let bloco = self.comando(*s);
                let bs = self.a.stmt(*s).span;
                let inicio = if modificador == AsyncModifier::None { bs.start } else { self.inicio_do_modificador(bs.start) };
                self.no("BlockFunctionBody", inicio, bs.end, bloco.into_iter().collect())
            }
            FunctionBody::Expression(e) => {
                let es = self.a.expr(*e).span;
                let seta = self.t.ultimo_op(Op::Arrow, 0, es.start);
                let inicio = match (modificador, seta) {
                    (AsyncModifier::None, Some(i)) => self.t.span(i).start,
                    (_, Some(i)) => self.inicio_do_modificador(self.t.span(i).start),
                    (_, None) => es.start,
                };
                let fim = if de_declaracao { self.t.op_depois(es.end, Op::Semicolon).map_or(es.end, |i| self.t.span(i).end) } else { es.end };
                let expr = self.expressao(*e);
                self.no("ExpressionFunctionBody", inicio, fim, expr.into_iter().collect())
            }
            FunctionBody::Empty => {
                let i = self.t.antes(fim_da_funcao).filter(|&i| self.t.e_op(i, Op::Semicolon))?;
                self.folha("EmptyFunctionBody", self.t.span(i))
            }
            FunctionBody::Native(s) => {
                let fim = self.t.antes(fim_da_funcao).filter(|&i| self.t.e_op(i, Op::Semicolon)).map_or(fim_da_funcao, |i| self.t.span(i).end);
                let lit = s.as_ref().and_then(|s| self.string(s));
                let native = match s {
                    Some(l) => self.t.antes(l.span.start).map_or(l.span.start, |i| self.t.span(i).start),
                    None => self.t.antes(fim).and_then(|i| self.t.antes(self.t.span(i).start)).map_or(fim, |i| self.t.span(i).start),
                };
                self.no("NativeFunctionBody", native, fim, lit.into_iter().collect())
            }
        }
    }

    /// O `async`/`sync` antes de `pos` (`async`, `async*`, `sync*`).
    fn inicio_do_modificador(&self, pos: usize) -> usize {
        let mut i = match self.t.antes(pos) {
            Some(i) => i,
            None => return pos,
        };
        if self.t.e_op(i, Op::Star)
            && let Some(j) = self.t.antes(self.t.span(i).start)
        {
            i = j;
        }
        if self.t.e_palavra(i, self.fonte, "async") || self.t.e_palavra(i, self.fonte, "sync") { self.t.span(i).start } else { pos }
    }

    fn constante_de_enum(&mut self, k: &'a ast::EnumConstant) -> Option<usize> {
        let n = self.constante_de_enum_bruta(k);
        self.ligar_a(n, Ligacao::ConstanteDeEnum(endereco(k)))
    }

    fn constante_de_enum_bruta(&mut self, k: &'a ast::EnumConstant) -> Option<usize> {
        let (inicio, mut filhos) = self.doc_e_anotacoes(k.span.start, &k.metadata);
        let mut args = Vec::new();
        let tas = self.argumentos_de_tipo(&k.type_args);
        let mut ini_args = tas.map(|n| self.nos[n].inicio);
        args.extend(tas);
        if let Some(c) = k.constructor {
            let ponto = self.t.op_antes(c.span.start, Op::Dot).map_or(c.span.start, |i| self.t.span(i).start);
            ini_args = ini_args.or(Some(ponto));
            args.extend(self.no("ConstructorSelector", ponto, c.span.end, Vec::new()));
        }
        if let Some(a) = &k.arguments {
            ini_args = ini_args.or(Some(a.span.start));
            args.extend(self.lista_de_argumentos(a));
            filhos.extend(self.no("EnumConstantArguments", ini_args.unwrap_or(a.span.start), a.span.end, args));
        }
        self.no("EnumConstantDeclaration", inicio, k.span.end, filhos)
    }

    fn membro(&mut self, mid: ast::MemberId) -> Option<usize> {
        let n = self.membro_bruta(mid);
        self.ligar_a(n, Ligacao::Membro(mid))
    }

    fn membro_bruta(&mut self, mid: ast::MemberId) -> Option<usize> {
        let a = self.a;
        let m = a.member(mid);
        let (inicio, mut filhos) = self.doc_e_anotacoes(m.span.start, &m.metadata);
        match &m.kind {
            MemberKind::Field(l) => {
                filhos.extend(self.lista_de_variaveis(l, m.span.start, &[]));
                self.no("FieldDeclaration", inicio, m.span.end, filhos)
            }
            MemberKind::Method(fid) => {
                let f = a.function(*fid);
                if let Some(r) = f.return_type {
                    filhos.extend(self.tipo(r));
                }
                let depois_do_nome = f.name.map_or(f.span.start, |n| n.span.end);
                let tps = self.parametros_de_tipo(&f.type_params);
                let depois_dos_tipos = tps.map_or(depois_do_nome, |n| self.nos[n].fim);
                filhos.extend(tps);
                if let Some(ps) = &f.parameters {
                    filhos.extend(self.lista_de_parametros(ps, depois_dos_tipos));
                }
                filhos.extend(self.corpo(&f.body, f.modifier, m.span.end, true));
                let n = self.no("MethodDeclaration", inicio, m.span.end, filhos);
                let n = self.marcar(n, Marca::Funcao(*fid));
                match f.name {
                    Some(nome) => self.com_sobrescrita(n, nome.span.end),
                    None => n,
                }
            }
            MemberKind::Constructor(k) => {
                filhos.extend(self.identificador(k.class_name));
                let depois_do_nome = k.name.unwrap_or(k.class_name).span.end;
                filhos.extend(self.lista_de_parametros(&k.parameters, depois_do_nome));
                for i in k.initializers.iter() {
                    filhos.extend(self.inicializador(i));
                }
                if let Some(r) = &k.redirect {
                    let tipo = self.tipo(r.ty);
                    let ts = a.ty(r.ty).span;
                    let mut f = tipo.into_iter().collect::<Vec<_>>();
                    let fim = match r.constructor {
                        Some(n) => {
                            f.extend(self.identificador(n));
                            n.span.end
                        }
                        None => ts.end,
                    };
                    filhos.extend(self.no("ConstructorName", ts.start, fim, f));
                }
                filhos.extend(self.corpo(&k.body, AsyncModifier::None, m.span.end, true));
                let n = self.no("ConstructorDeclaration", inicio, m.span.end, filhos);
                self.com_sobrescrita(n, depois_do_nome)
            }
        }
    }

    fn inicializador(&mut self, i: &'a Initializer) -> Option<usize> {
        let n = self.inicializador_bruta(i);
        self.ligar_a(n, Ligacao::Inicializador(endereco(i)))
    }

    fn inicializador_bruta(&mut self, i: &'a Initializer) -> Option<usize> {
        match i {
            Initializer::Field { span, name, value, .. } => {
                let mut f = Vec::new();
                f.extend(self.identificador(*name));
                f.extend(self.expressao(*value));
                self.no("ConstructorFieldInitializer", span.start, span.end, f)
            }
            Initializer::Super { span, constructor, arguments } => {
                let mut f = Vec::new();
                if let Some(n) = constructor {
                    f.extend(self.identificador(*n));
                }
                f.extend(self.lista_de_argumentos(arguments));
                self.no("SuperConstructorInvocation", span.start, span.end, f)
            }
            Initializer::Redirect { span, constructor, arguments } => {
                let mut f = Vec::new();
                if let Some(n) = constructor {
                    f.extend(self.identificador(*n));
                }
                f.extend(self.lista_de_argumentos(arguments));
                self.no("RedirectingConstructorInvocation", span.start, span.end, f)
            }
            Initializer::Assert { span, condition, message } => {
                let mut f = Vec::new();
                f.extend(self.expressao(*condition));
                if let Some(m) = message {
                    f.extend(self.expressao(*m));
                }
                self.no("AssertInitializer", span.start, span.end, f)
            }
        }
    }

    // -- Variáveis e parâmetros --------------------------------------------------

    /// `VariableDeclarationList`: do primeiro de `late`/`final`/`const`/`var`
    /// ?? tipo ?? 1ª variável (depois de `static`/`covariant`/`abstract`/
    /// `external`) ao fim da última variável.
    fn lista_de_variaveis(&mut self, l: &'a ast::VariableList, inicio_do_conteiner: usize, metadata: &'a [Annotation]) -> Option<usize> {
        self.lista_de_variaveis_com(l, inicio_do_conteiner, metadata, false)
    }

    fn lista_de_variaveis_locais(&mut self, l: &'a ast::VariableList, inicio_do_conteiner: usize, metadata: &'a [Annotation]) -> Option<usize> {
        self.lista_de_variaveis_com(l, inicio_do_conteiner, metadata, true)
    }

    fn lista_de_variaveis_com(&mut self, l: &'a ast::VariableList, inicio_do_conteiner: usize, metadata: &'a [Annotation], locais: bool) -> Option<usize> {
        let primeira = l.variables.first()?;
        let ultima = l.variables.last()?;
        let fim = ultima.initializer.map_or(ultima.name.span.end, |e| self.a.expr(e).span.end);
        // O primeiro token da lista: o primeiro modificador da lista entre o
        // começo do contêiner e o tipo (ou a 1ª variável).
        let limite = l.ty.map_or(primeira.name.span.start, |t| self.a.ty(t).span.start);
        let mut inicio = limite;
        let mut i = self.t.indice(inicio_do_conteiner);
        while let Some(tk) = self.t.lista.get(i) {
            if tk.span.start >= limite {
                break;
            }
            let texto = tk.text(self.fonte);
            if matches!(texto, "late" | "final" | "const" | "var") {
                inicio = tk.span.start;
                break;
            }
            i += 1;
        }
        let (inicio, mut filhos) = if metadata.is_empty() { (inicio, Vec::new()) } else { self.doc_e_anotacoes(inicio, metadata) };
        if let Some(t) = l.ty {
            filhos.extend(self.tipo(t));
        }
        for v in l.variables.iter() {
            let fim_v = v.initializer.map_or(v.name.span.end, |e| self.a.expr(e).span.end);
            let ini = v.initializer.and_then(|e| self.expressao(e));
            let n = self.no("VariableDeclaration", v.name.span.start, fim_v, ini.into_iter().collect());
            filhos.extend(if locais { self.marcar(n, Marca::VariavelLocal) } else { n });
        }
        self.no("VariableDeclarationList", inicio, fim, filhos)
    }

    /// `FormalParameterList`: `(` → `)`; `depois` é onde procurar o `(` de
    /// uma lista vazia.
    fn lista_de_parametros(&mut self, ps: &'a [Parameter], depois: usize) -> Option<usize> {
        let abre = match ps.first() {
            Some(p) => {
                let ini = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
                self.t.ultimo_op(Op::LParen, 0, ini)?
            }
            None => self.t.primeiro_op(Op::LParen, depois, usize::MAX)?,
        };
        let fecha = self.t.par_de(abre)?;
        let filhos: Vec<usize> = ps.iter().filter_map(|p| self.parametro(p)).collect();
        self.no("FormalParameterList", self.t.span(abre).start, self.t.span(fecha).end, filhos)
    }

    fn parametro(&mut self, p: &'a Parameter) -> Option<usize> {
        let n = self.parametro_bruta(p);
        self.ligar_a(n, Ligacao::Parametro(endereco(p)))
    }

    fn parametro_bruta(&mut self, p: &'a Parameter) -> Option<usize> {
        let inicio = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
        let mut filhos = Vec::new();
        for m in p.metadata.iter() {
            filhos.extend(self.anotacao(m));
        }
        let (especie, fim) = if let Some(fps) = &p.function_parameters {
            // `void f(int x)`: `returnType`, parâmetros de tipo, parâmetros.
            if let Some(r) = p.ty {
                filhos.extend(self.tipo(r));
            }
            let depois = p.name.map_or(inicio, |n| n.span.end);
            let tps = self.parametros_de_tipo(&p.function_type_params);
            let depois = tps.map_or(depois, |n| self.nos[n].fim);
            filhos.extend(tps);
            let lista = self.lista_de_parametros(fps, depois);
            let mut fim = lista.map_or(depois, |n| self.nos[n].fim);
            filhos.extend(lista);
            if p.function_nullable
                && let Some(i) = self.t.op_depois(fim, Op::Question)
            {
                fim = self.t.span(i).end;
            }
            let especie = if p.this_ {
                "FieldFormalParameter"
            } else if p.super_ {
                "SuperFormalParameter"
            } else {
                "FunctionTypedFormalParameter"
            };
            (especie, fim)
        } else {
            if let Some(t) = p.ty {
                filhos.extend(self.tipo(t));
            }
            let fim = p.name.map(|n| n.span.end).or_else(|| p.ty.map(|t| self.a.ty(t).span.end)).unwrap_or(p.span.end);
            let especie = if p.this_ {
                "FieldFormalParameter"
            } else if p.super_ {
                "SuperFormalParameter"
            } else {
                "SimpleFormalParameter"
            };
            (especie, fim)
        };
        let interno = self.no(especie, inicio, fim, filhos);
        let interno = self.ligar_a(interno, Ligacao::Parametro(endereco(p)));
        if p.kind == ParameterKind::Required {
            return interno;
        }
        // Todo opcional ou nomeado é embrulhado.
        let mut f = interno.into_iter().collect::<Vec<_>>();
        let mut fim_d = fim;
        if let Some(d) = p.default_value {
            fim_d = self.a.expr(d).span.end;
            f.extend(self.expressao(d));
        }
        self.no("DefaultFormalParameter", inicio, fim_d, f)
    }

    /// `TypeParameterList`: `<` → `>`.
    fn parametros_de_tipo(&mut self, tps: &'a [TypeParameter]) -> Option<usize> {
        let (Some(p), Some(u)) = (tps.first(), tps.last()) else { return None };
        let ini_p = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
        let lt = self.t.op_antes(ini_p, Op::Lt)?;
        let gt = self.t.op_depois(u.span.end, Op::Gt)?;
        let mut filhos = Vec::new();
        for tp in tps.iter() {
            let (inicio, mut f) = self.doc_e_anotacoes(tp.span.start, &tp.metadata);
            if let Some(b) = tp.bound {
                f.extend(self.tipo(b));
            }
            let n = self.no("TypeParameter", inicio, tp.span.end, f);
            filhos.extend(self.ligar_a(n, Ligacao::ParametroDeTipo(endereco(tp))));
        }
        self.no("TypeParameterList", self.t.span(lt).start, self.t.span(gt).end, filhos)
    }

    /// `TypeArgumentList`: `<` → `>` em volta dos tipos.
    fn argumentos_de_tipo(&mut self, tipos: &[TypeId]) -> Option<usize> {
        let (Some(p), Some(u)) = (tipos.first(), tipos.last()) else { return None };
        let ps = self.a.ty(*p).span;
        let us = self.a.ty(*u).span;
        let lt = self.t.op_antes(ps.start, Op::Lt)?;
        let gt = self.t.op_depois(us.end, Op::Gt)?;
        let filhos: Vec<usize> = tipos.iter().filter_map(|t| self.tipo(*t)).collect();
        self.no("TypeArgumentList", self.t.span(lt).start, self.t.span(gt).end, filhos)
    }

    /// `ArgumentList` com os parênteses; `nome: e` é `NamedExpression` com o
    /// `Label`.
    fn lista_de_argumentos(&mut self, args: &'a Arguments) -> Option<usize> {
        let n = self.lista_de_argumentos_bruta(args);
        self.ligar_a(n, Ligacao::Argumentos(endereco(args)))
    }

    fn lista_de_argumentos_bruta(&mut self, args: &'a Arguments) -> Option<usize> {
        let mut filhos = Vec::new();
        for x in args.args.iter() {
            filhos.extend(self.argumento(x.name, x.value));
        }
        self.no("ArgumentList", args.span.start, args.span.end, filhos)
    }

    fn argumento(&mut self, nome: Option<ast::Name>, valor: ExprId) -> Option<usize> {
        match nome {
            None => self.expressao(valor),
            Some(n) => {
                let dois_pontos = self.t.op_depois(n.span.end, Op::Colon).map_or(n.span.end, |i| self.t.span(i).end);
                let id = self.identificador(n);
                let rotulo = self.no("Label", n.span.start, dois_pontos, id.into_iter().collect());
                let e = self.expressao(valor);
                let fim = self.a.expr(valor).span.end;
                self.no("NamedExpression", n.span.start, fim, rotulo.into_iter().chain(e).collect())
            }
        }
    }

    // -- Tipos ---------------------------------------------------------------

    fn tipo(&mut self, t: TypeId) -> Option<usize> {
        let n = self.tipo_bruta(t);
        self.ligar_a(n, Ligacao::Tipo(t))
    }

    fn tipo_bruta(&mut self, t: TypeId) -> Option<usize> {
        let ty = self.a.ty(t);
        match &ty.kind {
            TypeKind::Named { name, args } => {
                let mut filhos = Vec::new();
                if name.len() == 2 {
                    // `ImportPrefixReference`: o prefixo com o ponto.
                    let ponto = self.t.op_depois(name[0].span.end, Op::Dot).map_or(name[0].span.end, |i| self.t.span(i).end);
                    filhos.extend(self.no("ImportPrefixReference", name[0].span.start, ponto, Vec::new()));
                }
                filhos.extend(self.argumentos_de_tipo(args));
                self.no("NamedType", ty.span.start, ty.span.end, filhos)
            }
            TypeKind::Void => self.folha("NamedType", ty.span),
            TypeKind::Function { return_type, type_params, parameters } => {
                let mut filhos = Vec::new();
                if let Some(r) = return_type {
                    filhos.extend(self.tipo(*r));
                }
                let tps = self.parametros_de_tipo(type_params);
                let depois = tps.map_or_else(|| self.posicao_de_function(ty.span), |n| self.nos[n].fim);
                filhos.extend(tps);
                filhos.extend(self.lista_de_parametros(parameters, depois));
                self.no("GenericFunctionType", ty.span.start, ty.span.end, filhos)
            }
            TypeKind::Record { positional, named } => {
                let mut filhos = Vec::new();
                for p in positional.iter() {
                    let f = self.tipo(*p);
                    let s = self.a.ty(*p).span;
                    filhos.extend(self.no("RecordTypeAnnotationPositionalField", s.start, s.end, f.into_iter().collect()));
                }
                if let (Some(p), Some(u)) = (named.first(), named.last()) {
                    let ini = self.a.ty(p.1).span.start;
                    let abre = self.t.ultimo_op(Op::LBrace, 0, ini);
                    let fecha = abre.and_then(|i| self.t.par_de(i));
                    let mut campos = Vec::new();
                    for (n, t) in named.iter() {
                        let f = self.tipo(*t);
                        let s = self.a.ty(*t).span;
                        campos.extend(self.no("RecordTypeAnnotationNamedField", s.start, n.span.end, f.into_iter().collect()));
                    }
                    let (a, b) = match (abre, fecha) {
                        (Some(a), Some(b)) => (self.t.span(a).start, self.t.span(b).end),
                        _ => (ini, u.0.span.end),
                    };
                    filhos.extend(self.no("RecordTypeAnnotationNamedFields", a, b, campos));
                }
                self.no("RecordTypeAnnotation", ty.span.start, ty.span.end, filhos)
            }
        }
    }

    /// Onde procurar o `(` de um `Function(...)` sem parâmetros de tipo: o
    /// token `Function` dentro do tipo.
    fn posicao_de_function(&self, s: Span) -> usize {
        let mut i = self.t.indice(s.start);
        while let Some(tk) = self.t.lista.get(i) {
            if tk.span.start >= s.end {
                break;
            }
            if tk.text(self.fonte) == "Function" {
                return tk.span.end;
            }
            i += 1;
        }
        s.start
    }

    // -- Comandos --------------------------------------------------------------

    fn comando(&mut self, sid: StmtId) -> Option<usize> {
        let n = self.comando_bruta(sid);
        self.ligar_a(n, Ligacao::Stmt(sid))
    }

    fn comando_bruta(&mut self, sid: StmtId) -> Option<usize> {
        let a = self.a;
        let s = a.stmt(sid);
        match &s.kind {
            StmtKind::Block(ss) => {
                let filhos: Vec<usize> = ss.iter().filter_map(|x| self.comando(*x)).collect();
                self.no("Block", s.span.start, s.span.end, filhos)
            }
            StmtKind::Variables(l) => {
                let meta = self.metadados_locais.get(&sid.0).copied().unwrap_or(&[]);
                let lista = self.lista_de_variaveis_locais(l, s.span.start, meta);
                let inicio = lista.map_or(s.span.start, |n| self.nos[n].inicio.min(s.span.start));
                self.no("VariableDeclarationStatement", inicio, s.span.end, lista.into_iter().collect())
            }
            StmtKind::PatternVariables { pattern, value, .. } => {
                let fim = a.expr(*value).span.end;
                let p = self.padrao(*pattern);
                let e = self.expressao(*value);
                let decl = self.no("PatternVariableDeclaration", s.span.start, fim, p.into_iter().chain(e).collect());
                self.no("PatternVariableDeclarationStatement", s.span.start, s.span.end, decl.into_iter().collect())
            }
            StmtKind::Function(fid) => {
                let meta = self.metadados_locais.get(&sid.0).copied().unwrap_or(&[]);
                let (inicio, filhos) = self.doc_e_anotacoes(s.span.start, meta);
                let d = self.declaracao_de_funcao(*fid, inicio, filhos, true);
                let nome = a.function(*fid).name.map(|n| n.span.end);
                let d = match nome {
                    Some(p) => self.com_sobrescrita(d, p),
                    None => d,
                };
                let ini = d.map_or(s.span.start, |n| self.nos[n].inicio);
                let fim = d.map_or(s.span.end, |n| self.nos[n].fim);
                self.no("FunctionDeclarationStatement", ini, fim, d.into_iter().collect())
            }
            StmtKind::Expression(e) => {
                let x = self.expressao(*e);
                self.no("ExpressionStatement", s.span.start, s.span.end, x.into_iter().collect())
            }
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                let mut filhos = Vec::new();
                filhos.extend(self.expressao(*condition));
                filhos.extend(self.clausula_case(*case_pattern, *guard));
                filhos.extend(self.comando(*then));
                if let Some(e) = else_ {
                    filhos.extend(self.comando(*e));
                }
                self.no("IfStatement", s.span.start, s.span.end, filhos)
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                let partes = self.partes_de_for(s.span.start, init.as_ref(), *condition, updates);
                let corpo = self.comando(*body);
                self.no("ForStatement", s.span.start, s.span.end, partes.into_iter().chain(corpo).collect())
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                let partes = self.partes_de_for_in(s.span.start, target, *iterable);
                let corpo = self.comando(*body);
                self.no("ForStatement", s.span.start, s.span.end, partes.into_iter().chain(corpo).collect())
            }
            StmtKind::While { condition, body } => {
                let c = self.expressao(*condition);
                let b = self.comando(*body);
                self.no("WhileStatement", s.span.start, s.span.end, c.into_iter().chain(b).collect())
            }
            StmtKind::DoWhile { body, condition } => {
                let b = self.comando(*body);
                let c = self.expressao(*condition);
                self.no("DoStatement", s.span.start, s.span.end, b.into_iter().chain(c).collect())
            }
            StmtKind::Switch { value, cases } => {
                let mut filhos = Vec::new();
                filhos.extend(self.expressao(*value));
                for c in cases.iter() {
                    filhos.extend(self.caso(c));
                }
                self.no("SwitchStatement", s.span.start, s.span.end, filhos)
            }
            StmtKind::Break(l) | StmtKind::Continue(l) => {
                let especie = if matches!(s.kind, StmtKind::Break(_)) { "BreakStatement" } else { "ContinueStatement" };
                let r = l.and_then(|n| self.identificador(n));
                self.no(especie, s.span.start, s.span.end, r.into_iter().collect())
            }
            StmtKind::Return(e) => {
                let x = e.and_then(|e| self.expressao(e));
                self.no("ReturnStatement", s.span.start, s.span.end, x.into_iter().collect())
            }
            StmtKind::Yield { value, .. } => {
                let x = self.expressao(*value);
                self.no("YieldStatement", s.span.start, s.span.end, x.into_iter().collect())
            }
            StmtKind::Try { body, catches, finally_ } => {
                let mut filhos = Vec::new();
                filhos.extend(self.comando(*body));
                for c in catches.iter() {
                    let mut f = Vec::new();
                    if let Some(t) = c.on_type {
                        f.extend(self.tipo(t));
                    }
                    if let Some(e) = c.exception {
                        f.extend(self.folha("CatchClauseParameter", e.span));
                    }
                    if let Some(st) = c.stack_trace {
                        f.extend(self.folha("CatchClauseParameter", st.span));
                    }
                    f.extend(self.comando(c.body));
                    let cc = self.no("CatchClause", c.span.start, c.span.end, f);
                    filhos.extend(self.ligar_a(cc, Ligacao::Catch(endereco(c))));
                }
                if let Some(fi) = finally_ {
                    filhos.extend(self.comando(*fi));
                }
                self.no("TryStatement", s.span.start, s.span.end, filhos)
            }
            StmtKind::Labeled { labels, body } => {
                let mut filhos = Vec::new();
                for l in labels.iter() {
                    filhos.extend(self.rotulo(*l));
                }
                filhos.extend(self.comando(*body));
                self.no("LabeledStatement", s.span.start, s.span.end, filhos)
            }
            StmtKind::Assert { condition, message } => {
                let mut filhos = Vec::new();
                filhos.extend(self.expressao(*condition));
                if let Some(m) = message {
                    filhos.extend(self.expressao(*m));
                }
                self.no("AssertStatement", s.span.start, s.span.end, filhos)
            }
            StmtKind::Empty => self.folha("EmptyStatement", s.span),
        }
    }

    /// `Label` de comando: nome → `:`.
    fn rotulo(&mut self, n: ast::Name) -> Option<usize> {
        let fim = self.t.op_depois(n.span.end, Op::Colon).map_or(n.span.end, |i| self.t.span(i).end);
        let id = self.identificador(n);
        let id = self.marcar(id, Marca::ContextoDeDeclaracao);
        self.no("Label", n.span.start, fim, id.into_iter().collect())
    }

    /// `CaseClause` de `if (e case p when g)`.
    fn clausula_case(&mut self, padrao: Option<PatternId>, guarda: Option<ExprId>) -> Option<usize> {
        let p = padrao?;
        let ps = self.a.pattern(p).span;
        let palavra = self.t.antes(ps.start).map_or(ps.start, |i| self.t.span(i).start);
        let gp = self.padrao_guardado(p, guarda);
        let fim = gp.map_or(ps.end, |n| self.nos[n].fim);
        self.no("CaseClause", palavra, fim, gp.into_iter().collect())
    }

    /// `GuardedPattern` (padrão → guarda) com o `WhenClause`.
    fn padrao_guardado(&mut self, p: PatternId, guarda: Option<ExprId>) -> Option<usize> {
        let ps = self.a.pattern(p).span;
        let mut filhos = Vec::new();
        filhos.extend(self.padrao(p));
        let mut fim = ps.end;
        if let Some(g) = guarda {
            let gs = self.a.expr(g).span;
            let when = self.t.antes(gs.start).map_or(gs.start, |i| self.t.span(i).start);
            let e = self.expressao(g);
            filhos.extend(self.no("WhenClause", when, gs.end, e.into_iter().collect()));
            fim = gs.end;
        }
        self.no("GuardedPattern", ps.start, fim, filhos)
    }

    fn caso(&mut self, c: &'a ast::SwitchCase) -> Option<usize> {
        let n = self.caso_bruta(c);
        self.ligar_a(n, Ligacao::Caso(endereco(c)))
    }

    fn caso_bruta(&mut self, c: &'a ast::SwitchCase) -> Option<usize> {
        let mut filhos = Vec::new();
        for l in c.labels.iter() {
            filhos.extend(self.rotulo(*l));
        }
        let especie = match c.pattern {
            None => "SwitchDefault",
            Some(p) => {
                if self.antes_de_3 {
                    match &self.a.pattern(p).kind {
                        PatternKind::Constant(e) => filhos.extend(self.expressao(*e)),
                        _ => filhos.extend(self.padrao(p)),
                    }
                    "SwitchCase"
                } else {
                    filhos.extend(self.padrao_guardado(p, c.guard));
                    "SwitchPatternCase"
                }
            }
        };
        for s in c.body.iter() {
            filhos.extend(self.comando(*s));
        }
        self.no(especie, c.span.start, c.span.end, filhos)
    }

    /// Os dois `;` de nível zero dentro do `(` de um `for`.
    fn pontos_e_virgulas_do_for(&self, inicio_do_for: usize) -> (Option<usize>, Option<usize>, Option<usize>) {
        let Some(abre) = self.t.primeiro_op(Op::LParen, inicio_do_for, usize::MAX) else { return (None, None, None) };
        let Some(fecha) = self.t.par_de(abre) else { return (Some(abre), None, None) };
        let mut primeiro = None;
        let mut segundo = None;
        let mut i = abre + 1;
        while i < fecha {
            if let Some(j) = self.t.par_de(i)
                && j > i
            {
                i = j + 1;
                continue;
            }
            if self.t.e_op(i, Op::Semicolon) {
                if primeiro.is_none() {
                    primeiro = Some(i);
                } else {
                    segundo = Some(i);
                    break;
                }
            }
            i += 1;
        }
        (Some(abre), primeiro, segundo)
    }

    fn partes_de_for(&mut self, inicio_do_for: usize, init: Option<&'a ForInit>, cond: Option<ExprId>, updates: &'a [ExprId]) -> Option<usize> {
        let (_, p1, p2) = self.pontos_e_virgulas_do_for(inicio_do_for);
        let fim = match updates.last() {
            Some(u) => self.a.expr(*u).span.end,
            None => p2.map(|i| self.t.span(i).end)?,
        };
        let mut filhos = Vec::new();
        let mut partes = PartesDeFor::default();
        let (especie, inicio) = match init {
            Some(ForInit::Variables(l)) => {
                // A lista começa logo depois do `(` do `for`.
                let depois_do_parentese = self.t.primeiro_op(Op::LParen, inicio_do_for, usize::MAX).map_or(inicio_do_for, |i| self.t.span(i).end);
                let lista = self.lista_de_variaveis_locais(l, depois_do_parentese, &[]);
                let inicio = lista.map_or(depois_do_parentese, |n| self.nos[n].inicio);
                filhos.extend(lista);
                ("ForPartsWithDeclarations", inicio)
            }
            Some(ForInit::Expression(e)) => {
                let s = self.a.expr(*e).span;
                filhos.extend(self.expressao(*e));
                ("ForPartsWithExpression", s.start)
            }
            Some(ForInit::Pattern { pattern, value, .. }) => {
                let abre = self.t.primeiro_op(Op::LParen, inicio_do_for, usize::MAX).map_or(0, |i| self.t.span(i).end);
                let palavra = self.t.depois(abre).map_or(abre, |i| self.t.span(i).start);
                let fim_v = self.a.expr(*value).span.end;
                let p = self.padrao(*pattern);
                let e = self.expressao(*value);
                filhos.extend(self.no("PatternVariableDeclaration", palavra, fim_v, p.into_iter().chain(e).collect()));
                ("ForPartsWithPattern", palavra)
            }
            None => ("ForPartsWithExpression", p1.map(|i| self.t.span(i).start)?),
        };
        partes.inicio = if init.is_some() { filhos.first().copied() } else { None };
        if let Some(c) = cond {
            let n = self.expressao(c);
            partes.condicao = n;
            filhos.extend(n);
        }
        for u in updates.iter() {
            let n = self.expressao(*u);
            partes.atualizacoes.extend(n);
            filhos.extend(n);
        }
        let n = self.no(especie, inicio, fim, filhos);
        if let Some(k) = n {
            self.partes_de_for.insert(k, partes);
        }
        n
    }

    fn partes_de_for_in(&mut self, inicio_do_for: usize, alvo: &'a ForInTarget, iteravel: ExprId) -> Option<usize> {
        let fim = self.a.expr(iteravel).span.end;
        let abre = self.t.primeiro_op(Op::LParen, inicio_do_for, usize::MAX).map_or(inicio_do_for, |i| self.t.span(i).end);
        let primeiro = self.t.depois(abre).map_or(abre, |i| self.t.span(i).start);
        let mut filhos = Vec::new();
        let (especie, inicio) = match alvo {
            ForInTarget::Declared { metadata, ty, name, .. } => {
                let (ini, mut f) = self.doc_e_anotacoes(primeiro, metadata);
                if let Some(t) = ty {
                    f.extend(self.tipo(*t));
                }
                let d = self.no("DeclaredIdentifier", ini, name.span.end, f);
                let inicio = d.map_or(ini, |n| self.nos[n].inicio);
                filhos.extend(d);
                ("ForEachPartsWithDeclaration", inicio)
            }
            ForInTarget::Pattern { pattern, .. } => {
                filhos.extend(self.padrao(*pattern));
                ("ForEachPartsWithPattern", primeiro)
            }
            ForInTarget::Expression(e) => {
                let s = self.a.expr(*e).span;
                filhos.extend(self.expressao(*e));
                ("ForEachPartsWithIdentifier", s.start)
            }
        };
        filhos.extend(self.expressao(iteravel));
        self.no(especie, inicio, fim, filhos)
    }

    // -- Expressões -------------------------------------------------------------

    /// O nó mais à esquerda da expressão é o alvo implícito da cascata.
    fn em_cascata(&self, e: ExprId) -> bool {
        let mut atual = e;
        for _ in 0..256 {
            atual = match &self.a.expr(atual).kind {
                ExprKind::CascadeTarget => return true,
                ExprKind::Property { target, .. }
                | ExprKind::Index { target, .. }
                | ExprKind::Call { target, .. }
                | ExprKind::TypeArguments { target, .. }
                | ExprKind::Assign { target, .. } => *target,
                ExprKind::Unary { op: UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert, operand } => *operand,
                _ => return false,
            };
        }
        false
    }

    /// O início de uma seção de cascata: o `..`/`?..` antes dela.
    fn inicio(&self, e: ExprId) -> usize {
        let s = self.a.expr(e).span;
        if self.em_cascata(e) {
            let i = self.t.antes(s.start).filter(|&i| self.t.e_op(i, Op::DotDot) || self.t.e_op(i, Op::QuestionDotDot));
            if let Some(i) = i {
                return self.t.span(i).start;
            }
            // O `..` pode estar dentro do span (o parser o inclui).
            if let Some(i) = self.t.depois(s.start)
                && (self.t.e_op(i, Op::DotDot) || self.t.e_op(i, Op::QuestionDotDot))
            {
                return self.t.span(i).start;
            }
        }
        s.start
    }

    fn expressao(&mut self, e: ExprId) -> Option<usize> {
        let n = self.expressao_bruta(e);
        self.ligar_a(n, Ligacao::Expr(e))
    }

    fn expressao_bruta(&mut self, e: ExprId) -> Option<usize> {
        let a = self.a;
        let x = a.expr(e);
        let s = Span { start: self.inicio(e), end: x.span.end };
        match &x.kind {
            ExprKind::Int(_) => self.folha("IntegerLiteral", s),
            ExprKind::Double(_) => self.folha("DoubleLiteral", s),
            ExprKind::Bool(_) => self.folha("BooleanLiteral", s),
            ExprKind::Null => self.folha("NullLiteral", s),
            ExprKind::String(lit) => self.string(lit),
            ExprKind::Symbol(_) => self.folha("SymbolLiteral", s),
            ExprKind::Identifier(_) => {
                let n = self.folha("SimpleIdentifier", s);
                self.marcar(n, Marca::Expr(e))
            }
            ExprKind::This => self.folha("ThisExpression", s),
            ExprKind::Super => self.folha("SuperExpression", s),
            ExprKind::Rethrow => self.folha("RethrowExpression", s),
            ExprKind::CascadeTarget => None,
            ExprKind::DotShorthand { .. } => self.folha("DotShorthand", s),
            ExprKind::Parenthesized(i) => {
                let f = self.expressao(*i);
                self.no("ParenthesizedExpression", s.start, s.end, f.into_iter().collect())
            }
            ExprKind::List { type_args, elements, .. } | ExprKind::SetOrMap { type_args, elements, .. } => {
                let especie = if matches!(x.kind, ExprKind::List { .. }) { "ListLiteral" } else { "SetOrMapLiteral" };
                let mut filhos = Vec::new();
                filhos.extend(self.argumentos_de_tipo(type_args));
                for el in elements.iter() {
                    filhos.extend(self.elemento(el));
                }
                self.no(especie, s.start, s.end, filhos)
            }
            ExprKind::Record { positional, named, .. } => {
                let mut itens: Vec<(usize, Option<ast::Name>, ExprId)> = Vec::new();
                for p in positional.iter() {
                    itens.push((a.expr(*p).span.start, None, *p));
                }
                for (n, v) in named.iter() {
                    itens.push((n.span.start, Some(*n), *v));
                }
                itens.sort_by_key(|(p, _, _)| *p);
                let mut filhos = Vec::new();
                for (_, n, v) in itens {
                    filhos.extend(self.argumento(n, v));
                }
                self.no("RecordLiteral", s.start, s.end, filhos)
            }
            ExprKind::InstanceCreation { keyword, ty, constructor, arguments } => {
                let ts = a.ty(*ty).span;
                if keyword.is_none() && constructor.is_none() && !self.construtores.is_some_and(|c| c.contains(&e)) {
                    // `A<T>()` sem palavra-chave: `MethodInvocation`.
                    return self.invocacao_de_tipo(s, *ty, arguments);
                }
                let mut f = Vec::new();
                f.extend(self.tipo(*ty));
                let fim = match constructor {
                    Some(n) => {
                        f.extend(self.identificador(*n));
                        n.span.end
                    }
                    None => ts.end,
                };
                let nome = self.no("ConstructorName", ts.start, fim, f);
                let mut filhos: Vec<usize> = nome.into_iter().collect();
                filhos.extend(self.argumentos_de_tipo(&arguments.type_args));
                filhos.extend(self.lista_de_argumentos(arguments));
                self.no("InstanceCreationExpression", s.start, s.end, filhos)
            }
            ExprKind::FunctionExpression(fid) => {
                let n = self.expressao_de_funcao(*fid, false)?;
                // A closure começa nos parâmetros de tipo ou no `(`.
                Some(n)
            }
            ExprKind::Property { target, name, null_aware } => {
                let mut filhos = Vec::new();
                filhos.extend(self.expressao(*target));
                let id = self.identificador(*name);
                filhos.extend(self.marcar(id, Marca::Expr(e)));
                let prefixado = !*null_aware && matches!(a.expr(*target).kind, ExprKind::Identifier(_)) && !self.em_cascata(e);
                self.no(if prefixado { "PrefixedIdentifier" } else { "PropertyAccess" }, s.start, s.end, filhos)
            }
            ExprKind::Index { target, index, .. } => {
                let mut filhos = Vec::new();
                filhos.extend(self.expressao(*target));
                filhos.extend(self.expressao(*index));
                self.no("IndexExpression", s.start, s.end, filhos)
            }
            ExprKind::Call { target, arguments } => self.chamada(e, s, *target, arguments),
            ExprKind::TypeArguments { target, type_args } => {
                let mut filhos = Vec::new();
                filhos.extend(self.expressao(*target));
                filhos.extend(self.argumentos_de_tipo(type_args));
                self.no("FunctionReference", s.start, s.end, filhos)
            }
            ExprKind::Unary { op, operand } => {
                let especie = match op {
                    UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert => "PostfixExpression",
                    _ => "PrefixExpression",
                };
                let f = self.expressao(*operand);
                let n = self.no(especie, s.start, s.end, f.into_iter().collect());
                if especie == "PrefixExpression" { self.marcar(n, Marca::Operador(e)) } else { n }
            }
            ExprKind::Binary { left, right, .. } => {
                let l = self.expressao(*left);
                let r = self.expressao(*right);
                self.no("BinaryExpression", s.start, s.end, l.into_iter().chain(r).collect())
            }
            ExprKind::Conditional { condition, then, else_ } => {
                let mut f = Vec::new();
                f.extend(self.expressao(*condition));
                f.extend(self.expressao(*then));
                f.extend(self.expressao(*else_));
                self.no("ConditionalExpression", s.start, s.end, f)
            }
            ExprKind::Is { value, ty, .. } | ExprKind::As { value, ty } => {
                let especie = if matches!(x.kind, ExprKind::Is { .. }) { "IsExpression" } else { "AsExpression" };
                let v = self.expressao(*value);
                let t = self.tipo(*ty);
                self.no(especie, s.start, s.end, v.into_iter().chain(t).collect())
            }
            ExprKind::Assign { target, value, .. } => {
                let l = self.expressao(*target);
                let r = self.expressao(*value);
                self.no("AssignmentExpression", s.start, s.end, l.into_iter().chain(r).collect())
            }
            ExprKind::PatternAssign { pattern, value } => {
                let p = self.padrao(*pattern);
                let v = self.expressao(*value);
                self.no("PatternAssignment", s.start, s.end, p.into_iter().chain(v).collect())
            }
            ExprKind::Cascade { target, sections, .. } => {
                let mut f = Vec::new();
                f.extend(self.expressao(*target));
                for sec in sections.iter() {
                    f.extend(self.expressao(*sec));
                }
                self.no("CascadeExpression", s.start, s.end, f)
            }
            ExprKind::Await(i) => {
                let f = self.expressao(*i);
                self.no("AwaitExpression", s.start, s.end, f.into_iter().collect())
            }
            ExprKind::Throw(i) => {
                let f = self.expressao(*i);
                self.no("ThrowExpression", s.start, s.end, f.into_iter().collect())
            }
            ExprKind::Switch { value, cases } => {
                let mut f = Vec::new();
                f.extend(self.expressao(*value));
                for (k, c) in cases.iter().enumerate() {
                    let gp = self.padrao_guardado(c.pattern, c.guard);
                    let b = self.expressao(c.body);
                    let ini = self.a.pattern(c.pattern).span.start;
                    let fim = self.a.expr(c.body).span.end;
                    let n = self.no("SwitchExpressionCase", ini, fim, gp.into_iter().chain(b).collect());
                    f.extend(self.ligar_a(n, Ligacao::CasoDeSwitchExpr(e, k)));
                }
                self.no("SwitchExpression", s.start, s.end, f)
            }
        }
    }

    /// `A<T>()` sem `new`/`const`: a `MethodInvocation` do parser.
    fn invocacao_de_tipo(&mut self, s: Span, ty: TypeId, arguments: &'a Arguments) -> Option<usize> {
        let mut filhos = Vec::new();
        if let TypeKind::Named { name, args } = &self.a.ty(ty).kind {
            match &name[..] {
                [n] => filhos.extend(self.identificador(*n)),
                [p, n] => {
                    filhos.extend(self.identificador(*p));
                    filhos.extend(self.identificador(*n));
                }
                _ => {}
            }
            filhos.extend(self.argumentos_de_tipo(args));
        }
        filhos.extend(self.lista_de_argumentos(arguments));
        self.no("MethodInvocation", s.start, s.end, filhos)
    }

    /// `MethodInvocation` (alvo identificador ou `alvo.nome`) ou
    /// `FunctionExpressionInvocation`.
    fn chamada(&mut self, e: ExprId, s: Span, alvo: ExprId, arguments: &'a Arguments) -> Option<usize> {
        let a = self.a;
        // `f<int>(x)`: os argumentos de tipo podem vir no alvo.
        let (alvo, tipos_no_alvo): (ExprId, Option<&'a [TypeId]>) = match &a.expr(alvo).kind {
            ExprKind::TypeArguments { target, type_args }
                if matches!(a.expr(*target).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) =>
            {
                (*target, Some(&type_args[..]))
            }
            _ => (alvo, None),
        };
        if self.construtores.is_some_and(|c| c.contains(&e))
            && let Some(n) = self.criacao_sem_new(s, alvo, tipos_no_alvo.unwrap_or(&arguments.type_args), arguments)
        {
            return Some(n);
        }
        let mut filhos = Vec::new();
        let especie = match &a.expr(alvo).kind {
            ExprKind::Identifier(n) => {
                let id = self.identificador(*n);
                filhos.extend(self.marcar(id, Marca::Expr(alvo)));
                "MethodInvocation"
            }
            ExprKind::Property { target, name, .. } => {
                filhos.extend(self.expressao(*target));
                let id = self.identificador(*name);
                filhos.extend(self.marcar(id, Marca::Expr(alvo)));
                "MethodInvocation"
            }
            _ => {
                filhos.extend(self.expressao(alvo));
                "FunctionExpressionInvocation"
            }
        };
        match tipos_no_alvo {
            Some(t) => filhos.extend(self.argumentos_de_tipo(t)),
            None => filhos.extend(self.argumentos_de_tipo(&arguments.type_args)),
        }
        filhos.extend(self.lista_de_argumentos(arguments));
        self.no(especie, s.start, s.end, filhos)
    }

    /// `AstRewriter.methodInvocation`: `A()`, `A.n()`, `p.A()` e `p.A.n()`
    /// que invocam construtor viram `InstanceCreationExpression` com
    /// `ConstructorName(NamedType, name?)`; os argumentos de tipo da
    /// invocação vão para o `NamedType`.
    fn criacao_sem_new(&mut self, s: Span, alvo: ExprId, tipos: &'a [TypeId], arguments: &'a Arguments) -> Option<usize> {
        let a = self.a;
        let e_prefixo = |x: ExprId| self.prefixos.is_some_and(|p| p.contains(&x));
        // (prefixo, tipo, construtor nomeado)
        let (prefixo, tipo, nome): (Option<ast::Name>, ast::Name, Option<(ast::Name, ExprId)>) = match &a.expr(alvo).kind {
            ExprKind::Identifier(n) => (None, *n, None),
            ExprKind::Property { target, name, .. } => match &a.expr(*target).kind {
                ExprKind::Identifier(p) if e_prefixo(*target) => (Some(*p), *name, None),
                ExprKind::Identifier(c) => (None, *c, Some((*name, alvo))),
                ExprKind::Property { target: t2, name: c, .. } => match &a.expr(*t2).kind {
                    ExprKind::Identifier(p) if e_prefixo(*t2) => (Some(*p), *c, Some((*name, alvo))),
                    _ => return None,
                },
                _ => return None,
            },
            _ => return None,
        };
        let mut f = Vec::new();
        let inicio_do_tipo = prefixo.map_or(tipo.span.start, |p| p.span.start);
        if let Some(p) = prefixo {
            let ponto = self.t.op_depois(p.span.end, Op::Dot).map_or(p.span.end, |i| self.t.span(i).end);
            f.extend(self.no("ImportPrefixReference", p.span.start, ponto, Vec::new()));
        }
        // Os argumentos de tipo ficam no tipo quando vêm logo depois dele
        // (`A<T>()`, `p.A<T>()`); depois do nome do construtor
        // (`A.n<T>()`, erro) ficam na criação.
        let args_no_tipo = nome.is_none() && !tipos.is_empty();
        let mut fim_do_tipo = tipo.span.end;
        if args_no_tipo {
            let n = self.argumentos_de_tipo(tipos);
            if let Some(k) = n {
                fim_do_tipo = self.nos[k].fim;
            }
            f.extend(n);
        }
        let named = self.no("NamedType", inicio_do_tipo, fim_do_tipo, f);
        let mut cn = Vec::new();
        cn.extend(named);
        let mut fim_cn = fim_do_tipo;
        if let Some((n, de)) = nome {
            let id = self.identificador(n);
            cn.extend(self.marcar(id, Marca::Expr(de)));
            fim_cn = n.span.end;
        }
        let nome_do_construtor = self.no("ConstructorName", inicio_do_tipo, fim_cn, cn);
        let mut filhos: Vec<usize> = nome_do_construtor.into_iter().collect();
        if !args_no_tipo {
            filhos.extend(self.argumentos_de_tipo(tipos));
        }
        filhos.extend(self.lista_de_argumentos(arguments));
        self.no("InstanceCreationExpression", s.start, s.end, filhos)
    }

    /// Um elemento de coleção.
    fn elemento(&mut self, el: &'a CollectionElement) -> Option<usize> {
        let n = self.elemento_bruta(el);
        self.ligar_a(n, Ligacao::Elemento(endereco(el)))
    }

    fn elemento_bruta(&mut self, el: &'a CollectionElement) -> Option<usize> {
        match el {
            CollectionElement::Expression(e) => self.expressao(*e),
            CollectionElement::NullAwareExpression(e) => {
                let s = self.a.expr(*e).span;
                let q = self.t.op_antes(s.start, Op::Question).map_or(s.start, |i| self.t.span(i).start);
                let f = self.expressao(*e);
                self.no("NullAwareElement", q, s.end, f.into_iter().collect())
            }
            CollectionElement::MapEntry { key, value, null_aware_key, .. } => {
                let ks = self.a.expr(*key).span;
                let inicio = if *null_aware_key { self.t.op_antes(ks.start, Op::Question).map_or(ks.start, |i| self.t.span(i).start) } else { ks.start };
                let k = self.expressao(*key);
                let v = self.expressao(*value);
                let fim = self.a.expr(*value).span.end;
                self.no("MapLiteralEntry", inicio, fim, k.into_iter().chain(v).collect())
            }
            CollectionElement::Spread { value, .. } => {
                let s = self.a.expr(*value).span;
                let tres = self
                    .t
                    .antes(s.start)
                    .filter(|&i| self.t.e_op(i, Op::Ellipsis) || self.t.e_op(i, Op::EllipsisQuestion))
                    .map_or(s.start, |i| self.t.span(i).start);
                let f = self.expressao(*value);
                self.no("SpreadElement", tres, s.end, f.into_iter().collect())
            }
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                let cs = self.a.expr(*condition).span;
                // `if (`: o `if` antes do `(` que abre a condição.
                let abre = self.t.ultimo_op(Op::LParen, 0, cs.start);
                let palavra = abre.and_then(|i| self.t.antes(self.t.span(i).start)).map_or(cs.start, |i| self.t.span(i).start);
                let mut f = Vec::new();
                f.extend(self.expressao(*condition));
                f.extend(self.clausula_case(*case_pattern, *guard));
                let t = self.elemento(then);
                let mut fim = t.map_or(cs.end, |n| self.nos[n].fim);
                f.extend(t);
                if let Some(e) = else_ {
                    let x = self.elemento(e);
                    fim = x.map_or(fim, |n| self.nos[n].fim);
                    f.extend(x);
                }
                self.no("IfElement", palavra, fim, f)
            }
            CollectionElement::For { await_, init, condition, updates, body } => {
                let corpo_ini = self.inicio_de_elemento(body);
                let (palavra, abre) = self.for_antes(corpo_ini, *await_);
                let partes = self.partes_de_for(abre, init.as_ref(), *condition, updates);
                let b = self.elemento(body);
                let fim = b.map_or(corpo_ini, |n| self.nos[n].fim);
                self.no("ForElement", palavra, fim, partes.into_iter().chain(b).collect())
            }
            CollectionElement::ForIn { await_, target, iterable, body } => {
                let corpo_ini = self.inicio_de_elemento(body);
                let (palavra, abre) = self.for_antes(corpo_ini, *await_);
                let partes = self.partes_de_for_in(abre, target, *iterable);
                let b = self.elemento(body);
                let fim = b.map_or(corpo_ini, |n| self.nos[n].fim);
                self.no("ForElement", palavra, fim, partes.into_iter().chain(b).collect())
            }
        }
    }

    /// O primeiro offset de um elemento (sem montar nós).
    fn inicio_de_elemento(&self, el: &CollectionElement) -> usize {
        match el {
            CollectionElement::Expression(e) | CollectionElement::NullAwareExpression(e) | CollectionElement::Spread { value: e, .. } => {
                self.t.antes(self.a.expr(*e).span.start).map_or(self.a.expr(*e).span.start, |i| self.t.span(i).start).min(self.a.expr(*e).span.start)
            }
            CollectionElement::MapEntry { key, .. } => self.a.expr(*key).span.start,
            CollectionElement::If { condition, .. } => self.a.expr(*condition).span.start,
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => self.inicio_de_elemento(body),
        }
    }

    /// O `for` (ou `await`) e o `(` de um elemento `for` cujo corpo começa em
    /// `corpo`: o `)` antes do corpo fecha o `(` do `for`.
    fn for_antes(&self, corpo: usize, await_: bool) -> (usize, usize) {
        let fecha = self.t.ultimo_op(Op::RParen, 0, corpo + 1);
        let abre = fecha.and_then(|i| self.t.par_de(i));
        let palavra = abre.and_then(|i| self.t.antes(self.t.span(i).start));
        let mut inicio = palavra.map_or(corpo, |i| self.t.span(i).start);
        if await_
            && let Some(p) = palavra
            && let Some(i) = self.t.antes(self.t.span(p).start)
            && self.t.e_palavra(i, self.fonte, "await")
        {
            inicio = self.t.span(i).start;
        }
        (inicio, palavra.map_or(corpo, |i| self.t.span(i).start))
    }

    // -- Strings ---------------------------------------------------------------

    /// `SimpleStringLiteral`, `StringInterpolation` ou `AdjacentStrings`,
    /// pelos tokens da string.
    fn string(&mut self, lit: &'a StringLit) -> Option<usize> {
        let interpolacoes: Vec<ExprId> = lit
            .parts
            .iter()
            .filter_map(|p| match p {
                ast::StringPart::Interpolation(e) => Some(*e),
                ast::StringPart::Text(_) => None,
            })
            .collect();
        let mut proxima = 0usize;
        let mut grupos: Vec<usize> = Vec::new();
        let mut i = self.t.indice(lit.span.start);
        while let Some(tk) = self.t.lista.get(i).copied() {
            if tk.span.start >= lit.span.end {
                break;
            }
            match tk.kind {
                Kind::Str(_) => {
                    grupos.extend(self.folha("SimpleStringLiteral", tk.span));
                    i += 1;
                }
                Kind::StrBegin(_, interp) => {
                    // `StringInterpolation`: do começo ao `StrEnd`.
                    let inicio = tk.span.start;
                    let mut elementos = Vec::new();
                    let mut trecho = tk.span;
                    let mut modo = interp;
                    let mut fechou = false;
                    let mut fim = tk.span.end;
                    loop {
                        // O texto sem o `$`/`${` do fim.
                        let corte = match modo {
                            dartforge_frontend::token::Interp::Brace => 2,
                            dartforge_frontend::token::Interp::Ident => 1,
                        };
                        let texto_fim = trecho.end.saturating_sub(corte).max(trecho.start);
                        elementos.extend(self.folha("InterpolationString", Span { start: trecho.start, end: texto_fim }));
                        // A expressão interpolada.
                        let e = interpolacoes.get(proxima).copied();
                        proxima += 1;
                        let (expr_no, expr_fim) = match e {
                            Some(e) => (self.expressao(e), self.a.expr(e).span.end),
                            None => (None, trecho.end),
                        };
                        // O próximo trecho da string (`StrMid`/`StrEnd`).
                        let mut j = self.t.indice(expr_fim);
                        while let Some(t2) = self.t.lista.get(j) {
                            if matches!(t2.kind, Kind::StrMid(..) | Kind::StrEnd(_)) {
                                break;
                            }
                            j += 1;
                        }
                        let Some(t2) = self.t.lista.get(j).copied() else { break };
                        // `${e}`: a `}` é o 1º caractere do trecho seguinte.
                        let fim_interp = match modo {
                            dartforge_frontend::token::Interp::Brace => (t2.span.start + 1).min(t2.span.end),
                            dartforge_frontend::token::Interp::Ident => expr_fim,
                        };
                        elementos.extend(self.no("InterpolationExpression", texto_fim, fim_interp, expr_no.into_iter().collect()));
                        let inicio_texto = fim_interp.max(t2.span.start);
                        match t2.kind {
                            Kind::StrMid(_, m) => {
                                trecho = Span { start: inicio_texto, end: t2.span.end };
                                modo = m;
                                i = j + 1;
                            }
                            _ => {
                                elementos.extend(self.folha("InterpolationString", Span { start: inicio_texto, end: t2.span.end }));
                                fim = t2.span.end;
                                i = j + 1;
                                fechou = true;
                                break;
                            }
                        }
                    }
                    if !fechou {
                        i += 1;
                    }
                    grupos.extend(self.no("StringInterpolation", inicio, fim, elementos));
                }
                _ => i += 1,
            }
        }
        match grupos.len() {
            0 => None,
            1 => Some(grupos[0]),
            _ => {
                let ini = self.nos[grupos[0]].inicio;
                let fim = self.nos[*grupos.last().expect("não vazio")].fim;
                self.no("AdjacentStrings", ini, fim, grupos)
            }
        }
    }

    // -- Padrões ---------------------------------------------------------------

    fn padrao(&mut self, p: PatternId) -> Option<usize> {
        let n = self.padrao_bruta(p);
        self.ligar_a(n, Ligacao::Padrao(p))
    }

    fn padrao_bruta(&mut self, p: PatternId) -> Option<usize> {
        let a = self.a;
        let x = a.pattern(p);
        let s = x.span;
        match &x.kind {
            PatternKind::Wildcard { ty } => {
                let t = ty.and_then(|t| self.tipo(t));
                self.no("WildcardPattern", s.start, s.end, t.into_iter().collect())
            }
            PatternKind::Variable { ty, final_, var_, .. } => {
                if !final_ && !var_ && ty.is_none() {
                    // `x` solto: numa atribuição é `AssignedVariablePattern`;
                    // numa declaração, `DeclaredVariablePattern`.
                    return self.folha("DeclaredVariablePattern", s);
                }
                let t = ty.and_then(|t| self.tipo(t));
                self.no("DeclaredVariablePattern", s.start, s.end, t.into_iter().collect())
            }
            PatternKind::Constant(e) => {
                let f = self.expressao(*e);
                self.no("ConstantPattern", s.start, s.end, f.into_iter().collect())
            }
            PatternKind::Relational { value, .. } => {
                let f = self.expressao(*value);
                self.no("RelationalPattern", s.start, s.end, f.into_iter().collect())
            }
            PatternKind::Or(l, r) | PatternKind::And(l, r) => {
                let especie = if matches!(x.kind, PatternKind::Or(..)) { "LogicalOrPattern" } else { "LogicalAndPattern" };
                let a1 = self.padrao(*l);
                let b1 = self.padrao(*r);
                self.no(especie, s.start, s.end, a1.into_iter().chain(b1).collect())
            }
            PatternKind::NullCheck(i) | PatternKind::NullAssert(i) => {
                let especie = if matches!(x.kind, PatternKind::NullCheck(_)) { "NullCheckPattern" } else { "NullAssertPattern" };
                let f = self.padrao(*i);
                self.no(especie, s.start, s.end, f.into_iter().collect())
            }
            PatternKind::Cast { pattern, ty } => {
                let f = self.padrao(*pattern);
                let t = self.tipo(*ty);
                self.no("CastPattern", s.start, s.end, f.into_iter().chain(t).collect())
            }
            PatternKind::Parenthesized(i) => {
                let f = self.padrao(*i);
                self.no("ParenthesizedPattern", s.start, s.end, f.into_iter().collect())
            }
            PatternKind::List { type_args, elements } => {
                let mut f = Vec::new();
                f.extend(self.argumentos_de_tipo(type_args));
                for el in elements.iter() {
                    match el {
                        ListPatternElement::Pattern(q) => f.extend(self.padrao(*q)),
                        ListPatternElement::Rest(q) => {
                            let fim = q.map(|q| self.a.pattern(q).span.end);
                            let ini = q.map(|q| self.a.pattern(q).span.start);
                            let tres = ini
                                .and_then(|i| self.t.op_antes(i, Op::Ellipsis))
                                .map(|i| self.t.span(i));
                            let inner = q.and_then(|q| self.padrao(q));
                            if let Some(ts) = tres {
                                f.extend(self.no("RestPatternElement", ts.start, fim.unwrap_or(ts.end), inner.into_iter().collect()));
                            }
                        }
                    }
                }
                self.no("ListPattern", s.start, s.end, f)
            }
            PatternKind::Map { type_args, entries, .. } => {
                let mut f = Vec::new();
                f.extend(self.argumentos_de_tipo(type_args));
                for en in entries.iter() {
                    let k = self.expressao(en.key);
                    let v = self.padrao(en.value);
                    let ini = self.a.expr(en.key).span.start;
                    let fim = self.a.pattern(en.value).span.end;
                    f.extend(self.no("MapPatternEntry", ini, fim, k.into_iter().chain(v).collect()));
                }
                self.no("MapPattern", s.start, s.end, f)
            }
            PatternKind::Record { fields } => {
                let f: Vec<usize> = fields.iter().filter_map(|c| self.campo_de_padrao(c)).collect();
                self.no("RecordPattern", s.start, s.end, f)
            }
            PatternKind::Object { ty, fields } => {
                let mut f = Vec::new();
                f.extend(self.tipo(*ty));
                for c in fields.iter() {
                    f.extend(self.campo_de_padrao(c));
                }
                self.no("ObjectPattern", s.start, s.end, f)
            }
        }
    }

    fn campo_de_padrao(&mut self, c: &'a ast::PatternField) -> Option<usize> {
        let ps = self.a.pattern(c.pattern).span;
        let mut f = Vec::new();
        // `PatternFieldName`: `nome:` ou `:` (nome implícito).
        let dois_pontos = match c.name {
            Some(n) => self.t.op_depois(n.span.end, Op::Colon).map(|i| (n.span.start, self.t.span(i).end)),
            None => self.t.op_antes(ps.start, Op::Colon).map(|i| (self.t.span(i).start, self.t.span(i).end)),
        };
        let tem_nome = c.name.is_some() || c.span.start < ps.start;
        if tem_nome && let Some((a, b)) = dois_pontos {
            f.extend(self.no("PatternFieldName", a, b, Vec::new()));
        }
        f.extend(self.padrao(c.pattern));
        self.no("PatternField", c.span.start, c.span.end, f)
    }
}

/// As referências `[ref]` de um comentário de documentação: o conteúdo de
/// `[…]` que é um identificador (com `.` e um `new ` opcional), fora de
/// código entre crases e de blocos cercados, e que não é link
/// (`[texto](url)`, `[texto][ref]`, `[ref]:`). Devolve os intervalos do
/// conteúdo, sem os colchetes.
fn referencias_de_doc(texto: &str) -> Vec<(usize, usize)> {
    let b = texto.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    let mut em_bloco = false;
    let mut inicio_da_linha = true;
    while i < b.len() {
        // Blocos cercados (```) começam numa linha (depois do `///`).
        if inicio_da_linha {
            let resto = &texto[i..];
            let sem_marca = resto.trim_start_matches(['/', '*', ' ', '\t']);
            if sem_marca.starts_with("```") {
                em_bloco = !em_bloco;
            }
        }
        inicio_da_linha = b[i] == b'\n';
        if em_bloco {
            i += 1;
            continue;
        }
        match b[i] {
            b'`' => {
                // Código em linha até a crase seguinte na mesma linha.
                let fim = texto[i + 1..].find(['`', '\n']).map_or(b.len(), |k| i + 1 + k);
                i = fim + 1;
            }
            b'[' => {
                let Some(k) = texto[i + 1..].find([']', '\n', '[']) else { break };
                let fim = i + 1 + k;
                if b.get(fim) != Some(&b']') {
                    i += 1;
                    continue;
                }
                let conteudo = &texto[i + 1..fim];
                let seguinte = b.get(fim + 1).copied();
                let link = matches!(seguinte, Some(b'(') | Some(b'[') | Some(b':'));
                let valido = {
                    let c = conteudo.strip_prefix("new ").map(str::trim_start).unwrap_or(conteudo);
                    !c.is_empty()
                        && c.split('.').all(|p| {
                            let mut cs = p.chars();
                            cs.next().is_some_and(|x| x.is_ascii_alphabetic() || x == '_' || x == '$')
                                && cs.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '$')
                        })
                        && c.split('.').count() <= 3
                };
                if valido && !link {
                    v.push((i + 1, fim));
                }
                i = fim + 1;
            }
            _ => i += 1,
        }
    }
    v
}

/// As faixas de seleção em `offset`: o nó do `NodeLocator` e os ancestrais,
/// sem a unidade, sem repetir o intervalo anterior
/// (`DartSelectionRangeComputer`).
pub fn selecoes(texto: &str, features: dartforge_frontend::LibraryFeatures, offset: usize) -> Vec<Span> {
    let antes_de_3 = features.versao().major < 3;
    let mut nomes = dartforge_intern::Interner::new();
    let analisado = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
    let arvore = construir(texto, &analisado.ast, &analisado.unit, antes_de_3);
    let Some(no) = arvore.localizar(offset, offset) else { return Vec::new() };
    let mut v: Vec<Span> = Vec::new();
    for n in arvore.cadeia(no) {
        let s = arvore.span(n);
        if v.last() != Some(&s) {
            v.push(s);
        }
    }
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cadeia(texto: &str, cursor: &str) -> Vec<String> {
        let offset = texto.find(cursor).expect("cursor") + cursor.len() / 2;
        let features = dartforge_frontend::LibraryFeatures::new(dartforge_frontend::LanguageVersion::ATUAL, &[]);
        selecoes(texto, features, offset).into_iter().map(|s| texto[s.start..s.end].to_string()).collect()
    }

    #[test]
    fn lista_de_parametros_com_parenteses() {
        let t = "int soma(int a, {int b = 0}) => a + b;\n";
        let c = cadeia(t, "int a");
        assert_eq!(c[0], "int");
        assert!(c.contains(&"int a".to_string()));
        assert!(c.contains(&"(int a, {int b = 0})".to_string()));
    }

    #[test]
    fn lista_de_variaveis_de_campo() {
        let t = "class P {\n  final double y = 1;\n}\n";
        let c = cadeia(t, "double");
        assert!(c.contains(&"final double y = 1".to_string()));
        assert!(c.contains(&"final double y = 1;".to_string()));
    }

    #[test]
    fn corpo_de_expressao_com_ponto_e_virgula() {
        let t = "int f() => 1 + 2;\n";
        let c = cadeia(t, "1 +");
        assert!(c.contains(&"=> 1 + 2;".to_string()));
    }

    #[test]
    fn nome_de_anotacao_e_anotacao() {
        let t = "class A {\n  @override\n  String toString() => '';\n}\n";
        let c = cadeia(t, "override");
        assert_eq!(c[0], "override");
        assert_eq!(c[1], "@override");
    }
}

/// As subclasses de `Expression` que a árvore produz.
pub const EXPRESSOES: &[&str] = &[
    "SimpleIdentifier",
    "PrefixedIdentifier",
    "MethodInvocation",
    "PropertyAccess",
    "IntegerLiteral",
    "DoubleLiteral",
    "BooleanLiteral",
    "NullLiteral",
    "SimpleStringLiteral",
    "StringInterpolation",
    "AdjacentStrings",
    "SymbolLiteral",
    "ListLiteral",
    "SetOrMapLiteral",
    "RecordLiteral",
    "InstanceCreationExpression",
    "FunctionExpression",
    "FunctionExpressionInvocation",
    "FunctionReference",
    "IndexExpression",
    "PrefixExpression",
    "PostfixExpression",
    "BinaryExpression",
    "ConditionalExpression",
    "IsExpression",
    "AsExpression",
    "AssignmentExpression",
    "PatternAssignment",
    "CascadeExpression",
    "AwaitExpression",
    "ThrowExpression",
    "RethrowExpression",
    "ThisExpression",
    "SuperExpression",
    "SwitchExpression",
    "ParenthesizedExpression",
    "NamedExpression",
    "DotShorthand",
];

/// As subclasses de `Statement`.
pub const COMANDOS: &[&str] = &[
    "Block",
    "ExpressionStatement",
    "VariableDeclarationStatement",
    "PatternVariableDeclarationStatement",
    "ReturnStatement",
    "IfStatement",
    "ForStatement",
    "WhileStatement",
    "DoStatement",
    "SwitchStatement",
    "TryStatement",
    "BreakStatement",
    "ContinueStatement",
    "YieldStatement",
    "LabeledStatement",
    "EmptyStatement",
    "AssertStatement",
    "FunctionDeclarationStatement",
];

/// A espécie é uma subclasse de `Statement`.
pub fn e_comando(especie: &str) -> bool {
    COMANDOS.contains(&especie)
}

/// A espécie é uma subclasse de `Expression`.
pub fn e_expressao(especie: &str) -> bool {
    EXPRESSOES.contains(&especie)
}
