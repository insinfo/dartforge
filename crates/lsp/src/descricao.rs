//! Descrição de um elemento para o hover, no formato do servidor do Dart
//! (`ElementDisplayStringBuilder` do analyzer): `int soma(int a, [int b =
//! 0])`, `String get nome`, `abstract class B<T> extends A implements C`,
//! mais o tipo estático (variáveis e getters) e a documentação.
//!
//! Tipos vêm da tabela da inferência comum (`TypeTable::format`); nomes e
//! valores padrão, da árvore da declaração. Nada é inventado: um elemento sem
//! forma conhecida não tem hover.

use crate::Hover;
use crate::dartdoc;
use crate::projeto::{Alvo, Concreto, Denotado, Projeto, nome_base, palavra};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, UnitId,
};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_types::{Type, TypeId};

impl Projeto {
    /// O hover do que `d` denota na unidade `unidade`.
    pub(crate) fn hover(&self, unidade: UnitId, d: &Denotado) -> Option<Hover> {
        let (descricao, tipo, inicio_doc) = match (&d.alvo, d.concreto) {
            (
                Alvo::Local {
                    unidade: u,
                    declaracao,
                },
                _,
            ) => {
                let (descricao, tipo) = self.descrever_local(*u, *declaracao, d.expr, unidade)?;
                (descricao, tipo, None)
            }
            (
                Alvo::ParametroDeTipo {
                    unidade: u,
                    declaracao,
                },
                _,
            ) => (
                self.descrever_parametro_de_tipo(*u, *declaracao)?,
                None,
                None,
            ),
            (Alvo::Prefixo { .. }, _) => return None,
            (Alvo::Topo(Element::Class(_)), Some(Concreto::Funcao(f)))
                if matches!(
                    self.programa().function(f).kind,
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor
                ) =>
            {
                (
                    self.descrever_funcao(f),
                    None,
                    self.consulta.inicio_da_funcao(f),
                )
            }
            (_, Some(Concreto::Funcao(f))) => {
                let fe = self.programa().function(f);
                let tipo = matches!(fe.kind, FunctionKind::Getter)
                    .then(|| {
                        self.tipo_da_referencia(unidade, d.expr)
                            .unwrap_or(self.consulta.outline.functions[f.0 as usize].return_type)
                    })
                    .map(|t| self.consulta.formatar(t));
                (
                    self.descrever_funcao(f),
                    tipo,
                    self.consulta.inicio_da_funcao(f),
                )
            }
            (_, Some(Concreto::Variavel(v))) => {
                let declarado = self.consulta.tipo_da_variavel(v);
                let tipo = self.tipo_da_referencia(unidade, d.expr).or(declarado)?;
                let texto_tipo = self.consulta.formatar(tipo);
                let nome = self.nome(self.programa().variable(v).name);
                let declarado =
                    declarado.map_or_else(|| texto_tipo.clone(), |t| self.consulta.formatar(t));
                (
                    format!("{declarado} {nome}"),
                    Some(texto_tipo),
                    self.consulta.inicio_da_variavel(v),
                )
            }
            (Alvo::Topo(el), None) => (
                self.descrever_tipo(*el)?,
                None,
                self.consulta.inicio_do_elemento(*el),
            ),
            (Alvo::Construtor(f), None) => (
                self.descrever_funcao(*f),
                None,
                self.consulta.inicio_da_funcao(*f),
            ),
            (Alvo::Membro { .. }, None) => return None,
        };
        let documentacao = inicio_doc
            .and_then(|(u, inicio)| dartdoc::documentacao(&self.programa().unit(u).source, inicio))
            .or_else(|| self.documentacao_herdada(d));
        Some(Hover {
            intervalo: d.nome,
            descricao,
            tipo,
            documentacao,
        })
    }

    /// O tipo estático da expressão (com promoções), quando o cursor está
    /// num uso.
    fn tipo_da_referencia(&self, unidade: UnitId, expr: Option<ast::ExprId>) -> Option<TypeId> {
        self.consulta
            .corpos
            .units
            .get(unidade.0 as usize)?
            .get_type(expr?)
    }

