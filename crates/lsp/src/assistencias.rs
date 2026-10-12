//! Assistências (`refactor.*`) do servidor do Dart 3.6.2 que só reescrevem
//! o documento, com os títulos e espécies dele
//! (`services/correction/dart/*.dart`):
//!
//! | Assistência | Espécie | Quando |
//! | --- | --- | --- |
//! | `Convert to async function body` | `refactor.convert.bodyToAsync` | função ou método (não construtor nem *closure*) sem `async`/`sync*`, cursor antes do corpo ou no `{`/`=>`; `async` e `Future<T>` no retorno escrito |
//! | `Convert to block body` | `refactor.convert.bodyToBlock` | corpo `=> e;`: `{ return e; }` (`e;` se o retorno é `void`) |
//! | `Convert to expression body` | `refactor.convert.bodyToExpression` | corpo com um só `return e;` (ou `e;`), cursor antes de `e`; não construtor gerador |
//! | `Remove type annotation` | `refactor.remove.typeAnnotation` | local com tipo e inicializador (cursor até o fim do nome; o tipo vira `var`, ou some depois de `final`), parâmetro simples com tipo |
//! | `Split variable declaration` | `refactor.splitVariableDeclaration` | local `var`/tipo (não `final`/`const`) com uma variável e inicializador, cursor até o fim do nome: `T x;` + `x = e;` |
//! | `Use curly braces` | `refactor.surround.curlyBraces` | `if`/`else`/`for`/`while`/`do` com corpo sem chaves |
//! | `Assign value to new local variable` | `refactor.assignToVariable` | comando de expressão de tipo não `void`: `var nome = …` |
//!
//! `Inline Local Variable` e `Extract Local Variable` são refatorações por
//! comando (`refatoracoes.rs` e `refatoracoes_exec.rs`).
//!
//! Os nomes novos seguem o `getVariableNameSuggestionsForExpression` do
//! Dart: o da expressão (identificador, propriedade, método sem
//! `get`/`is`/`to`, classe criada), senão o do tipo (`i`, `d`, `s`, a
//! classe em camelCase), evitando os locais visíveis.

use crate::acoes::AcaoDeCodigo;
use crate::projeto::Projeto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, ExprId, ExprKind, FunctionBody, StmtId, StmtKind};
use dartforge_types::{Type, TypeId};
use std::collections::HashSet;

fn acao(uri: &str, titulo: &str, especie: &str, edicoes: Vec<(Span, String)>) -> AcaoDeCodigo {
    AcaoDeCodigo {
        titulo: titulo.into(),
        especie: especie.into(),
        edicoes: edicoes.into_iter().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
        diagnostico: None,
        criar_arquivo: None, ligado: Default::default(),
    }
}

