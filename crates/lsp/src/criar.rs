//! Correções para nomes que a inferência comum deixou sem resolução (os
//! `undefined_*` do Dart, que este servidor ainda não publica mas cujas
//! correções o editor pede no cursor), com os títulos e espécies dos
//! produtores do Dart 3.6.2 (`services/correction/dart/`):
//!
//! * `Change to 'x'` (`change_to.dart`): o nome existente mais próximo
//!   (distância de Levenshtein menor que 3) do mesmo tipo — método do
//!   receptor, getter/campo do receptor, função de topo, classe;
//! * `Create method 'm'` (`create_method.dart`) numa chamada com receptor de
//!   classe do projeto, ou sem receptor dentro de um membro de classe;
//! * `Create function 'f'` (`create_function.dart`) numa chamada sem
//!   receptor, depois da declaração de topo que a contém;
//! * `Create getter 'g'` e `Create field 'g'` (`create_getter.dart`,
//!   `create_field.dart`) numa leitura de propriedade de classe do projeto
//!   (ou sem receptor dentro de uma classe);
//! * `Create local variable 'x'` (`create_local_variable.dart`) numa leitura
//!   sem receptor dentro de um corpo de função;
//! * `Create class 'C'` e `Create mixin 'C'` (`create_class.dart`,
//!   `create_mixin.dart`) num nome com cara de tipo (inicial maiúscula):
//!   tipo escrito, chamada sem receptor (classe com o construtor dos
//!   argumentos) ou leitura.
//!
//! O tipo das declarações criadas vem do contexto, como o
//! `inferUndefinedExpressionType`: comando → `void`; argumento → o tipo do
//! parâmetro; inicializador de variável com tipo escrito → esse tipo;
//! senão nenhum. Os parâmetros criados seguem os argumentos
//! (`writeParametersMatchingArguments`): o tipo estático de cada argumento e
//! um nome sugerido pela expressão (o identificador, ou a inicial do tipo).

use crate::acoes::AcaoDeCodigo;
use crate::projeto::Projeto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, StmtKind};
use dartforge_types::{Resolved, Type};
use std::collections::BTreeSet;

/// Distância de Levenshtein limitada (`levenshtein.dart`).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut anterior: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut atual = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let custo = usize::from(ca != cb);
            atual[j + 1] = (anterior[j] + custo).min(anterior[j + 1] + 1).min(atual[j] + 1);
        }
        anterior = atual;
    }
    anterior[b.len()]
}

/// O nome mais próximo de `alvo` com distância menor que 3 (o primeiro em
/// caso de empate).
fn mais_proximo<'a>(alvo: &str, nomes: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut melhor: Option<(usize, &str)> = None;
    for n in nomes {
        if n == alvo {
            continue;
        }
        let d = levenshtein(n, alvo);
        if d < melhor.map_or(3, |(m, _)| m) {
            melhor = Some((d, n));
        }
    }
    melhor.map(|(_, n)| n.to_string())
}

fn acao(titulo: String, especie: &str, edicoes: Vec<(String, Span, String)>) -> AcaoDeCodigo {
    AcaoDeCodigo {
        titulo,
        especie: especie.into(),
        edicoes: edicoes.into_iter().map(|(uri, span, texto)| Edicao { uri, span, texto }).collect(),
        diagnostico: None,
        criar_arquivo: None,
    }
}

/// O contexto de um uso indefinido.
#[derive(Clone, Copy)]
enum Forma {
    /// `alvo.m(…)` ou `m(…)`.
    Chamada { chamada: ast::ExprId, receptor: Option<ast::ExprId> },
    /// `alvo.g` ou `g` lido.
    Leitura { receptor: Option<ast::ExprId> },
}