    /// Local, parâmetro ou função local.
    fn descrever_local(
        &self,
        u: UnitId,
        declaracao: usize,
        expr: Option<ast::ExprId>,
        unidade_expr: UnitId,
    ) -> Option<(String, Option<String>)> {
        let unidade = self.programa().unit(u);
        let nome_span = palavra(&unidade.source, declaracao)?;
        let nome = &unidade.source[nome_span.start..nome_span.end];
        let corpos = &self.consulta.corpos.units[u.0 as usize];
        let declarado = corpos.tipo_local(declaracao)?;
        // Função local: a assinatura escrita.
        if let Some(fid) = unidade
            .ast
            .functions
            .iter()
            .position(|f| f.name.is_some_and(|n| n.span.start == declaracao))
        {
            let f = unidade.ast.function(ast::FunctionId(fid as u32));
            return Some((self.assinatura_local(u, f, nome, declarado), None));
        }
        let atual = self
            .tipo_da_referencia(unidade_expr, expr)
            .unwrap_or(declarado);
        Some((
            format!("{} {nome}", self.consulta.formatar(declarado)),
            Some(self.consulta.formatar(atual)),
        ))
    }

    /// Assinatura de uma função local pelo tipo inferido e pela árvore.
    fn assinatura_local(&self, u: UnitId, f: &ast::Function, nome: &str, tipo: TypeId) -> String {
        let Type::Function {
            ret,
            positional,
            optional,
            named,
            ..
        } = self.consulta.tabela.get(tipo).clone()
        else {
            return format!("{} {nome}", self.consulta.formatar(tipo));
        };
        let parametros = f.parameters.as_deref().unwrap_or(&[]);
        // Posicionais pela ordem; nomeados pelo nome (o tipo os guarda
        // na ordem canônica, não na escrita).
        let mut posicionais = positional.iter().chain(optional.iter());
        let tipos: Vec<Option<TypeId>> = parametros
            .iter()
            .map(|p| match p.kind {
                ParameterKind::Named => {
                    let simbolo = p.public_name.or(p.name)?.sym;
                    named
                        .iter()
                        .find(|(n, _, _)| *n == simbolo)
                        .map(|(_, t, _)| *t)
                }
                _ => posicionais.next().copied(),
            })
            .collect();
        format!(
            "{} {nome}{}{}",
            self.consulta.formatar(ret),
            self.parametros_de_tipo_escritos(&f.type_params, &self.programa().unit(u).source),
            self.lista_de_parametros(Some(u), parametros, &tipos)
        )
    }

    /// `<T extends B, U>` como escrito na fonte (vazio sem parâmetros).
    fn parametros_de_tipo_escritos(&self, ps: &[ast::TypeParameter], fonte: &str) -> String {
        if ps.is_empty() {
            return String::new();
        }
        let partes: Vec<&str> = ps
            .iter()
            .map(|t| &fonte[t.span.start..t.span.end])
            .collect();
        format!("<{}>", partes.join(", "))
    }

    /// `(int a, [int b = 0], {required int c})`: `tipos` alinhados com os
    /// parâmetros escritos em `u`.
    fn lista_de_parametros(
        &self,
        u: Option<UnitId>,
        ps: &[ast::Parameter],
        tipos: &[Option<TypeId>],
    ) -> String {
        let (fonte, arvore) = match u {
            Some(u) => (
                self.programa().unit(u).source.as_str(),
                Some(&self.programa().unit(u).ast),
            ),
            None => ("", None),
        };
        let mut obrigatorios = Vec::new();
        let mut opcionais = Vec::new();
        let mut nomeados = Vec::new();
        for (i, p) in ps.iter().enumerate() {
            let tipo = tipos
                .get(i)
                .copied()
                .flatten()
                .map_or_else(|| "dynamic".to_string(), |t| self.consulta.formatar(t));
            let nome = p
                .public_name
                .or(p.name)
                .map_or("", |n| &fonte[n.span.start..n.span.end]);
            let padrao = match (p.default_value, arvore) {
                (Some(e), Some(a)) => {
                    let s = a.expr(e).span;
                    format!(" = {}", &fonte[s.start..s.end])
                }
                _ => String::new(),
            };
            let texto = if nome.is_empty() {
                tipo
            } else {
                format!("{tipo} {nome}{padrao}")
            };
            match p.kind {
                ParameterKind::Required => obrigatorios.push(texto),
                ParameterKind::Named => nomeados.push(if p.required {
                    format!("required {texto}")
                } else {
                    texto
                }),
                _ => opcionais.push(texto),
            }
        }
        let mut partes = obrigatorios;
        if !opcionais.is_empty() {
            partes.push(format!("[{}]", opcionais.join(", ")));
        }
        if !nomeados.is_empty() {
            partes.push(format!("{{{}}}", nomeados.join(", ")));
        }
        format!("({})", partes.join(", "))
    }