/// A indentação da linha que contém `offset`.
fn indentacao(fonte: &str, offset: usize) -> String {
    let ini = fonte[..offset.min(fonte.len())].rfind('\n').map_or(0, |i| i + 1);
    fonte[ini..].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

impl Projeto {
    /// As assistências desta lista em `inicio..fim` de `unidade`.
    pub(crate) fn assistencias_de_reescrita(&self, uri: &str, unidade: UnitId, inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
        let mut saida = Vec::new();
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_str();
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let offset = inicio;
        let _ = fim;

        // A expressão de função mais interna que contém o cursor.
        // O trecho de cada função conta com os metadados da declaração dona
        // (`@override` antes de um método: o `getEnclosingFunctionBody` do
        // Dart sobe da anotação ao método).
        let trecho = |i: usize, f: &ast::Function| -> Span {
            let dono = ast
                .members
                .iter()
                .find(|m| matches!(m.kind, ast::MemberKind::Method(x) if x.0 as usize == i))
                .map(|m| m.span)
                .or_else(|| ast.decls.iter().find(|d| matches!(d.kind, ast::DeclKind::Function(x) if x.0 as usize == i)).map(|d| d.span));
            match dono {
                Some(s) if s.start < f.span.start => Span { start: s.start, end: f.span.end },
                _ => f.span,
            }
        };
        let interna = ast
            .functions
            .iter()
            .enumerate()
            .map(|(i, f)| (i, f, trecho(i, f)))
            .filter(|(_, _, s)| s.start <= offset && offset <= s.end)
            .min_by_key(|(_, _, s)| s.end - s.start)
            .map(|(i, f, _)| (i, f));

        // `Convert to async function body`: `assist_funcoes` (o produtor do Dart).

        // Convert to expression body: a função mais interna.
        if let Some((_, f)) = interna {
            match f.body {
                // `Convert to block body` sai pelo porte do produtor do Dart
                // (`Contexto::converter_em_corpo_de_bloco`).
                FunctionBody::Block(b) if f.modifier != ast::AsyncModifier::SyncStar && f.modifier != ast::AsyncModifier::AsyncStar => {
                    if let StmtKind::Block(cmds) = &ast.stmt(b).kind
                        && cmds.len() == 1
                    {
                        let unico = ast.stmt(cmds[0]);
                        let expr = match &unico.kind {
                            StmtKind::Return(Some(e)) | StmtKind::Expression(e) => Some(*e),
                            _ => None,
                        };
                        if let Some(e) = expr
                            && offset < ast.expr(e).span.start
                        {
                            let es = ast.expr(e).span;
                            let bs = ast.stmt(b).span;
                            let async_ = if f.modifier == ast::AsyncModifier::Async { "async " } else { "" };
                            let ponto_e_virgula = if f.name.is_some() { ";" } else { "" };
                            // O `async` já escrito fica; troca só o bloco.
                            let _ = async_;
                            saida.push(acao(
                                uri,
                                "Convert to expression body",
                                "refactor.convert.bodyToExpression",
                                vec![(bs, format!("=> {}{ponto_e_virgula}", &fonte[es.start..es.end]))],
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // Comando local que contém o cursor.
        let comando = ast
            .stmts
            .iter()
            .enumerate()
            .filter(|(_, s)| s.span.start <= offset && offset <= s.span.end && !matches!(s.kind, StmtKind::Block(_)))
            .min_by_key(|(_, s)| s.span.end - s.span.start)
            .map(|(i, _)| StmtId(i as u32));

        if let Some(cid) = comando {
            let s = ast.stmt(cid);
            match &s.kind {
                StmtKind::Variables(l) => {
                    // `Remove type annotation`: `assist_tipo` (o produtor do Dart).
                    // Split variable declaration.
                    if !l.final_ && !l.const_
                        && l.variables.len() == 1
                        && let Some(v) = l.variables.first()
                        && v.initializer.is_some()
                        && offset <= v.name.span.end
                    {
                        let nome = &fonte[v.name.span.start..v.name.span.end];
                        let prefixo = indentacao(fonte, s.span.start);
                        let mut edicoes = Vec::new();
                        if l.ty.is_none()
                            && let Some(t) = corpos.tipo_local(v.name.span.start)
                            && !matches!(self.consulta.tabela.get(t), Type::Dynamic)
                            && let Some(k) = fonte[s.span.start..v.name.span.start].find("var")
                        {
                            let p = s.span.start + k;
                            edicoes.push((Span { start: p, end: p + 3 }, self.consulta.formatar(t)));
                        }
                        edicoes.push((Span { start: v.name.span.end, end: v.name.span.end }, format!(";\n{prefixo}{nome}")));
                        saida.push(acao(uri, "Split variable declaration", "refactor.splitVariableDeclaration", edicoes));
                    }
                }
                StmtKind::Expression(e) => {
                    // Assign value to new local variable.
                    let tipo = corpos.get_type(*e);
                    let atribuicao = matches!(ast.expr(*e).kind, ExprKind::Assign { .. } | ExprKind::Throw(_));
                    if !atribuicao
                        && let Some(t) = tipo
                        && !matches!(self.consulta.tabela.get(t), Type::Void)
                    {
                        // `AssignToLocalVariable`: as sugestões de
                        // `getVariableNameSuggestionsForExpression` com os
                        // nomes do `ScopedNameFinder` excluídos; a primeira é
                        // escrita, todas vão para o grupo ligado `NAME`.
                        let es = ast.expr(*e).span;
                        let cx = crate::refatoracoes::Contexto::novo(self, unidade);
                        let sugestoes = match cx.arvore.localizar(es.start, es.end) {
                            Some(n) => {
                                let excluidos = cx.nomes_em_escopo(n, es.start);
                                cx.nomes_para_expressao(Some(t), n, &excluidos, false)
                            }
                            None => vec![self.nome_sugerido(unidade, *e, Some(t), offset)],
                        };
                        if let Some(nome) = sugestoes.first().cloned() {
                            let chave = if crate::refatoracoes_exec::regra_ligada(self, unidade, "prefer_final_locals") { "final" } else { "var" };
                            let texto = format!("{chave} {nome} = ");
                            let a = acao(uri, "Assign value to new local variable", "refactor.assignToVariable", vec![(Span { start: es.start, end: es.start }, texto)]);
                            saida.push(a.com_grupo(0, &[chave.len() + 1], nome.len(), sugestoes));
                        }
                    }
                }
                _ => {}
            }
            // Use curly braces (o comando ou o pai).
            if let Some(acao_chaves) = self.usar_chaves(uri, unidade, cid, offset) {
                saida.push(acao_chaves);
            }
        }

        // Remove type annotation num parâmetro simples com tipo.
        for f in &ast.functions {
            for p in f.parameters.iter().flatten() {
                if let (Some(t), Some(n)) = (p.ty, p.name)
                    && !p.this_
                    && !p.super_
                    && p.function_parameters.is_none()
                    && p.span.start <= offset
                    && offset <= n.span.end
                {
                    let ts = ast.ty(t).span;
                    let resto = &fonte[ts.end..];
                    let fim_tipo = ts.end + (resto.len() - resto.trim_start().len());
                    let substituto = if p.final_ || p.var_ { String::new() } else { String::new() };
                    saida.push(acao(uri, "Remove type annotation", "refactor.remove.typeAnnotation", vec![(Span { start: ts.start, end: fim_tipo }, substituto)]));
                }
            }
        }

        // `Inline Local Variable` e `Extract Local Variable` são refatorações
        // por comando (`refatoracoes.rs`), não assistências.
        saida
    }

    /// `Use curly braces` no comando `cid` (ou no pai dele): o porte de
    /// `UseCurlyBraces` (`use_curly_braces.dart`). O comando e o pai são
    /// examinados na ordem do Dart (`do`, `for`, `if`, `while`); cada corpo
    /// sem chaves recebe duas edições: ` {<eol><recuo>` no lugar do espaço
    /// entre o token da esquerda e o corpo (os comentários antes do corpo
    /// ficam), e `<eol><prefixo>}` depois do corpo (e do comentário na mesma
    /// linha), ou `<eol><prefixo>} ` até o `else`/`while` seguinte.
    fn usar_chaves(&self, uri: &str, unidade: UnitId, cid: StmtId, offset: usize) -> Option<AcaoDeCodigo> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_str();
        let pai = ast
            .stmts
            .iter()
            .position(|s| match &s.kind {
                StmtKind::If { then, else_, .. } => *then == cid || *else_ == Some(cid),
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } | StmtKind::DoWhile { body, .. } => *body == cid,
                _ => false,
            })
            .map(|i| StmtId(i as u32));
        let tokens = dartforge_frontend::lexer::lex(fonte).ok()?;
        // O fim do último token antes de `o` e o início do primeiro depois.
        let antes = |o: usize| tokens.iter().take_while(|t| t.span.end <= o).last().map_or(0, |t| t.span.end);
        let depois = |o: usize| tokens.iter().find(|t| t.span.start >= o).map(|t| t.span);
        let eol = if fonte.contains("\r\n") { "\r\n" } else { "\n" };
        let e_bloco = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::Block(_));
        let e_do = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::DoWhile { .. });
        let e_for = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::For { .. } | StmtKind::ForIn { .. });
        let e_if = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::If { .. });
        let e_while = |s: StmtId| matches!(ast.stmt(s).kind, StmtKind::While { .. });
        let corpo = |s: StmtId| match &ast.stmt(s).kind {
            StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } | StmtKind::DoWhile { body, .. } => Some(*body),
            _ => None,
        };
        // `_endAfterComments`: o fim do corpo, ou do comentário que começa na
        // mesma linha logo depois dele.
        let fim_com_comentario = |fim: usize| -> usize {
            let resto = &fonte[fim..];
            let espacos = resto.len() - resto.trim_start_matches([' ', '\t']).len();
            let c = &resto[espacos..];
            if c.starts_with("//") {
                fim + espacos + c.find(['\n', '\r']).unwrap_or(c.len())
            } else if c.starts_with("/*") {
                c.find("*/").map_or(fim, |k| fim + espacos + k + 2)
            } else {
                fim
            }
        };
        let mut edicoes: Vec<(Span, String)> = Vec::new();
        // `_replaceLeftParenthesis`: do fim do token da esquerda ao corpo (ou
        // ao primeiro comentário antes dele).
        let esquerda = |edicoes: &mut Vec<(Span, String)>, corpo: StmtId, recuo: &str| {
            let b = ast.stmt(corpo).span.start;
            let a = antes(b);
            let entre = &fonte[a..b];
            let inicio = a + (entre.len() - entre.trim_start().len());
            edicoes.push((Span { start: a, end: inicio.min(b) }, format!(" {{{eol}{recuo}")));
        };
        let mut so: Option<StmtId> = None;
        let alvo = if e_do(cid) {
            cid
        } else if pai.is_some_and(e_do) {
            pai?
        } else if e_for(cid) {
            cid
        } else if pai.is_some_and(e_for) {
            pai?
        } else if e_if(cid) {
            if let StmtKind::If { then, else_: Some(e), .. } = &ast.stmt(cid).kind
                && let Some(k) = depois(ast.stmt(*then).span.end)
                && &fonte[k.start..k.end] == "else"
                && k.start <= offset
                && offset <= k.end
            {
                so = Some(*e);
            }
            cid
        } else if pai.is_some_and(e_if) {
            so = Some(cid);
            pai?
        } else if e_while(cid) {
            cid
        } else if pai.is_some_and(e_while) {
            pai?
        } else {
            return None;
        };
        let alvo_stmt = ast.stmt(alvo);
        let prefixo = indentacao(fonte, alvo_stmt.span.start);
        let recuo = format!("{prefixo}  ");
        match &alvo_stmt.kind {
            StmtKind::If { then, else_, .. } => {
                // O `else if` de uma cadeia não muda.
                if ast.stmts.iter().any(|s| matches!(&s.kind, StmtKind::If { else_: Some(e), .. } if *e == alvo)) {
                    return None;
                }
                let palavra_else = else_.and_then(|_| depois(ast.stmt(*then).span.end)).filter(|k| &fonte[k.start..k.end] == "else");
                if !e_bloco(*then) && so.is_none_or(|x| x == *then) {
                    esquerda(&mut edicoes, *then, &recuo);
                    let fim = fim_com_comentario(ast.stmt(*then).span.end);
                    match palavra_else {
                        None => edicoes.push((Span { start: fim, end: fim }, format!("{eol}{prefixo}}}"))),
                        Some(k) => edicoes.push((Span { start: fim, end: k.start }, format!("{eol}{prefixo}}} "))),
                    }
                }
                if let (Some(e), Some(_)) = (else_, palavra_else)
                    && !e_bloco(*e)
                    && !e_if(*e)
                    && so.is_none_or(|x| x == *e)
                {
                    esquerda(&mut edicoes, *e, &recuo);
                    let fim = fim_com_comentario(ast.stmt(*e).span.end);
                    edicoes.push((Span { start: fim, end: fim }, format!("{eol}{prefixo}}}")));
                }
            }
            StmtKind::DoWhile { body, .. } => {
                if e_bloco(*body) {
                    return None;
                }
                esquerda(&mut edicoes, *body, &recuo);
                let fim = fim_com_comentario(ast.stmt(*body).span.end);
                let k = depois(ast.stmt(*body).span.end)?;
                edicoes.push((Span { start: fim, end: k.start.max(fim) }, format!("{eol}{prefixo}}} ")));
            }
            _ => {
                let b = corpo(alvo)?;
                if e_bloco(b) {
                    return None;
                }
                esquerda(&mut edicoes, b, &recuo);
                let fim = fim_com_comentario(ast.stmt(b).span.end);
                edicoes.push((Span { start: fim, end: fim }, format!("{eol}{prefixo}}}")));
            }
        }
        if edicoes.is_empty() {
            return None;
        }
        Some(acao(uri, "Use curly braces", "refactor.surround.curlyBraces", edicoes))
    }

    /// O nome que o Dart sugere para guardar `expr`.
    fn nome_sugerido(&self, unidade: UnitId, expr: ExprId, tipo: Option<TypeId>, offset: usize) -> String {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_str();
        let texto = |s: Span| fonte[s.start..s.end].to_string();
        let mut e = expr;
        loop {
            match &ast.expr(e).kind {
                ExprKind::Parenthesized(x) | ExprKind::As { value: x, .. } => e = *x,
                _ => break,
            }
        }
        let base: Option<String> = match &ast.expr(e).kind {
            ExprKind::Identifier(n) => Some(texto(n.span)),
            ExprKind::Property { name, .. } => Some(texto(name.span)),
            ExprKind::Call { target, .. } => match &ast.expr(*target).kind {
                ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                    let n = texto(n.span);
                    let sem = ["get", "is", "to"].iter().find_map(|p| n.strip_prefix(p).filter(|r| r.starts_with(char::is_uppercase)).map(str::to_string));
                    Some(sem.unwrap_or(n))
                }
                _ => None,
            },
            ExprKind::InstanceCreation { ty, .. } => match &ast.ty(*ty).kind {
                ast::TypeKind::Named { name, .. } => name.last().map(|n| texto(n.span)),
                _ => None,
            },
            _ => None,
        };
        let pelo_tipo = || -> Option<String> {
            let t = tipo?;
            if t == self.consulta.core.int {
                return Some("i".into());
            }
            match self.consulta.tabela.get(t) {
                Type::Interface { class, .. } => {
                    let n = self.nome(self.programa().class(*class).name);
                    Some(match n {
                        "double" => "d".into(),
                        "String" => "s".into(),
                        "int" => "i".into(),
                        _ => n.to_string(),
                    })
                }
                _ => None,
            }
        };
        let base = base.or_else(pelo_tipo).unwrap_or_else(|| "value".into());
        let base = base.trim_start_matches('_').to_string();
        let mut nome: String = base.chars().next().map(|c| c.to_lowercase().collect::<String>() + &base[c.len_utf8()..]).unwrap_or(base);
        // Os locais e parâmetros declarados antes não podem ser repetidos.
        let visiveis: HashSet<String> = ast
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Variables(l) => Some(l.variables.iter().map(|v| v.name).collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .chain(ast.functions.iter().flat_map(|f| f.parameters.iter().flatten().filter_map(|p| p.name)))
            .filter(|n| n.span.start < offset)
            .map(|n| texto(n.span))
            .collect();
        if visiveis.contains(&nome) {
            let base = nome.clone();
            let mut k = 2;
            while visiveis.contains(&nome) {
                nome = format!("{base}{k}");
                k += 1;
            }
        }
        nome
    }
}
