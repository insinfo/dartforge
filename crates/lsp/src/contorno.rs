//! O contorno do documento no formato do servidor do Dart
//! (`dart/textDocument/publishOutline`): o `Outline` de
//! `DartUnitOutlineComputer` convertido por `toOutline` e `toElement`
//! (`AS:src/computer/computer_outline.dart` e `AS:src/lsp/mapping.dart:1197-1317`
//! da 3.6.2, lidos por inteiro; docs/LSP-ESPECIFICACAO.md §3.4):
//! `{element: {name, kind, range, parameters, typeParameters, returnType},
//! range, codeRange, children}`.
//!
//! `range` é o nó com o comentário de documentação e as anotações;
//! `codeRange` começa no primeiro token depois deles. Os textos de
//! parâmetros, parâmetros de tipo e tipos são os da fonte, sem a
//! normalização do `toSource`. A árvore é a do parser (sem resolução): os
//! nós de `group`/`test` seguem a regra de `crate::simbolos`, e não há os nós
//! de criação de widget do Flutter.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::utf16::TabelaLinhas;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    Annotation, Ast, DeclKind, DirectiveKind, ExprKind, FunctionId, FunctionKind, MemberId, MemberKind, Name, StmtKind, TypeKind,
    TypedefKind, VariableList,
};
use dartforge_intern::Interner;
use serde_json::{Value, json};

struct Ctx<'a> {
    texto: &'a str,
    linhas: TabelaLinhas,
    nomes: &'a Interner,
    ast: &'a Ast,
    testes: bool,
}

/// O par balanceado `abre`…`fecha` que começa em `de` (depois de brancos):
/// o intervalo com os delimitadores.
fn balanceado(texto: &str, de: usize, abre: u8, fecha: u8) -> Option<Span> {
    let b = texto.as_bytes();
    let mut i = de;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    if b.get(i) != Some(&abre) {
        return None;
    }
    let inicio = i;
    let mut nivel = 0usize;
    while i < b.len() {
        if b[i] == abre {
            nivel += 1;
        } else if b[i] == fecha {
            nivel -= 1;
            if nivel == 0 {
                return Some(Span { start: inicio, end: i + 1 });
            }
        }
        i += 1;
    }
    None
}