    /// Assinatura de uma função, método, getter, setter ou construtor.
    pub(crate) fn descrever_funcao(&self, f: FunctionElementId) -> String {
        let p = self.programa();
        let fe = p.function(f);
        let dados = &self.consulta.outline.functions[f.0 as usize];
        let nome = nome_base(self.nome(fe.name)).to_string();
        let retorno = self.consulta.formatar(dados.return_type);
        let (unidade, parametros, tipos_escritos): (
            Option<UnitId>,
            &[ast::Parameter],
            &[ast::TypeParameter],
        ) = match fe.node {
            FunctionRef::Function { unit, function } => {
                let func = p.unit(unit).ast.function(function);
                (
                    Some(unit),
                    func.parameters.as_deref().unwrap_or(&[]),
                    &func.type_params,
                )
            }
            FunctionRef::Constructor { unit, member } => {
                match &p.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => (Some(unit), &k.parameters, &[]),
                    _ => (None, &[], &[]),
                }
            }
            FunctionRef::None => (None, &[], &[]),
        };
        let fonte = unidade.map_or("", |u| p.unit(u).source.as_str());
        let tipos: Vec<Option<TypeId>> = dados.parameters.iter().map(|q| Some(q.ty)).collect();
        let lista = self.lista_de_parametros(unidade, parametros, &tipos);
        match fe.kind {
            FunctionKind::Getter => format!("{retorno} get {nome}"),
            FunctionKind::Setter => format!("set {nome}{lista}"),
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                let classe = fe
                    .class
                    .map_or(String::new(), |c| self.nome(p.class(c).name).to_string());
                let exibido = if nome.is_empty() {
                    classe.clone()
                } else {
                    format!("{classe}.{nome}")
                };
                format!("{retorno} {exibido}{lista}")
            }
            FunctionKind::ImplicitAccessor => {
                let tipo = fe
                    .variable
                    .and_then(|v| self.consulta.tipo_da_variavel(v))
                    .unwrap_or(dados.return_type);
                format!("{} {nome}", self.consulta.formatar(tipo))
            }
            _ => format!(
                "{retorno} {nome}{}{lista}",
                self.parametros_de_tipo_escritos(tipos_escritos, fonte)
            ),
        }
    }

    /// Classe, mixin, enum, extension type, extensão ou typedef.
    fn descrever_tipo(&self, el: Element) -> Option<String> {
        let p = self.programa();
        let fmt = |t: TypeId| self.consulta.formatar(t);
        match el {
            Element::Class(c) => {
                let cl = p.class(c);
                let dados = self.consulta.outline.classes.get(c.0 as usize)?;
                let d = cl.decl?;
                let fonte = &p.unit(d.unit).source;
                let decl = p.unit(d.unit).ast.decl(d.decl);
                let nome = self.nome(cl.name);
                let lista =
                    |ts: &[TypeId]| ts.iter().map(|t| fmt(*t)).collect::<Vec<_>>().join(", ");
                let tipos = |ps: &[ast::TypeParameter]| self.parametros_de_tipo_escritos(ps, fonte);
                let mut s = String::new();
                match (&decl.kind, cl.kind) {
                    (DeclKind::Class(k), _) => {
                        let m = k.modifiers;
                        if m.sealed {
                            s.push_str("sealed ");
                        } else if m.abstract_ {
                            s.push_str("abstract ");
                        }
                        if m.base {
                            s.push_str("base ");
                        } else if m.interface {
                            s.push_str("interface ");
                        } else if m.final_ {
                            s.push_str("final ");
                        }
                        if m.mixin {
                            s.push_str("mixin ");
                        }
                        s.push_str(&format!("class {nome}{}", tipos(&k.type_params)));
                        if let Some(sup) = dados.supertype
                            && !self.eh_object(sup)
                        {
                            s.push_str(&format!(" extends {}", fmt(sup)));
                        }
                        if !dados.mixins.is_empty() {
                            s.push_str(&format!(" with {}", lista(&dados.mixins)));
                        }
                        if !dados.interfaces.is_empty() {
                            s.push_str(&format!(" implements {}", lista(&dados.interfaces)));
                        }
                    }
                    (DeclKind::Mixin(k), _) => {
                        if k.base {
                            s.push_str("base ");
                        }
                        s.push_str(&format!("mixin {nome}{}", tipos(&k.type_params)));
                        let on: Vec<TypeId> = dados
                            .on
                            .iter()
                            .copied()
                            .filter(|t| !self.eh_object(*t))
                            .collect();
                        if !on.is_empty() {
                            s.push_str(&format!(" on {}", lista(&on)));
                        }
                        if !dados.interfaces.is_empty() {
                            s.push_str(&format!(" implements {}", lista(&dados.interfaces)));
                        }
                    }
                    (DeclKind::Enum(k), _) => {
                        s.push_str(&format!("enum {nome}{}", tipos(&k.type_params)));
                        if !dados.mixins.is_empty() {
                            s.push_str(&format!(" with {}", lista(&dados.mixins)));
                        }
                        if !dados.interfaces.is_empty() {
                            s.push_str(&format!(" implements {}", lista(&dados.interfaces)));
                        }
                    }
                    (DeclKind::ExtensionType(k), _) => {
                        let rep = &fonte[p.unit(d.unit).ast.ty(k.representation_type).span.start
                            ..k.representation_name.span.end];
                        s.push_str(&format!(
                            "extension type {nome}{}({rep})",
                            tipos(&k.type_params)
                        ));
                        if !dados.interfaces.is_empty() {
                            s.push_str(&format!(" implements {}", lista(&dados.interfaces)));
                        }
                    }
                    (_, ClassKind::MixinApplication) => return None,
                    _ => return None,
                }
                Some(s)
            }
            Element::Extension(x) => {
                let ext = p.extension(x);
                let dados = self.consulta.outline.extensions.get(x.0 as usize)?;
                let fonte = &p.unit(ext.decl.unit).source;
                let DeclKind::Extension(k) = &p.unit(ext.decl.unit).ast.decl(ext.decl.decl).kind
                else {
                    return None;
                };
                let nome = ext.name.map_or("", |n| self.nome(n));
                let separador = if nome.is_empty() { "" } else { " " };
                Some(format!(
                    "extension{separador}{nome}{} on {}",
                    self.parametros_de_tipo_escritos(&k.type_params, fonte),
                    fmt(dados.on)
                ))
            }
            Element::Typedef(t) => {
                let td = p.typedef(t);
                let dados = self.consulta.outline.typedefs.get(t.0 as usize)?;
                let fonte = &p.unit(td.decl.unit).source;
                let DeclKind::Typedef(k) = &p.unit(td.decl.unit).ast.decl(td.decl.decl).kind else {
                    return None;
                };
                Some(format!(
                    "typedef {}{} = {}",
                    self.nome(td.name),
                    self.parametros_de_tipo_escritos(&k.type_params, fonte),
                    fmt(dados.target_type)
                ))
            }
            _ => None,
        }
    }

    /// `t` é `Object` (não se escreve `extends Object`).
    fn eh_object(&self, t: TypeId) -> bool {
        matches!(self.consulta.tabela.get(t), Type::Interface { class, .. } if Some(*class) == self.consulta.core.object_class)
    }

    /// `T extends B` como declarado.
    fn descrever_parametro_de_tipo(&self, u: UnitId, declaracao: usize) -> Option<String> {
        let unidade = self.programa().unit(u);
        let ast = &unidade.ast;
        let achar = |ps: &[ast::TypeParameter]| {
            ps.iter()
                .find(|t| t.name.span.start == declaracao)
                .map(|t| t.span)
        };
        let span = ast
            .decls
            .iter()
            .find_map(|d| match &d.kind {
                DeclKind::Class(c) => achar(&c.type_params),
                DeclKind::Mixin(m) => achar(&m.type_params),
                DeclKind::Enum(e) => achar(&e.type_params),
                DeclKind::Extension(x) => achar(&x.type_params),
                DeclKind::ExtensionType(x) => achar(&x.type_params),
                DeclKind::Typedef(t) => achar(&t.type_params),
                _ => None,
            })
            .or_else(|| ast.functions.iter().find_map(|f| achar(&f.type_params)))
            .or_else(|| {
                ast.types.iter().find_map(|t| match &t.kind {
                    ast::TypeKind::Function { type_params, .. } => achar(type_params),
                    _ => None,
                })
            })?;
        Some(unidade.source[span.start..span.end].to_string())
    }

    /// Membro de instância sem documentação própria: a do primeiro membro
    /// sobrescrito que a tem (como o analyzer faz no hover).
    fn documentacao_herdada(&self, d: &Denotado) -> Option<String> {
        let Alvo::Membro {
            nome,
            estatico: false,
            ..
        } = &d.alvo
        else {
            return None;
        };
        let c = match d.concreto? {
            Concreto::Funcao(f) => self.programa().function(f).class?,
            Concreto::Variavel(v) => self.programa().variable(v).class?,
        };
        let mut supers: Vec<ClassId> = self.supertipos(c).into_iter().collect();
        supers.sort();
        supers.into_iter().find_map(|s| {
            self.declarados(s, nome, false).into_iter().find_map(|f| {
                let (u, inicio) = self.consulta.inicio_da_funcao(f)?;
                dartdoc::documentacao(&self.programa().unit(u).source, inicio)
            })
        })
    }
}
