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
        criar_arquivo: None,
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
        let interna = ast
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| f.span.start <= offset && offset <= f.span.end)
            .min_by_key(|(_, f)| f.span.end - f.span.start);

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
                        let nome = self.nome_sugerido(unidade, *e, Some(t), offset);
                        let p = ast.expr(*e).span.start;
                        saida.push(acao(uri, "Assign value to new local variable", "refactor.assignToVariable", vec![(Span { start: p, end: p }, format!("var {nome} = "))]));
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

    /// `Use curly braces` no comando `cid` (ou no pai dele).
    fn usar_chaves(&self, uri: &str, unidade: UnitId, cid: StmtId, offset: usize) -> Option<AcaoDeCodigo> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_str();
        let pai = ast.stmts.iter().position(|s| match &s.kind {
            StmtKind::If { then, else_, .. } => *then == cid || *else_ == Some(cid),
            StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } | StmtKind::DoWhile { body, .. } => *body == cid,
            _ => false,
        });
        let alvo = match &ast.stmt(cid).kind {
            StmtKind::If { .. } | StmtKind::For { .. } | StmtKind::ForIn { .. } | StmtKind::While { .. } | StmtKind::DoWhile { .. } => cid,
            _ => StmtId(pai? as u32),
        };
        let s = ast.stmt(alvo);
        let corpos_sem_chaves: Vec<StmtId> = match &s.kind {
            StmtKind::If { then, else_, .. } => {
                // Cursor no `else`: só o `else`; senão o `then` (e o `else`
                // sem chaves que não seja outro `if`).
                let mut v = vec![*then];
                if let Some(e) = else_
                    && !matches!(ast.stmt(*e).kind, StmtKind::If { .. })
                {
                    let antes_do_else = fonte[ast.stmt(*then).span.end..ast.stmt(*e).span.start].find("else").map(|k| ast.stmt(*then).span.end + k);
                    if antes_do_else.is_some_and(|k| k <= offset && offset <= k + 4) {
                        v = vec![*e];
                    }
                }
                v
            }
            StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } | StmtKind::DoWhile { body, .. } => vec![*body],
            _ => return None,
        };
        let corpos_sem_chaves: Vec<StmtId> = corpos_sem_chaves.into_iter().filter(|b| !matches!(ast.stmt(*b).kind, StmtKind::Block(_))).collect();
        if corpos_sem_chaves.is_empty() {
            return None;
        }
        let prefixo = indentacao(fonte, s.span.start);
        let mut edicoes = Vec::new();
        for b in corpos_sem_chaves {
            let bs = ast.stmt(b).span;
            let antes = fonte[..bs.start].trim_end().len();
            let dentro = &fonte[bs.start..bs.end];
            edicoes.push((Span { start: antes, end: bs.end }, format!(" {{\n{prefixo}  {dentro}\n{prefixo}}}")));
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
