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
        // `_targetNode`: o nome do tipo de `A.n(…)` (o `NamedType` de um
        // `ConstructorName` de criação) mostra a criação, o construtor.
        let ajustado;
        let d = match (&d.alvo, d.concreto, d.expr) {
            (Alvo::Topo(Element::Class(_)), None, Some(e)) => match self.construtor_pelo_tipo(unidade, e) {
                Some(f) => {
                    ajustado = Denotado { concreto: Some(Concreto::Funcao(f)), ..d.clone() };
                    &ajustado
                }
                None => d,
            },
            _ => d,
        };
        // Declaração de constante de enum: o analyzer não tem hover ali.
        if d.expr.is_none()
            && let Some(Concreto::Variavel(v)) = d.concreto
            && self.programa().variable(v).class.is_some_and(|c| self.programa().class(c).enum_constants.contains(&v))
            && self.nome_da_variavel(v).is_some_and(|(u, s)| u == unidade && s == d.nome)
        {
            return None;
        }
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
            // A declaração do parâmetro de tipo (`TypeParameter`) não é nó
            // com hover no analyzer.
            (Alvo::ParametroDeTipo { unidade: u, declaracao }, _) if *u == unidade && d.nome.start == *declaracao => return None,
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
                    self.descrever_funcao_com(f, self.substituicao_do_membro(unidade, d, f)),
                    None,
                    self.consulta.inicio_da_funcao(f),
                )
            }
            (_, Some(Concreto::Funcao(f))) => {
                let fe = self.programa().function(f);
                // Na declaração (sem expressão), o analyzer não mostra tipo.
                let tipo = (matches!(fe.kind, FunctionKind::Getter) && d.expr.is_some())
                    .then(|| {
                        self.tipo_da_referencia(unidade, d.expr)
                            .unwrap_or(self.consulta.outline.functions[f.0 as usize].return_type)
                    })
                    .map(|t| self.consulta.formatar(t));
                (
                    self.descrever_funcao_com(f, self.substituicao_do_membro(unidade, d, f)),
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
        // `this.x` num construtor: o elemento do hover é o parâmetro.
        let campo_formal = d.expr.is_none()
            && crate::projeto::parametro_em(&self.programa().unit(unidade).ast, d.nome.start).is_some_and(|(x, _)| x.this_);
        let documentacao = self.documentacao_computada(unidade, d, inicio_doc, campo_formal);
        // Sem expressão e fora da declaração (metadados, `show`/`hide`,
        // `[ref]`): o analyzer não tem tipo estático para mostrar.
        let tipo = if d.expr.is_none() && !matches!(d.alvo, Alvo::Local { .. }) && !self.declaracao(d).is_some_and(|(u, s)| u == unidade && s == d.nome) {
            None
        } else {
            tipo
        };
        // `this.x`: o `type` do `FieldFormalParameterElement` (um
        // `VariableElement`), como na declaração de um parâmetro.
        let tipo = if campo_formal { tipo.or_else(|| self.tipo_do_campo_formal(unidade, d.nome.start)) } else { tipo };
        // Chamada: o tipo da invocação (`staticInvokeType`), como o Dart.
        let tipo = tipo.or_else(|| match d.concreto {
            Some(Concreto::Funcao(f))
                if matches!(self.programa().function(f).kind, FunctionKind::Function | FunctionKind::Operator)
                    && d.expr.is_some_and(|e| self.eh_alvo_de_chamada(unidade, e)) =>
            {
                self.tipo_da_referencia(unidade, d.expr)
                    .filter(|t| !matches!(self.consulta.tabela.get(*t), Type::Dynamic))
                    .map(|t| self.consulta.formatar(t))
            }
            // Função local chamada.
            None if matches!(d.alvo, Alvo::Local { .. })
                && d.expr.is_some_and(|e| self.eh_alvo_de_chamada(unidade, e))
                && d.expr.and_then(|e| self.tipo_da_referencia(unidade, Some(e))).is_some_and(|t| matches!(self.consulta.tabela.get(t), Type::Function { .. })) =>
            {
                self.tipo_da_referencia(unidade, d.expr).map(|t| self.consulta.formatar(t))
            }
            _ => None,
        });
        // Criação sem `new`/`const`: `(new) ` antes da descrição.
        let mut descricao = descricao;
        if let Some(Concreto::Funcao(f)) = d.concreto
            && matches!(self.programa().function(f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor)
            && d.expr.is_some_and(|e| self.eh_alvo_de_chamada(unidade, e) || self.eh_alvo_de_chamada_por_propriedade(unidade, e))
        {
            descricao = format!("(new) {descricao}");
        }
        if self.depreciado(d) {
            descricao = format!("(deprecated) {descricao}");
        }
        Some(Hover {
            intervalo: d.nome,
            descricao,
            tipo,
            documentacao,
            biblioteca: if campo_formal { None } else { self.biblioteca_exibida(d) },
        })
    }

    /// A substituição do `Member` que o hover mostra: um método ou operador
    /// de instância lido por `alvo.m`, pelos argumentos do tipo do alvo; um
    /// construtor, pelos do tipo criado. Getters, setters e campos passam
    /// pelo `nonSynthetic` e ficam como declarados.
    fn substituicao_do_membro(&self, unidade: UnitId, d: &Denotado, f: FunctionElementId) -> Option<Substituicao> {
        let p = self.programa();
        let fe = p.function(f);
        let classe = fe.class?;
        let e = d.expr?;
        let ast = &p.unit(unidade).ast;
        let corpos = self.consulta.corpos.units.get(unidade.0 as usize)?;
        let tipo = match fe.kind {
            FunctionKind::Function | FunctionKind::Operator if !fe.static_ => match &ast.expr(e).kind {
                ast::ExprKind::Property { target, .. } => corpos.get_type(*target)?,
                _ => return None,
            },
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                // A criação: `C(…)`, `C.n(…)`, `p.C(…)` (a chamada cujo alvo
                // é o nome, ou a propriedade sobre ele).
                let chamada = ast.exprs.iter().enumerate().find_map(|(i, x)| match &x.kind {
                    ast::ExprKind::Call { target, .. } if *target == e => Some(i),
                    ast::ExprKind::Call { target, .. } => match &ast.expr(*target).kind {
                        ast::ExprKind::Property { target: t2, .. } if *t2 == e => Some(i),
                        ast::ExprKind::TypeArguments { target: t2, .. } if *t2 == e => Some(i),
                        _ => None,
                    },
                    _ => None,
                })?;
                corpos.get_type(ast::ExprId(chamada as u32))?
            }
            _ => return None,
        };
        let mut tabela = self.consulta.tabela.clone();
        let visto = self.consulta.outline.hierarchy.supertype_of(tipo, classe, &mut tabela, &self.consulta.core)?;
        let Type::Interface { args, .. } = tabela.get(visto).clone() else { return None };
        let params = &self.consulta.outline.classes.get(classe.0 as usize)?.type_params;
        if params.len() != args.len() || params.is_empty() {
            return None;
        }
        Some((params.iter().copied().zip(args.iter().copied()).collect(), tabela))
    }

    /// O construtor de `A.n(…)`, `A<T>.n(…)` ou `p.A.n(…)` quando `e` é o
    /// nome do tipo (`A`).
    fn construtor_pelo_tipo(&self, unidade: UnitId, e: ast::ExprId) -> Option<FunctionElementId> {
        let ast = &self.programa().unit(unidade).ast;
        let corpos = self.consulta.corpos.units.get(unidade.0 as usize)?;
        let pai = |x: ast::ExprId| {
            ast.exprs.iter().position(|y| match &y.kind {
                ast::ExprKind::Property { target, .. } | ast::ExprKind::TypeArguments { target, .. } => *target == x,
                _ => false,
            })
        };
        let mut atual = ast::ExprId(pai(e)? as u32);
        if matches!(ast.expr(atual).kind, ast::ExprKind::TypeArguments { .. }) {
            atual = ast::ExprId(pai(atual)? as u32);
        }
        if !matches!(ast.expr(atual).kind, ast::ExprKind::Property { .. }) {
            return None;
        }
        ast.exprs.iter().enumerate().find_map(|(i, y)| match &y.kind {
            ast::ExprKind::Call { target, .. } if *target == atual => match corpos.get_resolved(ast::ExprId(i as u32)) {
                Some(dartforge_types::Resolved::Constructor(f)) => Some(*f),
                _ => None,
            },
            _ => None,
        })
    }

    /// O tipo de `this.x` (o do parâmetro no outline do construtor).
    fn tipo_do_campo_formal(&self, unidade: UnitId, nome: usize) -> Option<String> {
        let p = self.programa();
        let ast = &p.unit(unidade).ast;
        let (_, crate::projeto::DonoParametro::Construtor(m)) = crate::projeto::parametro_em(ast, nome)? else { return None };
        let MemberKind::Constructor(k) = &ast.member(m).kind else { return None };
        let i = k.parameters.iter().position(|x| x.name.is_some_and(|n| n.span.start == nome))?;
        let f = self.construtor_do_no(unidade, m)?;
        let t = self.consulta.outline.functions.get(f.0 as usize)?.parameters.get(i)?.ty;
        Some(self.consulta.formatar(t))
    }

    /// `DartUnitHoverComputer.computeDocumentation`: o elemento (o campo de
    /// `this.x`; o executável dono de um parâmetro), ele e os membros que ele
    /// sobrescreve (`findOverriddenElements`: as superclasses e os mixins,
    /// depois as interfaces), o primeiro com comentário de documentação; um
    /// setter sem ela usa a do getter correspondente. Vinda de outra classe,
    /// a documentação ganha o `Copied from` dela. Acessor sintético de campo
    /// não tem comentário.
    fn documentacao_computada(&self, unidade: UnitId, d: &Denotado, inicio_doc: Option<(UnitId, usize)>, campo_formal: bool) -> Option<String> {
        let p = self.programa();
        let semente: Option<Documentavel> = if campo_formal {
            let (prm, dono) = crate::projeto::parametro_em(&p.unit(unidade).ast, d.nome.start)?;
            let crate::projeto::DonoParametro::Construtor(m) = dono else { return None };
            let classe = p.function(self.construtor_do_no(unidade, m)?).class?;
            let nome = prm.name?.sym;
            Some(Documentavel::Variavel(p.class(classe).fields.iter().copied().find(|&v| p.variable(v).name == nome)?))
        } else {
            match (&d.alvo, d.concreto) {
                (Alvo::Local { unidade: u, declaracao }, _) => {
                    let ast = &p.unit(*u).ast;
                    match crate::projeto::parametro_em(ast, *declaracao)?.1 {
                        crate::projeto::DonoParametro::Funcao(fid) => match self.funcao_do_no(*u, fid) {
                            Some(f) => Some(Documentavel::Funcao(f)),
                            // Função local: o comentário dela; closure não tem.
                            None => {
                                ast.function(fid).name?;
                                return dartdoc::documentacao(&p.unit(*u).source, ast.function(fid).span.start);
                            }
                        },
                        crate::projeto::DonoParametro::Construtor(m) => Some(Documentavel::Funcao(self.construtor_do_no(*u, m)?)),
                    }
                }
                (_, Some(Concreto::Funcao(f))) => match (p.function(f).kind, p.function(f).variable) {
                    (FunctionKind::ImplicitAccessor, Some(v)) => Some(Documentavel::Variavel(v)),
                    _ => Some(Documentavel::Funcao(f)),
                },
                (_, Some(Concreto::Variavel(v))) => Some(Documentavel::Variavel(v)),
                _ => None,
            }
        };
        let Some(semente) = semente else {
            return inicio_doc.and_then(|(u, inicio)| dartdoc::documentacao(&p.unit(u).source, inicio));
        };
        let dono = |x: Documentavel| match x {
            Documentavel::Funcao(f) => p.function(f).class,
            Documentavel::Variavel(v) => p.variable(v).class,
        };
        let comentario = |x: Documentavel| -> Option<String> {
            let (u, inicio) = match x {
                Documentavel::Funcao(f) if p.function(f).kind == FunctionKind::ImplicitAccessor => return None,
                Documentavel::Funcao(f) => self.consulta.inicio_da_funcao(f)?,
                Documentavel::Variavel(v) => self.consulta.inicio_da_variavel(v)?,
            };
            dartdoc::documentacao(&p.unit(u).source, inicio)
        };
        let mut candidatos = vec![semente];
        if let Some(classe) = dono(semente) {
            let biblioteca = match semente {
                Documentavel::Funcao(f) => p.function(f).library,
                Documentavel::Variavel(v) => p.variable(v).library,
            };
            let (nome, especies): (String, Vec<FunctionKind>) = match semente {
                Documentavel::Variavel(v) => {
                    let var = p.variable(v);
                    let mut e = vec![FunctionKind::Getter];
                    if !var.final_ && !var.const_ {
                        e.push(FunctionKind::Setter);
                    }
                    (self.nome(var.name).to_string(), e)
                }
                Documentavel::Funcao(f) => {
                    let fe = p.function(f);
                    let nome = self.nome(fe.name);
                    match fe.kind {
                        FunctionKind::Function | FunctionKind::Operator => (nome.to_string(), vec![FunctionKind::Function]),
                        FunctionKind::Getter => (nome.to_string(), vec![FunctionKind::Getter]),
                        FunctionKind::Setter => (nome.trim_end_matches("_=").trim_end_matches('=').to_string(), vec![FunctionKind::Setter]),
                        _ => (nome.to_string(), Vec::new()),
                    }
                }
            };
            if !especies.is_empty() {
                let mut supers = Vec::new();
                let mut vistos = std::collections::HashSet::new();
                self.supers_da_sobrescrita(classe, false, &mut vistos, &mut supers);
                let mut interfaces = Vec::new();
                vistos.clear();
                self.interfaces_da_sobrescrita(classe, false, &mut vistos, &mut interfaces);
                let mut achados_super: Vec<FunctionElementId> = Vec::new();
                for s in supers {
                    if let Some(m) = self.membro_da_sobrescrita(s, &nome, &especies, biblioteca)
                        && !achados_super.contains(&m)
                    {
                        achados_super.push(m);
                    }
                }
                let mut achados_interface: Vec<FunctionElementId> = Vec::new();
                for s in interfaces {
                    if let Some(m) = self.membro_da_sobrescrita(s, &nome, &especies, biblioteca)
                        && !achados_interface.contains(&m)
                        && !achados_super.contains(&m)
                    {
                        achados_interface.push(m);
                    }
                }
                candidatos.extend(achados_super.into_iter().chain(achados_interface).map(Documentavel::Funcao));
            }
        }
        let mut documentado: Option<(Documentavel, String)> = None;
        let mut do_getter: Option<(Documentavel, String)> = None;
        for &c in candidatos.iter() {
            if let Some(texto) = comentario(c) {
                documentado = Some((c, texto));
                break;
            }
            if do_getter.is_none()
                && let Documentavel::Funcao(f) = c
                && p.function(f).kind == FunctionKind::Setter
                && let Some(classe) = p.function(f).class
                && let Some(chave) = self.consulta.nomes.lookup(self.nome(p.function(f).name).trim_end_matches("_=").trim_end_matches('='))
                && let Some(&g) = p.class(classe).instance_members.get(&chave).or_else(|| p.class(classe).static_members.get(&chave))
                && let Some(texto) = comentario(Documentavel::Funcao(g))
            {
                do_getter = Some((Documentavel::Funcao(g), texto));
            }
        }
        let (elemento, mut texto) = documentado.or(do_getter)?;
        if let Some(c) = dono(elemento)
            && dono(elemento) != dono(semente)
        {
            texto = format!("{texto}\n\nCopied from `{}`.", self.nome(p.class(c).name));
        }
        Some(texto)
    }

    /// A superclasse declarada (pulando as aplicações de mixin sintéticas) e
    /// os mixins na ordem escrita.
    fn super_e_mixins(&self, c: ClassId) -> (Option<ClassId>, Vec<ClassId>) {
        let p = self.programa();
        let mut sintetico: Vec<ClassId> = Vec::new();
        let mut s = p.class(c).supertype_class;
        let mut passos = 0;
        while let Some(x) = s
            && p.class(x).decl.is_none()
            && p.class(x).kind == ClassKind::MixinApplication
            && passos < 64
        {
            sintetico.extend(p.class(x).mixin_classes.iter().rev());
            s = p.class(x).supertype_class;
            passos += 1;
        }
        sintetico.reverse();
        sintetico.extend(p.class(c).mixin_classes.iter().copied());
        (s, sintetico)
    }

    /// `_addSuperOverrides`.
    fn supers_da_sobrescrita(&self, c: ClassId, com_este: bool, vistos: &mut std::collections::HashSet<ClassId>, saida: &mut Vec<ClassId>) {
        if !vistos.insert(c) {
            return;
        }
        if com_este {
            saida.push(c);
        }
        let (s, mixins) = self.super_e_mixins(c);
        if let Some(s) = s {
            self.supers_da_sobrescrita(s, true, vistos, saida);
        }
        for m in mixins {
            self.supers_da_sobrescrita(m, true, vistos, saida);
        }
        if self.programa().class(c).kind == ClassKind::Mixin {
            for o in self.programa().class(c).on_classes.clone() {
                self.supers_da_sobrescrita(o, true, vistos, saida);
            }
        }
    }

    /// `_addInterfaceOverrides`.
    fn interfaces_da_sobrescrita(&self, c: ClassId, verificar: bool, vistos: &mut std::collections::HashSet<ClassId>, saida: &mut Vec<ClassId>) {
        if !vistos.insert(c) {
            return;
        }
        if verificar {
            saida.push(c);
        }
        for i in self.programa().class(c).interface_classes.clone() {
            self.interfaces_da_sobrescrita(i, true, vistos, saida);
        }
        if let (Some(s), _) = self.super_e_mixins(c) {
            self.interfaces_da_sobrescrita(s, verificar, vistos, saida);
        }
    }

    /// `_lookupMember`: o método, o getter ou o setter declarado em `c` com
    /// o nome (privado só na mesma biblioteca).
    fn membro_da_sobrescrita(&self, c: ClassId, nome: &str, especies: &[FunctionKind], biblioteca: dartforge_elements::model::LibraryId) -> Option<FunctionElementId> {
        let p = self.programa();
        let k = p.class(c);
        if nome.starts_with('_') && k.library != biblioteca {
            return None;
        }
        let achar = |chave: &str, aceita: &dyn Fn(FunctionKind) -> bool| -> Option<FunctionElementId> {
            let s = self.consulta.nomes.lookup(chave)?;
            [k.instance_members.get(&s), k.static_members.get(&s)].into_iter().flatten().copied().find(|&f| aceita(p.function(f).kind))
        };
        if especies.contains(&FunctionKind::Function)
            && let Some(f) = achar(nome, &|e| matches!(e, FunctionKind::Function | FunctionKind::Operator))
        {
            return Some(f);
        }
        if especies.contains(&FunctionKind::Getter)
            && let Some(f) = achar(nome, &|e| matches!(e, FunctionKind::Getter | FunctionKind::ImplicitAccessor))
        {
            return Some(f);
        }
        if especies.contains(&FunctionKind::Setter)
            && let Some(f) = achar(&format!("{nome}_="), &|e| matches!(e, FunctionKind::Setter | FunctionKind::ImplicitAccessor))
        {
            return Some(f);
        }
        None
    }

    /// `e` é o alvo de uma chamada (`e(…)`).
    fn eh_alvo_de_chamada(&self, unidade: UnitId, e: ast::ExprId) -> bool {
        self.programa().unit(unidade).ast.exprs.iter().any(|x| matches!(&x.kind, ast::ExprKind::Call { target, .. } if *target == e))
    }

    /// `e` é a classe de `A.nome(…)` (o alvo da propriedade chamada).
    fn eh_alvo_de_chamada_por_propriedade(&self, unidade: UnitId, e: ast::ExprId) -> bool {
        let ast = &self.programa().unit(unidade).ast;
        ast.exprs.iter().enumerate().any(|(i, x)| {
            matches!(&x.kind, ast::ExprKind::Property { target, .. } if *target == e)
                && self.eh_alvo_de_chamada(unidade, ast::ExprId(i as u32))
        })
    }

    /// A biblioteca do elemento não local, como o hover do Dart a mostra.
    fn biblioteca_exibida(&self, d: &Denotado) -> Option<String> {
        let p = self.programa();
        let lib = match (&d.alvo, d.concreto) {
            // A variável de um `for` de coleção não tem executável como
            // `enclosingElement3`: o analyzer mostra a biblioteca.
            (Alvo::Local { unidade, declaracao }, _) if em_for_de_colecao(&p.unit(*unidade).ast, *declaracao) => p.unit(*unidade).library,
            (Alvo::Local { .. }, _) | (Alvo::Prefixo { .. }, _) => return None,
            (Alvo::ParametroDeTipo { unidade, declaracao }, _) => {
                // Parâmetro de tipo de método ou função: local.
                let ast = &p.unit(*unidade).ast;
                if ast.functions.iter().any(|f| f.type_params.iter().any(|t| t.name.span.start == *declaracao)) {
                    return None;
                }
                p.unit(*unidade).library
            }
            (_, Some(Concreto::Funcao(f))) => p.function(f).library,
            (_, Some(Concreto::Variavel(v))) => p.variable(v).library,
            (Alvo::Topo(el), None) => self.biblioteca_do_elemento(*el),
            (Alvo::Construtor(f), None) => p.function(*f).library,
            (Alvo::Membro { .. }, None) => return None,
        };
        let uri = &p.library(lib).uri;
        if !uri.starts_with("file:") {
            return Some(uri.clone());
        }
        let caminho = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        let raiz = crate::projeto::raiz_do_projeto(&caminho);
        let relativo = dartforge_elements::config::sem_verbatim(caminho.clone());
        let relativo = relativo.strip_prefix(dartforge_elements::config::sem_verbatim(raiz.clone())).ok()?;
        let partes: Vec<String> = relativo.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        // Em `lib/` de um pacote, o analyzer vê a URI `package:`.
        if partes.first().is_some_and(|c| c == "lib")
            && let Some(pacote) = crate::indice::nome_do_pacote(&raiz)
        {
            return Some(format!("package:{pacote}/{}", partes[1..].join("/")));
        }
        Some(partes.join("/"))
    }

    /// A declaração do elemento tem `@deprecated` ou `@Deprecated(…)`.
    fn depreciado(&self, d: &Denotado) -> bool {
        let p = self.programa();
        let metadados: Option<(UnitId, &[ast::Annotation])> = match (&d.alvo, d.concreto) {
            (_, Some(Concreto::Funcao(f))) => match p.function(f).node {
                FunctionRef::Function { unit, function } => {
                    let ast = &p.unit(unit).ast;
                    ast.decls
                        .iter()
                        .find(|x| matches!(x.kind, DeclKind::Function(g) if g == function))
                        .map(|x| (unit, &x.metadata[..]))
                        .or_else(|| ast.members.iter().find(|m| matches!(m.kind, MemberKind::Method(g) if g == function)).map(|m| (unit, &m.metadata[..])))
                }
                FunctionRef::Constructor { unit, member } => Some((unit, &p.unit(unit).ast.member(member).metadata[..])),
                FunctionRef::None => None,
            },
            (Alvo::Topo(Element::Class(c)), None) => p.class(*c).decl.map(|dr| (dr.unit, &p.unit(dr.unit).ast.decl(dr.decl).metadata[..])),
            _ => None,
        };
        metadados.is_some_and(|(u, ms)| {
            let fonte = &p.unit(u).source;
            ms.iter().any(|a| a.name.last().is_some_and(|n| matches!(&fonte[n.span.start..n.span.end], "deprecated" | "Deprecated")))
        })
    }

    /// O tipo estático da expressão (com promoções), quando o cursor está
    /// num uso.
    fn tipo_da_referencia(&self, unidade: UnitId, expr: Option<ast::ExprId>) -> Option<TypeId> {
        let e = expr?;
        let corpos = self.consulta.corpos.units.get(unidade.0 as usize)?;
        // O alvo de uma atribuição (`x = …`, `x += …`, `o.x = …`): o
        // `writeType` dela (`_getTypeOfDeclarationOrReference`).
        let ast = &self.programa().unit(unidade).ast;
        if let Some(i) = ast.exprs.iter().position(|x| matches!(&x.kind, ast::ExprKind::Assign { target, .. } if *target == e))
            && let Some(&t) = corpos.tipos_de_escrita.get(&ast::ExprId(i as u32))
        {
            return Some(t);
        }
        corpos.get_type(e)
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
        // Parâmetro: com os delimitadores do tipo dele (`[int b = 0]`,
        // `{required int c}`), como o `writeFormalParameter` do analyzer.
        let mut texto = format!("{} {nome}", self.consulta.formatar(declarado));
        if let Some((p, _)) = crate::projeto::parametro_em(&unidade.ast, declaracao)
            && p.name.is_some_and(|n| n.span.start == declaracao)
        {
            let padrao = p.default_value.map_or(String::new(), |e| {
                let s = unidade.ast.expr(e).span;
                format!(" = {}", &unidade.source[s.start..s.end])
            });
            texto = match p.kind {
                ParameterKind::Required => texto,
                ParameterKind::Optional => format!("[{texto}{padrao}]"),
                ParameterKind::Named => format!("{{{}{texto}{padrao}}}", if p.required { "required " } else { "" }),
            };
        }
        Some((texto, Some(self.consulta.formatar(atual))))
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
        self.lista_de_parametros_com(u, ps, tipos, &|t| self.consulta.formatar(t))
    }

    /// [`Projeto::lista_de_parametros`] com os tipos formatados por `fmt`.
    fn lista_de_parametros_com(
        &self,
        u: Option<UnitId>,
        ps: &[ast::Parameter],
        tipos: &[Option<TypeId>],
        fmt: &dyn Fn(TypeId) -> String,
    ) -> String {
        let (fonte, arvore) = match u {
            Some(u) => (
                self.programa().unit(u).source.as_str(),
                Some(&self.programa().unit(u).ast),
            ),
            None => ("", None),
        };
        // Como o `_writeFormalParameters` do analyzer com `multiline`: com
        // três parâmetros ou mais, um por linha, com vírgula final.
        let multilinha = ps.len() >= 3;
        let mut itens: Vec<(ParameterKind, String)> = Vec::new();
        for (i, p) in ps.iter().enumerate() {
            let tipo = tipos
                .get(i)
                .copied()
                .flatten()
                .map_or_else(|| "dynamic".to_string(), fmt);
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
            let texto = if p.kind == ParameterKind::Named && p.required {
                format!("required {texto}")
            } else {
                texto
            };
            itens.push((p.kind, texto));
        }
        let (abre_grupo, separador, fim, prefixo) =
            if multilinha { (" ", ",", ",
", "
  ") } else { ("", ", ", "", "") };
        let mut s = String::from("(");
        let mut ultimo: Option<ParameterKind> = None;
        let mut fecha = "";
        for (i, (k, texto)) in itens.iter().enumerate() {
            if i != 0 {
                s.push_str(separador);
            }
            if ultimo != Some(*k) {
                s.push_str(fecha);
                if ultimo.is_some() {
                    s.push_str(abre_grupo);
                }
                let (abre, f) = match k {
                    ParameterKind::Required => ("", ""),
                    ParameterKind::Optional => ("[", "]"),
                    ParameterKind::Named => ("{", "}"),
                };
                s.push_str(abre);
                fecha = f;
                ultimo = Some(*k);
            }
            s.push_str(prefixo);
            s.push_str(texto);
        }
        if !itens.is_empty() {
            s.push_str(fim);
        }
        s.push_str(fecha);
        s.push(')');
        s
    }

    /// Assinatura de uma função, método, getter, setter ou construtor.
    pub(crate) fn descrever_funcao(&self, f: FunctionElementId) -> String {
        self.descrever_funcao_com(f, None)
    }

    /// [`Projeto::descrever_funcao`] do `Member` substituído (`mapa`: os
    /// parâmetros de tipo da classe pelos argumentos do receptor).
    fn descrever_funcao_com(&self, f: FunctionElementId, substituicao: Option<Substituicao>) -> String {
        let p = self.programa();
        let fe = p.function(f);
        let dados = &self.consulta.outline.functions[f.0 as usize];
        let nome = nome_base(self.nome(fe.name)).to_string();
        let (mapa, mut copia) = match substituicao {
            Some((m, tb)) => (Some(m), Some(tb)),
            None => (None, None),
        };
        let mut sub = |t: TypeId| -> TypeId {
            match (copia.as_mut(), mapa.as_ref()) {
                (Some(tb), Some(m)) => dartforge_types::ops::substitute(t, m, tb),
                _ => t,
            }
        };
        let retorno_t = sub(dados.return_type);
        let tipos: Vec<Option<TypeId>> = dados.parameters.iter().map(|q| Some(sub(q.ty))).collect();
        let tabela = copia.as_ref().unwrap_or(&self.consulta.tabela);
        let fmt = |t: TypeId| tabela.format_sem_alias(t, &self.consulta.nomes, &self.consulta.programa);
        let retorno = fmt(retorno_t);
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
        let lista = self.lista_de_parametros_com(unidade, parametros, &tipos, &fmt);
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
}

/// O elemento cuja documentação o hover procura.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Documentavel {
    Funcao(FunctionElementId),
    Variavel(dartforge_elements::model::VariableId),
}

/// Os parâmetros de tipo da classe pelos argumentos do receptor, com a
/// cópia da tabela em que esses argumentos existem.
type Substituicao = (std::collections::HashMap<dartforge_types::TypeParamId, TypeId>, dartforge_types::TypeTable);

/// A declaração em `declaracao` é variável de um `for` de coleção
/// (`[for (var i = 0; …) …]`, `{for (var x in l) …}`, inclusive por padrão).
fn em_for_de_colecao(ast: &ast::Ast, declaracao: usize) -> bool {
    fn no_padrao(ast: &ast::Ast, p: ast::PatternId, d: usize) -> bool {
        let s = ast.pattern(p).span;
        s.start <= d && d < s.end
    }
    fn varrer(ast: &ast::Ast, els: &[ast::CollectionElement], d: usize) -> bool {
        els.iter().any(|el| match el {
            ast::CollectionElement::For { init, body, .. } => {
                let aqui = match init {
                    Some(ast::ForInit::Variables(vl)) => vl.variables.iter().any(|v| v.name.span.start == d),
                    Some(ast::ForInit::Pattern { pattern, .. }) => no_padrao(ast, *pattern, d),
                    _ => false,
                };
                aqui || varrer(ast, std::slice::from_ref(body), d)
            }
            ast::CollectionElement::ForIn { target, body, .. } => {
                let aqui = match target {
                    ast::ForInTarget::Declared { name, .. } => name.span.start == d,
                    ast::ForInTarget::Pattern { pattern, .. } => no_padrao(ast, *pattern, d),
                    ast::ForInTarget::Expression(_) => false,
                };
                aqui || varrer(ast, std::slice::from_ref(body), d)
            }
            ast::CollectionElement::If { then, else_, .. } => {
                varrer(ast, std::slice::from_ref(then), d) || else_.as_ref().is_some_and(|x| varrer(ast, std::slice::from_ref(x), d))
            }
            _ => false,
        })
    }
    ast.exprs.iter().any(|e| match &e.kind {
        ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } => varrer(ast, elements, declaracao),
        _ => false,
    })
}