impl Ctx<'_> {
    fn trecho(&self, s: Span) -> &str {
        self.texto.get(s.start..s.end).unwrap_or("")
    }

    fn faixa(&self, s: Span) -> Value {
        let (l0, c0) = self.linhas.posicao_de_offset(self.texto, s.start.min(self.texto.len()));
        let (l1, c1) = self.linhas.posicao_de_offset(self.texto, s.end.clamp(s.start, self.texto.len()));
        json!({"start": {"line": l0, "character": c0}, "end": {"line": l1, "character": c1}})
    }

    /// O começo do nó com o comentário de documentação que o precede: as
    /// linhas `///` contíguas, ou um bloco `/** */`.
    fn com_documentacao(&self, inicio: usize) -> usize {
        let mut atual = inicio;
        loop {
            let antes = self.texto[..atual].trim_end();
            if antes.ends_with("*/") {
                return match antes[..antes.len() - 2].rfind("/**") {
                    Some(k) => k,
                    None => atual,
                };
            }
            let linha_ini = antes.rfind('\n').map_or(0, |i| i + 1);
            let linha = &antes[linha_ini..];
            let recuo = linha.len() - linha.trim_start().len();
            if linha.trim_start().starts_with("///") {
                atual = linha_ini + recuo;
            } else {
                return atual;
            }
        }
    }

    /// `firstTokenAfterCommentAndMetadata`: depois da última anotação.
    fn depois_das_anotacoes(&self, inicio: usize, metadata: &[Annotation]) -> usize {
        let fim = metadata.iter().map(|m| m.span.end).max().unwrap_or(inicio).max(inicio);
        let resto = &self.texto[fim.min(self.texto.len())..];
        fim + (resto.len() - resto.trim_start().len())
    }

    /// `_nodeOutline` de um nó anotado.
    fn no(&self, elemento: Value, span: Span, metadata: &[Annotation], filhos: Vec<Value>) -> Value {
        let inicio = self.com_documentacao(span.start);
        let codigo = self.depois_das_anotacoes(span.start, metadata);
        let mut v = json!({
            "element": elemento,
            "range": self.faixa(Span { start: inicio, end: span.end }),
            "codeRange": self.faixa(Span { start: codigo.min(span.end), end: span.end }),
        });
        if !filhos.is_empty() {
            v["children"] = json!(filhos);
        }
        v
    }

    /// `toElement`: os campos nulos não saem.
    fn elemento(&self, especie: &str, nome: &str, local: Option<Span>, parametros: Option<&str>, de_tipo: Option<&str>, retorno: Option<&str>) -> Value {
        let mostrado = if !nome.is_empty() {
            nome
        } else if especie == "EXTENSION" {
            "<unnamed extension>"
        } else {
            "<unnamed>"
        };
        let mut v = json!({"name": mostrado, "kind": especie});
        if let Some(s) = local {
            v["range"] = self.faixa(s);
        }
        if let Some(p) = parametros {
            v["parameters"] = json!(p);
        }
        if let Some(t) = de_tipo {
            v["typeParameters"] = json!(t);
        }
        if let Some(r) = retorno {
            v["returnType"] = json!(r);
        }
        v
    }

    /// Os parâmetros de tipo `<…>` escritos logo depois de `depois_de`.
    fn parametros_de_tipo(&self, depois_de: usize) -> Option<Span> {
        balanceado(self.texto, depois_de, b'<', b'>')
    }

    /// A lista `(…)` escrita depois do nome e dos parâmetros de tipo.
    fn lista_de_parametros(&self, depois_de: usize) -> Option<Span> {
        let de = self.parametros_de_tipo(depois_de).map_or(depois_de, |s| s.end);
        balanceado(self.texto, de, b'(', b')')
    }

    fn tipo(&self, t: Option<dartforge_frontend::ast::TypeId>) -> &str {
        t.map_or("", |t| self.trecho(self.ast.ty(t).span))
    }

    /// As funções locais e os testes de um corpo (`_FunctionBodyOutlinesVisitor`).
    fn do_corpo(&self, dentro: Span) -> Vec<Value> {
        struct Item {
            span: Span,
            valor: Value,
            folha: bool,
        }
        let a = self.ast;
        let mut itens: Vec<Item> = Vec::new();
        for s in a.stmts.iter() {
            let StmtKind::Function(f) = &s.kind else { continue };
            // Só as estritamente dentro: o próprio nó não é filho de si.
            if s.span.start < dentro.start || s.span.end > dentro.end || s.span == dentro {
                continue;
            }
            if let Some(v) = self.funcao(*f, s.span, &[], "FUNCTION", false) {
                itens.push(Item { span: s.span, valor: v, folha: false });
            }
        }
        if self.testes {
            for e in a.exprs.iter() {
                let ExprKind::Call { target, arguments } = &e.kind else { continue };
                if e.span.start < dentro.start || e.span.end > dentro.end {
                    continue;
                }
                let ExprKind::Identifier(nome) = &a.expr(*target).kind else { continue };
                let chamada = self.nomes.resolve(nome.sym);
                if chamada != "group" && chamada != "test" {
                    continue;
                }
                let descricao = match arguments.args.first() {
                    None => "unnamed".to_string(),
                    Some(primeiro) => {
                        let expr = a.expr(primeiro.value);
                        let valor = match &expr.kind {
                            ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
                            _ => None,
                        };
                        valor.unwrap_or_else(|| self.trecho(expr.span).to_string())
                    }
                };
                let especie = if chamada == "group" { "UNIT_TEST_GROUP" } else { "UNIT_TEST_TEST" };
                let elemento = self.elemento(especie, &format!("{chamada}(\"{descricao}\")"), Some(nome.span), None, None, None);
                let valor = json!({"element": elemento, "range": self.faixa(e.span), "codeRange": self.faixa(e.span)});
                itens.push(Item { span: e.span, valor, folha: chamada == "test" });
            }
        }
        // Pré-ordem; as funções locais já trazem os próprios filhos, então
        // aqui só os grupos de teste aninham.
        itens.sort_by(|x, y| x.span.start.cmp(&y.span.start).then(y.span.end.cmp(&x.span.end)));
        fn montar(itens: &mut std::iter::Peekable<std::vec::IntoIter<Item>>, limite: usize) -> Vec<Value> {
            let mut saida = Vec::new();
            while let Some(item) = itens.next_if(|i| i.span.start < limite) {
                let mut valor = item.valor;
                let e_grupo = valor.pointer("/element/kind").and_then(Value::as_str) == Some("UNIT_TEST_GROUP");
                if e_grupo {
                    let filhos = montar(itens, item.span.end);
                    if !filhos.is_empty() {
                        valor["children"] = json!(filhos);
                    }
                } else {
                    // Função local (filhos já calculados) ou teste (folha):
                    // o que está dentro não é irmão.
                    let _ = item.folha;
                    while itens.next_if(|i| i.span.start < item.span.end).is_some() {}
                }
                saida.push(valor);
            }
            saida
        }
        montar(&mut itens.into_iter().peekable(), dentro.end)
    }

    /// `_newFunctionOutline` e `_newMethodOutline`.
    fn funcao(&self, f: FunctionId, span: Span, metadata: &[Annotation], especie_comum: &str, de_metodo: bool) -> Option<Value> {
        let func = self.ast.function(f);
        let nome = func.name?;
        let especie = match func.kind {
            FunctionKind::Getter => "GETTER",
            FunctionKind::Setter => "SETTER",
            _ => especie_comum,
        };
        let parametros = self.lista_de_parametros(nome.span.end).map(|s| self.trecho(s));
        // Uma função sem lista (getter) tem `''`; um método, nulo.
        let parametros = if de_metodo { parametros } else { Some(parametros.unwrap_or("")) };
        let de_tipo = self.parametros_de_tipo(nome.span.end).map(|s| self.trecho(s));
        let elemento = self.elemento(especie, self.nomes.resolve(nome.sym), Some(nome.span), parametros, de_tipo, Some(self.tipo(func.return_type)));
        Some(self.no(elemento, span, metadata, self.do_corpo(span)))
    }

    /// `_newVariableOutline` de cada variável de uma lista.
    fn variaveis(&self, lista: &VariableList, declaracao: Span, metadata: &[Annotation], especie: &str, saida: &mut Vec<Value>) {
        let total = lista.variables.len();
        for (i, v) in lista.variables.iter().enumerate() {
            let fim_da_variavel = v.initializer.map_or(v.name.span.end, |e| self.ast.expr(e).span.end);
            // A primeira começa na declaração (com documentação e
            // anotações); a última termina com ela.
            let inicio = if i == 0 { self.com_documentacao(declaracao.start) } else { v.name.span.start };
            let fim = if i + 1 == total { declaracao.end } else { fim_da_variavel };
            let _ = metadata;
            let elemento = self.elemento(especie, self.nomes.resolve(v.name.sym), Some(v.name.span), None, None, Some(self.tipo(lista.ty)));
            saida.push(json!({
                "element": elemento,
                "range": self.faixa(Span { start: inicio, end: fim }),
                "codeRange": self.faixa(Span { start: v.name.span.start, end: fim_da_variavel }),
            }));
        }
    }

    /// `_outlinesForMembers`.
    fn membros(&self, ids: &[MemberId]) -> Vec<Value> {
        let mut saida = Vec::new();
        for id in ids {
            let m = self.ast.member(*id);
            match &m.kind {
                MemberKind::Constructor(k) => {
                    let classe = self.nomes.resolve(k.class_name.sym);
                    let (nome, local) = match k.name {
                        Some(n) => (format!("{classe}.{}", self.nomes.resolve(n.sym)), n.span),
                        None => (classe.to_string(), k.class_name.span),
                    };
                    let parametros = self.lista_de_parametros(local.end).map(|s| self.trecho(s)).unwrap_or("");
                    let elemento = self.elemento("CONSTRUCTOR", &nome, Some(local), Some(parametros), None, None);
                    saida.push(self.no(elemento, m.span, &m.metadata, self.do_corpo(m.span)));
                }
                MemberKind::Field(l) => self.variaveis(l, m.span, &m.metadata, "FIELD", &mut saida),
                MemberKind::Method(f) => saida.extend(self.funcao(*f, m.span, &m.metadata, "METHOD", true)),
            }
        }
        saida
    }
}