impl Projeto {
    /// As correções de criação e troca para os nomes indefinidos em
    /// `inicio..fim` de `unidade`.
    pub(crate) fn criar_indefinidos(&mut self, uri: &str, unidade: UnitId, inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
        let p = self.programa();
        let u = p.unit(unidade);
        let ast = &u.ast;
        let lib = u.library;
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let toca = |s: Span| s.start <= fim && inicio <= s.end;
        // (nome, span, forma)
        let mut usos: Vec<(String, Span, Forma)> = Vec::new();
        let alvo_de_chamada: std::collections::HashMap<u32, u32> = ast
            .exprs
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match &e.kind {
                ExprKind::Call { target, .. } => Some((target.0, i as u32)),
                _ => None,
            })
            .collect();
        let atribuidos: BTreeSet<u32> = ast
            .exprs
            .iter()
            .filter_map(|e| match &e.kind {
                ExprKind::Assign { target, .. } => Some(target.0),
                _ => None,
            })
            .collect();
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let (nome, receptor) = match &e.kind {
                ExprKind::Identifier(n) if p.lookup(lib, n.sym).is_none() => (*n, None),
                ExprKind::Property { target, name, .. } => (*name, Some(*target)),
                _ => continue,
            };
            if !toca(nome.span) || corpos.get_resolved(id).is_some() || atribuidos.contains(&(i as u32)) {
                continue;
            }
            // Receptor que é prefixo, `dynamic` ou sem tipo: não há onde criar.
            if let Some(r) = receptor {
                match corpos.get_resolved(r) {
                    Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))) => continue,
                    _ => {}
                }
                let Some(t) = corpos.get_type(r) else { continue };
                if matches!(self.consulta.tabela.get(t), Type::Dynamic) {
                    continue;
                }
            }
            let texto = u.source[nome.span.start..nome.span.end].to_string();
            let forma = match alvo_de_chamada.get(&(i as u32)) {
                Some(c) => Forma::Chamada { chamada: ast::ExprId(*c), receptor },
                None => Forma::Leitura { receptor },
            };
            usos.push((texto, nome.span, forma));
        }
        // Nomes de tipo escritos que não resolvem.
        let mut tipos: Vec<(String, Span)> = Vec::new();
        for t in &ast.types {
            if let ast::TypeKind::Named { name, .. } = &t.kind
                && let [n] = &name[..]
                && toca(n.span)
                && p.lookup(lib, n.sym).is_none()
                && crate::projeto::declaracao_de_parametro_de_tipo(ast, n.span.start, n.sym).is_none()
            {
                let texto = u.source[n.span.start..n.span.end].to_string();
                if !["dynamic", "Never", "void", "Function", "Record"].contains(&texto.as_str()) {
                    tipos.push((texto, n.span));
                }
            }
        }
        let mut saida = Vec::new();
        for (nome, span, forma) in usos {
            saida.extend(self.para_uso(uri, unidade, &nome, span, forma));
        }
        for (nome, span) in tipos {
            if let Some(proximo) = mais_proximo(&nome, self.nomes_de_tipos(unidade).iter().map(String::as_str)) {
                saida.push(acao(format!("Change to '{proximo}'"), "quickfix.change.to", vec![(uri.to_string(), span, proximo)]));
            }
            if eh_nome_de_tipo(&nome)
                && let Some((pos, _)) = self.depois_do_topo(unidade, span.start)
            {
                saida.push(acao(format!("Create class '{nome}'"), "quickfix.create.class", vec![(uri.to_string(), Span { start: pos, end: pos }, format!("\n\nclass {nome} {{\n}}"))]));
                saida.push(acao(format!("Create mixin '{nome}'"), "quickfix.create.mixin", vec![(uri.to_string(), Span { start: pos, end: pos }, format!("\n\nmixin {nome} {{\n}}"))]));
            }
        }
        saida
    }

    /// As correções para um uso indefinido.
    fn para_uso(&mut self, uri: &str, unidade: UnitId, nome: &str, span: Span, forma: Forma) -> Vec<AcaoDeCodigo> {
        let mut saida = Vec::new();
        let receptor = match forma {
            Forma::Chamada { receptor, .. } | Forma::Leitura { receptor } => receptor,
        };
        // A classe onde criar: a do tipo do receptor (do projeto), ou a que
        // contém o uso.
        let (classe, estatico) = match receptor {
            Some(r) => {
                let corpos = &self.consulta.corpos.units[unidade.0 as usize];
                match corpos.get_resolved(r) {
                    Some(Resolved::Element(Element::Class(c))) => (Some(*c), true),
                    _ => (
                        corpos.get_type(r).and_then(|t| match self.consulta.tabela.get(t) {
                            Type::Interface { class, .. } => Some(*class),
                            _ => None,
                        }),
                        false,
                    ),
                }
            }
            None => (self.classe_que_contem(unidade, span.start), self.em_contexto_estatico(unidade, span.start)),
        };
        // Troca pelo nome mais próximo.
        let proximo = match (forma, receptor) {
            (Forma::Chamada { .. }, _) => classe.and_then(|c| {
                let metodos = self.membros_da_hierarquia(c, |k| matches!(k, FunctionKind::Function));
                mais_proximo(nome, metodos.iter().map(String::as_str))
            }),
            (Forma::Leitura { .. }, Some(_)) => classe.and_then(|c| {
                let props = self.membros_da_hierarquia(c, |k| matches!(k, FunctionKind::Getter | FunctionKind::ImplicitAccessor));
                mais_proximo(nome, props.iter().map(String::as_str))
            }),
            (Forma::Leitura { .. }, None) => None,
        };
        let proximo = proximo.or_else(|| match (forma, receptor) {
            (Forma::Chamada { .. }, None) => mais_proximo(nome, self.nomes_de_funcoes(unidade).iter().map(String::as_str)),
            _ => None,
        });
        if let Some(n) = proximo {
            saida.push(acao(format!("Change to '{n}'"), "quickfix.change.to", vec![(uri.to_string(), span, n)]));
        }
        let destino = classe.filter(|c| self.do_projeto(self.programa().class(*c).library) && self.programa().class(*c).decl.is_some());
        // Extensão sobre o tipo do receptor (`create_extension_member.dart`),
        // no fim do arquivo.
        if let Some(r) = receptor
            && !estatico
            && let Some(t) = self.consulta.corpos.units[unidade.0 as usize].get_type(r)
            && matches!(self.consulta.tabela.get(t), Type::Interface { .. })
        {
            let sobre = self.consulta.formatar(t).trim_end_matches('?').to_string();
            let fim_do_arquivo = self.programa().unit(unidade).source.len();
            let (titulo, especie, membro) = match forma {
                Forma::Chamada { chamada, .. } => {
                    let parametros = self.parametros_dos_argumentos(unidade, chamada);
                    let retorno = self.tipo_esperado(unidade, chamada).map_or(String::new(), |t| format!("{t} "));
                    (format!("Create extension method '{nome}'"), "quickfix.create.extension.method", format!("{retorno}{nome}({parametros}) {{}}"))
                }
                Forma::Leitura { .. } => {
                    let tipo = self.tipo_esperado_da_leitura(unidade, span).map_or(String::new(), |t| format!("{t} "));
                    (format!("Create extension getter '{nome}'"), "quickfix.create.extension.getter", format!("{tipo}get {nome} => null;"))
                }
            };
            saida.push(acao(titulo, especie, vec![(uri.to_string(), Span { start: fim_do_arquivo, end: fim_do_arquivo }, format!("\nextension on {sobre} {{\n  {membro}\n}}\n"))]));
        }
        match forma {
            Forma::Chamada { chamada, .. } => {
                let parametros = self.parametros_dos_argumentos(unidade, chamada);
                let retorno = self.tipo_esperado(unidade, chamada);
                if let Some(c) = destino
                    && (receptor.is_some() || self.classe_que_contem(unidade, span.start).is_some())
                    && let Some((u_c, pos, prefixo)) = self.depois_dos_membros(c)
                {
                    let estatico = if estatico { "static " } else { "" };
                    let tipo = retorno.as_ref().map_or(String::new(), |t| format!("{t} "));
                    let texto = format!("{prefixo}{estatico}{tipo}{nome}({parametros}) {{}}");
                    if let Some(alvo) = self.uri_da_unidade(u_c) {
                        saida.push(acao(format!("Create method '{nome}'"), "quickfix.create.method", vec![(alvo, Span { start: pos, end: pos }, texto)]));
                    }
                }
                if receptor.is_none() {
                    if let Some((pos, _)) = self.depois_do_topo(unidade, span.start) {
                        let tipo = retorno.as_ref().map_or(String::new(), |t| format!("{t} "));
                        saida.push(acao(
                            format!("Create function '{nome}'"),
                            "quickfix.create.function",
                            vec![(uri.to_string(), Span { start: pos, end: pos }, format!("\n\n{tipo}{nome}({parametros}) {{\n}}"))],
                        ));
                        if eh_nome_de_tipo(nome) {
                            let construtor = if parametros.is_empty() { String::new() } else { format!("\n  {nome}({parametros});\n") };
                            saida.push(acao(
                                format!("Create class '{nome}'"),
                                "quickfix.create.class",
                                vec![(uri.to_string(), Span { start: pos, end: pos }, format!("\n\nclass {nome} {{{construtor}\n}}"))],
                            ));
                        }
                    }
                }
            }
            Forma::Leitura { receptor } => {
                let tipo = self.tipo_esperado_da_leitura(unidade, span);
                if let Some(c) = destino
                    && let Some((u_c, pos, prefixo)) = self.depois_dos_membros(c)
                    && let Some(alvo) = self.uri_da_unidade(u_c)
                {
                    let estatico = if estatico { "static " } else { "" };
                    let campo = match &tipo {
                        Some(t) => format!("{prefixo}{estatico}{t} {nome};"),
                        None => format!("{prefixo}{estatico}var {nome};"),
                    };
                    let getter = match &tipo {
                        Some(t) => format!("{prefixo}{estatico}{t} get {nome} => null;"),
                        None => format!("{prefixo}{estatico}get {nome} => null;"),
                    };
                    saida.push(acao(format!("Create field '{nome}'"), "quickfix.create.field", vec![(alvo.clone(), Span { start: pos, end: pos }, campo)]));
                    saida.push(acao(format!("Create getter '{nome}'"), "quickfix.create.getter", vec![(alvo, Span { start: pos, end: pos }, getter)]));
                }
                if receptor.is_none()
                    && let Some((pos, indentacao)) = self.antes_do_comando(unidade, span.start)
                {
                    let decl = match &tipo {
                        Some(t) => format!("{t} {nome};\n{indentacao}"),
                        None => format!("var {nome};\n{indentacao}"),
                    };
                    saida.push(acao(format!("Create local variable '{nome}'"), "quickfix.create.localVariable", vec![(uri.to_string(), Span { start: pos, end: pos }, decl)]));
                }
                if receptor.is_none()
                    && eh_nome_de_tipo(nome)
                    && let Some((pos, _)) = self.depois_do_topo(unidade, span.start)
                {
                    saida.push(acao(format!("Create class '{nome}'"), "quickfix.create.class", vec![(uri.to_string(), Span { start: pos, end: pos }, format!("\n\nclass {nome} {{\n}}"))]));
                    saida.push(acao(format!("Create mixin '{nome}'"), "quickfix.create.mixin", vec![(uri.to_string(), Span { start: pos, end: pos }, format!("\n\nmixin {nome} {{\n}}"))]));
                }
            }
        }
        saida
    }

    /// Nomes de membros (de instância e estáticos) de `c` e dos supertipos
    /// cujo elemento passa em `filtro`.
    fn membros_da_hierarquia(&self, c: ClassId, filtro: impl Fn(FunctionKind) -> bool) -> Vec<String> {
        let p = self.programa();
        let mut classes: Vec<ClassId> = self.supertipos(c).into_iter().collect();
        classes.push(c);
        classes.sort();
        let mut nomes = BTreeSet::new();
        for x in classes {
            let cl = p.class(x);
            for (s, f) in cl.instance_members.iter().chain(cl.static_members.iter()) {
                let n = self.nome(*s);
                if !n.ends_with('=') && filtro(p.function(*f).kind) {
                    nomes.insert(n.to_string());
                }
            }
        }
        nomes.into_iter().collect()
    }

    /// Funções de topo visíveis na biblioteca da unidade.
    fn nomes_de_funcoes(&self, unidade: UnitId) -> Vec<String> {
        self.nomes_visiveis(unidade, |el| matches!(el, Element::Function(_)))
    }

    /// Classes visíveis na biblioteca da unidade.
    fn nomes_de_tipos(&self, unidade: UnitId) -> Vec<String> {
        self.nomes_visiveis(unidade, |el| matches!(el, Element::Class(_)))
    }

    fn nomes_visiveis(&self, unidade: UnitId, filtro: impl Fn(Element) -> bool) -> Vec<String> {
        let p = self.programa();
        let lib = p.library(p.unit(unidade).library);
        let mut nomes = BTreeSet::new();
        for (s, b) in lib.scope.iter().chain(lib.declared.iter()) {
            if let Some(el) = b.getter
                && filtro(el)
            {
                nomes.insert(self.nome(*s).to_string());
            }
        }
        nomes.into_iter().collect()
    }

    /// A classe declarada nesta unidade que contém `offset`.
    fn classe_que_contem(&self, unidade: UnitId, offset: usize) -> Option<ClassId> {
        let p = self.programa();
        let ast = &p.unit(unidade).ast;
        let (i, _) = ast
            .decls
            .iter()
            .enumerate()
            .filter(|(_, d)| d.span.start <= offset && offset < d.span.end && matches!(d.kind, DeclKind::Class(_) | DeclKind::Mixin(_) | DeclKind::Enum(_)))
            .min_by_key(|(_, d)| d.span.end - d.span.start)?;
        (0..p.classes.len()).map(|x| ClassId(x as u32)).find(|c| p.class(*c).decl.is_some_and(|d| d.unit == unidade && d.decl.0 as usize == i))
    }

    /// O uso está num membro estático (ou fora de membro de instância).
    fn em_contexto_estatico(&self, unidade: UnitId, offset: usize) -> bool {
        let ast = &self.programa().unit(unidade).ast;
        ast.members.iter().any(|m| {
            m.span.start <= offset
                && offset < m.span.end
                && match &m.kind {
                    ast::MemberKind::Method(f) => ast.function(*f).static_,
                    ast::MemberKind::Field(l) => l.static_,
                    ast::MemberKind::Constructor(k) => k.factory,
                }
        })
    }

    /// Onde inserir um membro novo em `c`: unidade, offset e o prefixo
    /// (`\n\n  ` depois do último membro, `\n  ` logo depois do `{`).
    fn depois_dos_membros(&self, c: ClassId) -> Option<(UnitId, usize, &'static str)> {
        let p = self.programa();
        let d = p.class(c).decl?;
        let u = p.unit(d.unit);
        let decl = u.ast.decl(d.decl);
        let membros: &[ast::MemberId] = match &decl.kind {
            DeclKind::Class(k) => &k.members,
            DeclKind::Mixin(k) => &k.members,
            DeclKind::Enum(k) => &k.members,
            DeclKind::ExtensionType(k) => &k.members,
            DeclKind::Extension(k) => &k.members,
            _ => return None,
        };
        if let Some(f) = membros.iter().map(|m| u.ast.member(*m).span.end).max() {
            return Some((d.unit, f, "\n\n  "));
        }
        let abre = u.source[decl.span.start..decl.span.end].find('{')? + decl.span.start + 1;
        Some((d.unit, abre, "\n  "))
    }

    /// O fim da declaração de topo que contém `offset`, e o texto dela.
    fn depois_do_topo(&self, unidade: UnitId, offset: usize) -> Option<(usize, ())> {
        let u = self.programa().unit(unidade);
        u.unit
            .declarations
            .iter()
            .map(|d| u.ast.decl(*d).span)
            .find(|s| s.start <= offset && offset <= s.end)
            .map(|s| (s.end, ()))
    }

    /// O início do comando que contém `offset` (num corpo de função) e a
    /// indentação dele.
    fn antes_do_comando(&self, unidade: UnitId, offset: usize) -> Option<(usize, String)> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let blocos: Vec<&[ast::StmtId]> = ast
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Block(cmds) if s.span.start <= offset && offset < s.span.end => Some(&cmds[..]),
                _ => None,
            })
            .collect();
        let cmd = blocos
            .iter()
            .flat_map(|b| b.iter())
            .map(|c| ast.stmt(*c).span)
            .filter(|s| s.start <= offset && offset <= s.end)
            .min_by_key(|s| s.end - s.start)?;
        let linha = u.source[..cmd.start].rfind('\n').map_or(0, |i| i + 1);
        let indentacao: String = u.source[linha..cmd.start].chars().take_while(|c| c.is_whitespace()).collect();
        Some((cmd.start, indentacao))
    }

    /// `int i, String s` dos argumentos da chamada.
    fn parametros_dos_argumentos(&self, unidade: UnitId, chamada: ast::ExprId) -> String {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let ExprKind::Call { arguments, .. } = &ast.expr(chamada).kind else { return String::new() };
        let mut usados: BTreeSet<String> = BTreeSet::new();
        let mut posicionais = Vec::new();
        let mut nomeados = Vec::new();
        for a in arguments.args.iter() {
            let tipo = corpos.get_type(a.value).map_or_else(|| "dynamic".to_string(), |t| self.consulta.formatar(t));
            let tipo = if tipo == "Null" { "dynamic".to_string() } else { tipo };
            match a.name {
                Some(n) => nomeados.push(format!("{tipo} {}", &u.source[n.span.start..n.span.end])),
                None => {
                    let base = match &ast.expr(a.value).kind {
                        ExprKind::Identifier(n) => u.source[n.span.start..n.span.end].to_string(),
                        ExprKind::Property { name, .. } => u.source[name.span.start..name.span.end].to_string(),
                        _ => tipo.chars().next().map_or("p".to_string(), |c| c.to_lowercase().to_string()),
                    };
                    let mut nome = base.clone();
                    let mut k = 2;
                    while !usados.insert(nome.clone()) {
                        nome = format!("{base}{k}");
                        k += 1;
                    }
                    posicionais.push(format!("{tipo} {nome}"));
                }
            }
        }
        if !nomeados.is_empty() {
            posicionais.push(format!("{{{}}}", nomeados.join(", ")));
        }
        posicionais.join(", ")
    }

    /// O tipo que o contexto espera de uma chamada: comando → `void`;
    /// argumento → tipo do parâmetro; inicializador com tipo escrito → ele.
    fn tipo_esperado(&self, unidade: UnitId, expr: ast::ExprId) -> Option<String> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let span = ast.expr(expr).span;
        if ast.stmts.iter().any(|s| matches!(s.kind, StmtKind::Expression(e) if e == expr)) {
            return Some("void".into());
        }
        self.tipo_do_contexto(unidade, span)
    }

    /// O tipo que o contexto espera de uma leitura em `span`.
    fn tipo_esperado_da_leitura(&self, unidade: UnitId, span: Span) -> Option<String> {
        let ast = &self.programa().unit(unidade).ast;
        let expr = ast.exprs.iter().filter(|e| e.span.start <= span.start && span.end <= e.span.end).min_by_key(|e| e.span.end - e.span.start)?;
        self.tipo_do_contexto(unidade, expr.span)
    }

    fn tipo_do_contexto(&self, unidade: UnitId, span: Span) -> Option<String> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        // Inicializador de variável com tipo escrito.
        for s in &ast.stmts {
            if let StmtKind::Variables(l) = &s.kind
                && let Some(t) = l.ty
                && l.variables.iter().any(|v| v.initializer.is_some_and(|i| ast.expr(i).span == span))
            {
                let ts = ast.ty(t).span;
                return Some(u.source[ts.start..ts.end].to_string());
            }
        }
        // Argumento posicional de chamada resolvida: o tipo do parâmetro.
        for (i, e) in ast.exprs.iter().enumerate() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let Some(k) = arguments.args.iter().filter(|a| a.name.is_none()).position(|a| ast.expr(a.value).span == span) else { continue };
            let _ = i;
            let tipo = corpos.get_type(*target);
            if let Some(t) = tipo
                && let Type::Function { positional, optional, .. } = self.consulta.tabela.get(t)
                && let Some(p) = positional.iter().chain(optional.iter()).nth(k)
            {
                return Some(self.consulta.formatar(*p));
            }
        }
        None
    }
}

/// Nome com cara de tipo (inicial maiúscula, como o `nameOfType`).
fn eh_nome_de_tipo(nome: &str) -> bool {
    nome.trim_start_matches(['_', '$']).chars().next().is_some_and(char::is_uppercase)
}

#[cfg(test)]
mod testes {
    use super::{levenshtein, mais_proximo};

    #[test]
    fn distancia_e_mais_proximo() {
        assert_eq!(levenshtein("raio", "rato"), 1);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(mais_proximo("mai", ["mais", "menos", "mai"].into_iter()), Some("mais".into()));
        assert_eq!(mais_proximo("xyz", ["abcdef"].into_iter()), None);
    }
}
