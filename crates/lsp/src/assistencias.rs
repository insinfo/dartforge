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
//! | `Inline Local Variable` | `refactor.inline` | local com inicializador e sem outra atribuição, cursor na declaração ou num uso |
//! | `Extract Local Variable` | `refactor.extract` | expressão (não alvo de atribuição) num corpo de função: `var nome = e;` antes do comando |
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

        // A função declarada (não *closure*) que contém o cursor, e a
        // expressão de função mais interna.
        let metodos: HashSet<u32> = ast
            .members
            .iter()
            .filter_map(|m| match m.kind {
                ast::MemberKind::Method(f) => Some(f.0),
                _ => None,
            })
            .collect();
        let declarada = ast
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| f.name.is_some() && f.span.start <= offset && offset <= f.span.end)
            .min_by_key(|(_, f)| f.span.end - f.span.start);
        let interna = ast
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| f.span.start <= offset && offset <= f.span.end)
            .min_by_key(|(_, f)| f.span.end - f.span.start);

        // Convert to async function body.
        if let Some((i, f)) = declarada
            && interna.is_some_and(|(j, _)| j == i)
            && f.modifier == ast::AsyncModifier::None
        {
            let corpo_inicio = match f.body {
                FunctionBody::Block(b) => Some(ast.stmt(b).span.start),
                FunctionBody::Expression(e) => fonte[..ast.expr(e).span.start].rfind("=>"),
                _ => None,
            };
            if let Some(c) = corpo_inicio
                && offset <= c + if matches!(f.body, FunctionBody::Expression(_)) { 2 } else { 1 }
            {
                let mut edicoes = vec![(Span { start: c, end: c }, "async ".to_string())];
                if let Some(t) = f.return_type {
                    let s = ast.ty(t).span;
                    let escrito = &fonte[s.start..s.end];
                    if !escrito.starts_with("Future") && !escrito.starts_with("FutureOr") {
                        edicoes.push((s, format!("Future<{escrito}>")));
                    }
                }
                saida.push(acao(uri, "Convert to async function body", "refactor.convert.bodyToAsync", edicoes));
            }
            let _ = metodos.contains(&(i as u32));
        }

        // Convert to block body / expression body: a função mais interna.
        if let Some((_, f)) = interna {
            match f.body {
                FunctionBody::Expression(e) if f.modifier != ast::AsyncModifier::SyncStar && f.modifier != ast::AsyncModifier::AsyncStar => {
                    let es = ast.expr(e).span;
                    if let Some(seta) = fonte[..es.start].rfind("=>") {
                        let fim_corpo = fonte[es.end..].find(';').filter(|k| fonte[es.end..es.end + k].trim().is_empty()).map_or(es.end, |k| es.end + k + 1);
                        let prefixo = indentacao(fonte, f.span.start);
                        let vazio = f.return_type.is_some_and(|t| &fonte[ast.ty(t).span.start..ast.ty(t).span.end] == "void");
                        let codigo = &fonte[es.start..es.end];
                        let linha = if vazio { format!("{codigo};") } else { format!("return {codigo};") };
                        let inicio_seta = fonte[..seta].trim_end().len();
                        let fecho = if f.name.is_some() || fonte.as_bytes().get(fim_corpo.saturating_sub(1)) == Some(&b';') { "" } else { "" };
                        let novo = format!(" {{\n{prefixo}  {linha}\n{prefixo}}}{fecho}");
                        // O `;` do corpo de expressão sai junto (só em
                        // declarações; num *closure* não há `;`).
                        let fim_trocado = if f.name.is_some() { fim_corpo } else { es.end };
                        saida.push(acao(uri, "Convert to block body", "refactor.convert.bodyToBlock", vec![(Span { start: inicio_seta, end: fim_trocado }, novo)]));
                    }
                }
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
                    // Remove type annotation.
                    if let (Some(t), Some(v)) = (l.ty, l.variables.first())
                        && v.initializer.is_some()
                        && offset <= v.name.span.end
                    {
                        let ts = ast.ty(t).span;
                        let edicao = if l.final_ || l.const_ {
                            let resto = &fonte[ts.end..];
                            (Span { start: ts.start, end: ts.end + (resto.len() - resto.trim_start().len()) }, String::new())
                        } else {
                            (ts, "var".to_string())
                        };
                        saida.push(acao(uri, "Remove type annotation", "refactor.remove.typeAnnotation", vec![edicao]));
                    }
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

        // Inline Local Variable e Extract Local Variable.
        saida.extend(self.embutir_local(uri, unidade, offset));
        saida.extend(self.extrair_local(uri, unidade, offset));
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

    /// `Inline Local Variable`: troca cada uso pelo inicializador e apaga a
    /// declaração, quando o local não é atribuído de novo.
    fn embutir_local(&self, uri: &str, unidade: UnitId, offset: usize) -> Option<AcaoDeCodigo> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_str();
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        // A declaração: o nome sob o cursor ou o local que um uso refere.
        let uso = ast.exprs.iter().position(|e| matches!(&e.kind, ExprKind::Identifier(n) if n.span.start <= offset && offset <= n.span.end));
        let decl = uso.and_then(|i| corpos.declaracao_local(ExprId(i as u32)));
        let (sid, l, var) = ast.stmts.iter().enumerate().find_map(|(i, s)| match &s.kind {
            StmtKind::Variables(l) if l.variables.len() == 1 => {
                let v = &l.variables[0];
                let casa = decl == Some(v.name.span.start) || (v.name.span.start <= offset && offset <= v.name.span.end);
                casa.then_some((i, l, v))
            }
            _ => None,
        })?;
        let inicializador = var.initializer?;
        let decl_offset = var.name.span.start;
        let mut usos = Vec::new();
        for (i, e) in ast.exprs.iter().enumerate() {
            if corpos.declaracao_local(ExprId(i as u32)) != Some(decl_offset) || !matches!(e.kind, ExprKind::Identifier(_)) {
                continue;
            }
            // Atribuição ao local: não embute.
            if ast.exprs.iter().any(|x| matches!(&x.kind, ExprKind::Assign { target, .. } if target.0 as usize == i)) {
                return None;
            }
            usos.push(e.span);
        }
        let _ = l;
        let is = ast.expr(inicializador).span;
        let texto = &fonte[is.start..is.end];
        let simples = matches!(ast.expr(inicializador).kind, ExprKind::Identifier(_) | ExprKind::Property { .. } | ExprKind::Call { .. } | ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::String(_) | ExprKind::Bool(_) | ExprKind::Null | ExprKind::Parenthesized(_) | ExprKind::InstanceCreation { .. } | ExprKind::List { .. } | ExprKind::SetOrMap { .. } | ExprKind::This);
        let substituto = if simples { texto.to_string() } else { format!("({texto})") };
        let s = ast.stmt(StmtId(sid as u32)).span;
        let ini_linha = fonte[..s.start].rfind('\n').map_or(0, |k| k + 1);
        let fim_linha = fonte[s.end..].find('\n').map_or(fonte.len(), |k| s.end + k + 1);
        let apagar = if fonte[ini_linha..s.start].trim().is_empty() && fonte[s.end..fim_linha].trim().is_empty() {
            Span { start: ini_linha, end: fim_linha }
        } else {
            s
        };
        let mut edicoes = vec![(apagar, String::new())];
        edicoes.extend(usos.into_iter().map(|span| (span, substituto.clone())));
        Some(acao(uri, "Inline Local Variable", "refactor.inline", edicoes))
    }

    /// `Extract Local Variable`: a expressão sob o cursor (a mais interna que
    /// não é nome declarado nem alvo de atribuição) vira um local antes do
    /// comando.
    fn extrair_local(&self, uri: &str, unidade: UnitId, offset: usize) -> Option<AcaoDeCodigo> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_str();
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let alvos_de_atribuicao: HashSet<u32> = ast
            .exprs
            .iter()
            .filter_map(|e| match &e.kind {
                ExprKind::Assign { target, .. } => Some(target.0),
                _ => None,
            })
            .collect();
        // O comando do bloco que contém o cursor (onde o local entra).
        let comando = ast
            .stmts
            .iter()
            .filter(|s| s.span.start <= offset && offset <= s.span.end && !matches!(s.kind, StmtKind::Block(_)))
            .filter(|s| {
                ast.stmts.iter().any(|b| match &b.kind {
                    StmtKind::Block(cmds) => cmds.iter().any(|c| ast.stmt(*c).span == s.span),
                    _ => false,
                })
            })
            .min_by_key(|s| s.span.end - s.span.start)?;
        let (eid, e) = ast
            .exprs
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                e.span.start <= offset
                    && offset <= e.span.end
                    && comando.span.start <= e.span.start
                    && e.span.end <= comando.span.end
                    && !alvos_de_atribuicao.contains(&(*i as u32))
                    && !matches!(e.kind, ExprKind::Assign { .. } | ExprKind::FunctionExpression(_) | ExprKind::CascadeTarget)
            })
            .min_by_key(|(_, e)| e.span.end - e.span.start)?;
        // Alvo de chamada (`f` em `f(x)`): extrai a chamada inteira.
        let (eid, e) = ast
            .exprs
            .iter()
            .enumerate()
            .find(|(_, x)| matches!(&x.kind, ExprKind::Call { target, .. } if target.0 as usize == eid))
            .unwrap_or((eid, e));
        let tipo = corpos.get_type(ExprId(eid as u32));
        if tipo.is_some_and(|t| matches!(self.consulta.tabela.get(t), Type::Void)) {
            return None;
        }
        let nome = self.nome_sugerido(unidade, ExprId(eid as u32), tipo, offset);
        let prefixo = indentacao(fonte, comando.span.start);
        let texto = &fonte[e.span.start..e.span.end];
        let decl = format!("var {nome} = {texto};\n{prefixo}");
        Some(acao(
            uri,
            "Extract Local Variable",
            "refactor.extract",
            vec![(Span { start: comando.span.start, end: comando.span.start }, decl), (e.span, nome)],
        ))
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