/// O `Outline` da unidade de texto `texto` (o nó `<unit>` com os filhos).
pub(crate) fn do_documento(texto: &str) -> Value {
    let mut nomes = Interner::new();
    let parsed = dartforge_frontend::parser::parse(texto, &mut nomes);
    let a = &parsed.ast;
    // A mesma regra de `crate::simbolos` para os nós de teste.
    let importa_testes = parsed.unit.directives.iter().any(|d| match &d.kind {
        DirectiveKind::Import { uri, .. } => dartforge_elements::load::string_lit_value(uri).is_some_and(|u| u.ends_with("test.dart")),
        _ => false,
    });
    let declara_homonimo = parsed.unit.declarations.iter().any(|d| match &a.decl(*d).kind {
        DeclKind::Function(f) => a.function(*f).name.is_some_and(|n| matches!(nomes.resolve(n.sym), "group" | "test")),
        _ => false,
    });
    let ctx = Ctx { texto, linhas: TabelaLinhas::construir(texto), nomes: &nomes, ast: a, testes: importa_testes && !declara_homonimo };
    let nome_de = |n: Name| nomes.resolve(n.sym);
    let mut filhos: Vec<Value> = Vec::new();
    for id in &parsed.unit.declarations {
        let decl = a.decl(*id);
        let de_tipo = |n: Name| ctx.parametros_de_tipo(n.span.end).map(|s| ctx.trecho(s));
        match &decl.kind {
            DeclKind::Class(x) => {
                let especie = if x.mixin_application { "CLASS_TYPE_ALIAS" } else { "CLASS" };
                let elemento = ctx.elemento(especie, nome_de(x.name), Some(x.name.span), None, de_tipo(x.name), None);
                let internos = if x.mixin_application { Vec::new() } else { ctx.membros(&x.members) };
                filhos.push(ctx.no(elemento, decl.span, &decl.metadata, internos));
            }
            DeclKind::Mixin(x) => {
                let elemento = ctx.elemento("MIXIN", nome_de(x.name), Some(x.name.span), None, de_tipo(x.name), None);
                filhos.push(ctx.no(elemento, decl.span, &decl.metadata, ctx.membros(&x.members)));
            }
            DeclKind::Enum(x) => {
                // Todas as constantes antes dos membros.
                let mut internos: Vec<Value> = x
                    .constants
                    .iter()
                    .map(|k| ctx.no(ctx.elemento("ENUM_CONSTANT", nome_de(k.name), Some(k.name.span), None, None, None), k.span, &k.metadata, Vec::new()))
                    .collect();
                internos.extend(ctx.membros(&x.members));
                let elemento = ctx.elemento("ENUM", nome_de(x.name), Some(x.name.span), None, None, None);
                filhos.push(ctx.no(elemento, decl.span, &decl.metadata, internos));
            }
            DeclKind::Extension(x) => {
                // Sem nome, a posição é a do tipo estendido.
                let (nome, local) = match x.name {
                    Some(n) => (nome_de(n), n.span),
                    None => ("", a.ty(x.on).span),
                };
                let elemento = ctx.elemento("EXTENSION", nome, Some(local), None, x.name.and_then(de_tipo), None);
                filhos.push(ctx.no(elemento, decl.span, &decl.metadata, ctx.membros(&x.members)));
            }
            DeclKind::ExtensionType(x) => {
                let elemento = ctx.elemento("EXTENSION_TYPE", nome_de(x.name), Some(x.name.span), None, de_tipo(x.name), None);
                filhos.push(ctx.no(elemento, decl.span, &decl.metadata, ctx.membros(&x.members)));
            }
            DeclKind::Variables(l) => ctx.variaveis(l, decl.span, &decl.metadata, "TOP_LEVEL_VARIABLE", &mut filhos),
            DeclKind::Function(f) => filhos.extend(ctx.funcao(*f, decl.span, &decl.metadata, "FUNCTION", false)),
            DeclKind::Typedef(x) => {
                let elemento = match &x.kind {
                    // `typedef R F(params);`
                    TypedefKind::Legacy { return_type, .. } => {
                        let parametros = ctx.lista_de_parametros(x.name.span.end).map(|s| ctx.trecho(s)).unwrap_or("");
                        ctx.elemento("FUNCTION_TYPE_ALIAS", nome_de(x.name), Some(x.name.span), Some(parametros), de_tipo(x.name), Some(ctx.tipo(*return_type)))
                    }
                    TypedefKind::Alias(t) => match &a.ty(*t).kind {
                        // `typedef F = R Function(params);`
                        TypeKind::Function { return_type, .. } => {
                            let escrito = ctx.trecho(a.ty(*t).span);
                            // A lista é o par de parênteses que fecha no
                            // último `)` do tipo.
                            let parametros = {
                                let b = escrito.as_bytes();
                                let mut achado: Option<(usize, usize)> = None;
                                if let Some(fecho) = escrito.rfind(')') {
                                    let mut nivel = 0usize;
                                    for i in (0..=fecho).rev() {
                                        if b[i] == b')' {
                                            nivel += 1;
                                        } else if b[i] == b'(' {
                                            nivel -= 1;
                                            if nivel == 0 {
                                                achado = Some((i, fecho + 1));
                                                break;
                                            }
                                        }
                                    }
                                }
                                achado.map_or("", |(x, y)| &escrito[x..y])
                            };
                            ctx.elemento("FUNCTION_TYPE_ALIAS", nome_de(x.name), Some(x.name.span), Some(parametros), de_tipo(x.name), Some(ctx.tipo(*return_type)))
                        }
                        _ => ctx.elemento("TYPE_ALIAS", nome_de(x.name), Some(x.name.span), None, de_tipo(x.name), None),
                    },
                };
                filhos.push(ctx.no(elemento, decl.span, &decl.metadata, Vec::new()));
            }
        }
    }
    // O nó da unidade: do primeiro ao último token.
    let inicio = parsed
        .unit
        .directives
        .first()
        .map(|d| d.span.start)
        .into_iter()
        .chain(parsed.unit.declarations.first().map(|d| a.decl(*d).span.start))
        .min()
        .unwrap_or(0);
    let fim = parsed
        .unit
        .directives
        .last()
        .map(|d| d.span.end)
        .into_iter()
        .chain(parsed.unit.declarations.last().map(|d| a.decl(*d).span.end))
        .max()
        .unwrap_or(0);
    let unidade = Span { start: inicio, end: fim.max(inicio) };
    let mut raiz = json!({
        "element": ctx.elemento("COMPILATION_UNIT", "<unit>", Some(unidade), None, None, None),
        "range": ctx.faixa(unidade),
        "codeRange": ctx.faixa(unidade),
    });
    if !filhos.is_empty() {
        raiz["children"] = json!(filhos);
    }
    raiz
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn classe_com_documentacao_e_anotacao() {
        let fonte = "/// Doc.\n@a\nclass A<T> {\n  int x = 1, y;\n  A.n(int p);\n  void m() {}\n}\nvoid f(int a) {}\n";
        let raiz = do_documento(fonte);
        assert_eq!(raiz["element"]["kind"], "COMPILATION_UNIT");
        let classe = &raiz["children"][0];
        assert_eq!(classe["element"]["name"], "A");
        assert_eq!(classe["element"]["typeParameters"], "<T>");
        // `range` começa no comentário; `codeRange`, na palavra `class`.
        assert_eq!(classe["range"]["start"]["line"], 0);
        assert_eq!(classe["codeRange"]["start"]["line"], 2);
        let membros = classe["children"].as_array().unwrap();
        assert_eq!(membros.len(), 4);
        assert_eq!(membros[0]["element"]["kind"], "FIELD");
        assert_eq!(membros[0]["element"]["returnType"], "int");
        assert_eq!(membros[2]["element"]["name"], "A.n");
        assert_eq!(membros[2]["element"]["parameters"], "(int p)");
        assert_eq!(membros[3]["element"]["kind"], "METHOD");
        let funcao = &raiz["children"][1];
        assert_eq!(funcao["element"]["parameters"], "(int a)");
        assert_eq!(funcao["element"]["returnType"], "void");
    }
}
