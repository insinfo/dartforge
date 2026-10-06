//! `Change to` e os `Create …` do Dart 3.6.2 (docs/LSP-ESPECIFICACAO.md
//! §13.7.4.2 e §13.7.4.3): `change_to.dart`, `create_class.dart`,
//! `create_mixin.dart`, `create_field.dart`, `create_getter.dart`,
//! `create_setter.dart`, `create_local_variable.dart`,
//! `create_parameter.dart`, `create_method_or_function.dart`,
//! `create_function.dart`, `create_method.dart` e
//! `create_extension_member.dart`, sobre a árvore do analyzer.
//!
//! Os `undefined_*` não são publicados por este servidor: cada nome sem
//! resolução das linhas pedidas recebe o código que o analyzer daria
//! (`UNDEFINED_IDENTIFIER`, `UNDEFINED_FUNCTION`, `UNDEFINED_CLASS`,
//! `UNDEFINED_METHOD`, `UNDEFINED_GETTER`, `UNDEFINED_SETTER`, os de
//! extensão) e os produtores desse código, na ordem do `fix_internal.dart`.
//! Os códigos publicados (`cast_to_non_type`, `not_a_type`,
//! `extends_non_class`…) usam os mesmos produtores pelo diagnóstico.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Edicao;
use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::Marca;
use crate::escrever_tipo::Escritor;
use crate::inserir::Filtro;
use crate::projeto::Projeto;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Mudanca, Texto, UM_RECUO};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, UnitId};
use dartforge_frontend::ast;
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::BTreeSet;

/// O código de erro que escolhe os produtores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Codigo {
    Identificador,
    Funcao,
    Classe,
    Metodo,
    Getter,
    Setter,
    GetterDeExtensao,
    MetodoDeExtensao,
    SetterDeExtensao,
    /// `CAST_TO_NON_TYPE`, `NOT_A_TYPE`.
    NaoTipo,
    /// `EXTENDS_NON_CLASS`, `IMPLEMENTS_NON_CLASS`, `MIXIN_OF_NON_CLASS`,
    /// `NEW_WITH_NON_TYPE`.
    NaoClasse,
    /// `INVALID_ANNOTATION`, `UNDEFINED_ANNOTATION`.
    Anotacao,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Produtor {
    TrocarClasse,
    TrocarAnotacao,
    TrocarFuncao,
    TrocarGetterOuSetter,
    TrocarMetodo,
    CriarClasse,
    CriarMixin,
    CriarCampo,
    CriarGetter,
    CriarSetter,
    CriarLocal,
    CriarParametro,
    CriarMetodoOuFuncao,
    CriarFuncao,
    CriarMetodo,
    CriarGetterDeExtensao,
    CriarMetodoDeExtensao,
    CriarSetterDeExtensao,
}

/// Os produtores de cada código (`nonLintProducerMap`).
fn produtores(c: Codigo) -> &'static [Produtor] {
    use Produtor::*;
    match c {
        Codigo::Identificador => &[TrocarGetterOuSetter, CriarClasse, CriarCampo, CriarGetter, CriarLocal, CriarParametro, CriarMetodoOuFuncao, CriarMixin, CriarSetter],
        Codigo::Funcao => &[TrocarFuncao, CriarClasse, CriarFuncao],
        Codigo::Classe => &[TrocarClasse, CriarClasse, CriarMixin],
        Codigo::Metodo => &[TrocarMetodo, CriarClasse, CriarMetodoDeExtensao, CriarFuncao, CriarMetodo],
        Codigo::Getter => &[TrocarGetterOuSetter, CriarClasse, CriarGetterDeExtensao, CriarCampo, CriarGetter, CriarLocal, CriarMetodoOuFuncao, CriarMixin],
        Codigo::Setter => &[TrocarGetterOuSetter, CriarSetterDeExtensao, CriarCampo, CriarSetter],
        Codigo::GetterDeExtensao => &[TrocarGetterOuSetter, CriarGetter],
        Codigo::MetodoDeExtensao => &[TrocarMetodo, CriarMetodo],
        Codigo::SetterDeExtensao => &[TrocarGetterOuSetter, CriarSetter],
        Codigo::NaoTipo => &[TrocarClasse, CriarClasse, CriarMixin],
        Codigo::NaoClasse => &[TrocarClasse, CriarClasse],
        Codigo::Anotacao => &[TrocarAnotacao, CriarClasse],
    }
}

/// Distância de Levenshtein com limiar (`levenshtein.dart`): `1 << 20`
/// quando passa do limiar ou a diferença de tamanhos passa dele.
fn levenshtein(s: &str, t: &str, limiar: usize) -> usize {
    const INFINITO: usize = 1 << 20;
    let s: Vec<char> = s.chars().collect();
    let t: Vec<char> = t.chars().collect();
    if s.len().abs_diff(t.len()) > limiar {
        return INFINITO;
    }
    let mut anterior: Vec<usize> = (0..=t.len()).collect();
    for (i, a) in s.iter().enumerate() {
        let mut atual = vec![i + 1; t.len() + 1];
        let mut minimo = atual[0];
        for (j, b) in t.iter().enumerate() {
            let custo = usize::from(a != b);
            atual[j + 1] = (anterior[j] + custo).min(anterior[j + 1] + 1).min(atual[j] + 1);
            minimo = minimo.min(atual[j + 1]);
        }
        if minimo > limiar {
            return INFINITO;
        }
        anterior = atual;
    }
    let d = anterior[t.len()];
    if d > limiar { INFINITO } else { d }
}

/// `_ClosestElementFinder`: o primeiro candidato com distância menor que a
/// melhor (começando em 3).
struct MaisProximo {
    alvo: String,
    distancia: usize,
    nome: Option<String>,
}

impl MaisProximo {
    fn novo(alvo: &str) -> MaisProximo {
        MaisProximo { alvo: alvo.to_string(), distancia: 3, nome: None }
    }

    /// `name` (o comparado; `x=` num setter) e o proposto.
    fn ver(&mut self, nome: &str, proposto: &str) {
        let d = levenshtein(nome, &self.alvo, self.distancia);
        if d < self.distancia {
            self.distancia = d;
            self.nome = Some(proposto.to_string());
        }
    }
}

/// O `inGetterContext` de um `SimpleIdentifier`.
fn em_contexto_de_leitura(cx: &Contexto<'_>, n: usize) -> bool {
    let Some(pai0) = cx.pai(n) else { return true };
    let (mut pai, mut alvo) = (pai0, n);
    match cx.especie(pai0) {
        "PrefixedIdentifier" | "PropertyAccess" => {
            if cx.filhos(pai0).first() == Some(&n) {
                return true;
            }
            let Some(p) = cx.pai(pai0) else { return true };
            pai = p;
            alvo = pai0;
        }
        _ => {}
    }
    match cx.especie(pai) {
        "Label" => false,
        "AssignmentExpression" => !(cx.filhos(pai).first() == Some(&alvo) && operador_de_atribuicao(cx, pai) == "="),
        "ForEachPartsWithIdentifier" => cx.filhos(pai).first() != Some(&alvo),
        "FieldFormalParameter" => false,
        _ => true,
    }
}

/// O `inSetterContext` de um `SimpleIdentifier`.
fn em_contexto_de_escrita(cx: &Contexto<'_>, n: usize) -> bool {
    let Some(pai0) = cx.pai(n) else { return false };
    let (mut pai, mut alvo) = (pai0, n);
    match cx.especie(pai0) {
        "PrefixedIdentifier" | "PropertyAccess" => {
            if cx.filhos(pai0).first() == Some(&n) {
                return false;
            }
            let Some(p) = cx.pai(pai0) else { return false };
            pai = p;
            alvo = pai0;
        }
        _ => {}
    }
    match cx.especie(pai) {
        "PrefixExpression" | "PostfixExpression" => {
            let t = cx.texto_do_no(pai);
            t.starts_with("++") || t.starts_with("--") || t.ends_with("++") || t.ends_with("--")
        }
        "AssignmentExpression" => cx.filhos(pai).first() == Some(&alvo),
        "ForEachPartsWithIdentifier" => cx.filhos(pai).first() == Some(&alvo),
        _ => false,
    }
}

/// O operador de uma `AssignmentExpression`.
fn operador_de_atribuicao<'c>(cx: &'c Contexto<'_>, a: usize) -> &'c str {
    let Some(&esq) = cx.filhos(a).first() else { return "" };
    match cx.token_seguinte(cx.arvore.nos[esq].fim) {
        Some(t) => &cx.fonte[t.start..t.end],
        None => "",
    }
}

/// `climbPropertyAccess`.
fn subir_acesso(cx: &Contexto<'_>, mut n: usize) -> usize {
    while let Some(p) = cx.pai(n) {
        let e = cx.especie(p);
        if (e == "PrefixedIdentifier" || e == "PropertyAccess") && cx.filhos(p).last() == Some(&n) && cx.filhos(p).len() > 1 {
            n = p;
        } else {
            break;
        }
    }
    n
}

/// O alvo de um nome qualificado: o prefixo do `PrefixedIdentifier` ou o
/// alvo do `PropertyAccess` (`getQualifiedPropertyTarget`).
fn alvo_qualificado(cx: &Contexto<'_>, n: usize) -> Option<usize> {
    let p = cx.pai(n)?;
    match cx.especie(p) {
        "PrefixedIdentifier" | "PropertyAccess" if cx.filhos(p).last() == Some(&n) && cx.filhos(p).len() > 1 => cx.filhos(p).first().copied(),
        _ => None,
    }
}

impl Contexto<'_> {
    /// O elemento resolvido de um nó identificador.
    fn resolvido(&self, n: usize) -> Option<&Resolved> {
        let x = match self.arvore.nos[n].marca {
            Marca::Expr(x) => x,
            _ => self.expr_do_no(n)?,
        };
        self.corpos.get_resolved(x)
    }

    /// A classe (ou mixin, enum, tipo de extensão) que o identificador nomeia.
    fn classe_nomeada(&self, n: usize) -> Option<ClassId> {
        match self.resolvido(n) {
            Some(Resolved::Element(Element::Class(c))) => Some(*c),
            _ => None,
        }
    }

    /// A extensão que o identificador nomeia, ou a do `ExtensionOverride`
    /// (`E(x)` / `p.E(x)` como invocação cujo nome resolve para a extensão).
    fn extensao_do_alvo(&self, alvo: usize) -> Option<ExtensionId> {
        if let Some(Resolved::Element(Element::Extension(x))) = self.resolvido(alvo) {
            return Some(*x);
        }
        if matches!(self.especie(alvo), "MethodInvocation" | "FunctionExpressionInvocation") {
            for &f in self.filhos(alvo) {
                if self.especie(f) == "SimpleIdentifier"
                    && let Some(Resolved::Element(Element::Extension(x))) = self.resolvido(f)
                {
                    return Some(*x);
                }
            }
        }
        None
    }

    /// O alvo é um `ExtensionOverride` (`E(x)`), não o nome da extensão.
    fn e_override_de_extensao(&self, alvo: usize) -> bool {
        matches!(self.especie(alvo), "MethodInvocation" | "FunctionExpressionInvocation") && self.extensao_do_alvo(alvo).is_some()
    }

    /// `getTargetInterfaceElement2(target)`.
    fn interface_do_alvo(&self, alvo: usize) -> Option<ClassId> {
        // O nome de uma classe como receptor (`A.m()`): o analyzer não dá
        // tipo estático ao literal de tipo e cai no elemento.
        if let Some(c) = self.classe_nomeada(alvo) {
            return Some(c);
        }
        match self.p.consulta.tabela.get(self.tipo_do_no(alvo)?) {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(*class),
            _ => None,
        }
    }

    /// `enclosingInterfaceElement2`: a `ClassDeclaration` ou
    /// `MixinDeclaration` envolvente.
    fn interface_envolvente(&self, n: usize) -> Option<(ClassId, usize)> {
        let mut atual = Some(n);
        while let Some(k) = atual {
            if matches!(self.especie(k), "ClassDeclaration" | "MixinDeclaration")
                && let Marca::Decl(d) = self.arvore.nos[k].marca
            {
                return self.classe_da_declaracao(self.unidade, d).map(|c| (c, k));
            }
            atual = self.pai(k);
        }
        None
    }

    /// `enclosingExtensionElement2`.
    fn extensao_envolvente(&self, n: usize) -> Option<(ExtensionId, usize)> {
        let prog = self.p.programa();
        let mut atual = Some(n);
        while let Some(k) = atual {
            if self.especie(k) == "ExtensionDeclaration"
                && let Marca::Decl(d) = self.arvore.nos[k].marca
            {
                let x = (0..prog.extensions.len()).map(|i| ExtensionId(i as u32)).find(|&x| prog.extension(x).decl.unit == self.unidade && prog.extension(x).decl.decl == d)?;
                return Some((x, k));
            }
            atual = self.pai(k);
        }
        None
    }

    /// `inStaticContext`.
    fn em_contexto_estatico(&self, n: usize) -> bool {
        let mut atual = Some(n);
        while let Some(k) = atual {
            match self.especie(k) {
                "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer" => return true,
                "FieldDeclaration" => {
                    let palavras = self.tokens_iniciais(k);
                    let estatico = palavras.iter().any(|s| s == "static");
                    let late = palavras.iter().any(|s| s == "late");
                    return estatico || !late;
                }
                "MethodDeclaration" => return self.tokens_iniciais(k).iter().any(|s| s == "static"),
                _ => {}
            }
            atual = self.pai(k);
        }
        false
    }

    /// As palavras do começo de uma declaração (depois de doc e anotações).
    pub(crate) fn tokens_iniciais_pub(&self, n: usize) -> Vec<String> {
        self.tokens_iniciais(n)
    }

    fn tokens_iniciais(&self, n: usize) -> Vec<String> {
        let mut v = Vec::new();
        let Some(mut t) = self.primeiro_token_apos_comentario_e_metadados(n) else { return v };
        loop {
            let s = &self.fonte[t.start..t.end];
            if !matches!(s, "static" | "late" | "final" | "const" | "var" | "abstract" | "external" | "covariant") {
                break;
            }
            v.push(s.to_string());
            match self.token_seguinte(t.end) {
                Some(p) => t = p,
                None => break,
            }
        }
        v
    }

    /// O parâmetro (tipo) que recebe o argumento `arg` (`staticParameterElement`
    /// / `correspondingParameter`).
    fn parametro_correspondente(&self, arg: usize) -> Option<TypeId> {
        let (valor, nome) = match self.pai(arg) {
            Some(p) if self.especie(p) == "NamedExpression" => (p, self.filhos(p).first().and_then(|&r| self.filhos(r).first()).map(|&i| self.texto_do_no(i).to_string())),
            _ => (arg, None),
        };
        let lista = self.pai(valor).filter(|&l| self.especie(l) == "ArgumentList")?;
        let chamada = self.pai(lista)?;
        let tabela = &self.p.consulta.tabela;
        let tipo_f = match self.especie(chamada) {
            "MethodInvocation" | "FunctionExpressionInvocation" => {
                let alvo = match self.especie(chamada) {
                    "MethodInvocation" => self.nome_do_metodo(chamada)?,
                    _ => *self.filhos(chamada).first()?,
                };
                match self.resolvido(chamada) {
                    Some(Resolved::Constructor(f)) => Some(self.p.consulta.outline.functions.get(f.0 as usize)?.signature),
                    _ => self.tipo_do_no(alvo),
                }
            }
            "InstanceCreationExpression" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" => match self.resolvido(chamada) {
                Some(Resolved::Constructor(f)) => Some(self.p.consulta.outline.functions.get(f.0 as usize)?.signature),
                _ => None,
            },
            _ => None,
        }?;
        let Type::Function { positional, optional, named, .. } = tabela.get(tipo_f) else { return None };
        match nome {
            Some(n) => named.iter().find(|(s, _, _)| self.p.nome(*s) == n).map(|(_, t, _)| *t),
            None => {
                let posicao = self.filhos(lista).iter().take_while(|&&a| a != valor).filter(|&&a| self.especie(a) != "NamedExpression").count();
                positional.iter().chain(optional.iter()).nth(posicao).copied()
            }
        }
    }

    /// O tipo de retorno do executável envolvente.
    fn retorno_envolvente(&self, n: usize) -> Option<TypeId> {
        let mut atual = self.pai(n);
        while let Some(k) = atual {
            match self.especie(k) {
                "FunctionExpression" => {
                    if let Marca::Funcao(fid) = self.arvore.nos[k].marca {
                        if let Some(f) = self.p.funcao_do_no(self.unidade, fid) {
                            return self.p.consulta.outline.functions.get(f.0 as usize).map(|d| d.return_type);
                        }
                        let nome = self.ast.function(fid).name?;
                        return match self.corpos.tipo_local(nome.span.start).map(|t| self.p.consulta.tabela.get(t)) {
                            Some(Type::Function { ret, .. }) => Some(*ret),
                            _ => None,
                        };
                    }
                    // A closure: o tipo da expressão de função.
                    let t = self.tipo_do_no(k)?;
                    return match self.p.consulta.tabela.get(t) {
                        Type::Function { ret, .. } => Some(*ret),
                        _ => None,
                    };
                }
                "MethodDeclaration" | "FunctionDeclaration" => {
                    let Marca::Funcao(fid) = self.arvore.nos[k].marca else { return None };
                    let f = self.p.funcao_do_no(self.unidade, fid)?;
                    return self.p.consulta.outline.functions.get(f.0 as usize).map(|d| d.return_type);
                }
                "ConstructorDeclaration" => return None,
                _ => {}
            }
            atual = self.pai(k);
        }
        None
    }

    /// `inferUndefinedExpressionType(expression)`.
    fn tipo_inferido_indefinido(&self, e: usize) -> Option<TypeId> {
        let core = &self.p.consulta.core;
        let pai = self.pai(e)?;
        let bool_ = Some(core.bool_);
        if self.especie(e) == "MethodInvocation" {
            if self.especie(pai) == "CascadeExpression" && self.pai(pai).is_some_and(|g| self.especie(g) == "ExpressionStatement") {
                return Some(core.void_);
            }
            if self.especie(pai) == "ExpressionStatement" {
                return Some(core.void_);
            }
        }
        match self.especie(pai) {
            "ConditionalExpression" => {
                if self.filhos(pai).first() == Some(&e) {
                    return bool_;
                }
                let t = self.parametro_correspondente(pai)?;
                return Some(t);
            }
            "ExpressionFunctionBody" | "ReturnStatement" => return self.retorno_envolvente(e),
            "VariableDeclaration" => {
                if self.filhos(pai).first() == Some(&e) {
                    let s = crate::projeto::palavra(self.fonte, self.arvore.nos[pai].inicio)?;
                    return self.corpos.tipo_local(s.start).or_else(|| {
                        crate::destaques::variavel_declarada_em(self.p, self.unidade, s.start).and_then(|v| self.p.consulta.tipo_da_variavel(v))
                    });
                }
            }
            "AssignmentExpression" => {
                let f = self.filhos(pai);
                if f.first() == Some(&e) {
                    return f.get(1).and_then(|&d| self.tipo_do_no(d));
                }
                if f.get(1) == Some(&e) {
                    if operador_de_atribuicao(self, pai) == "=" {
                        // `writeType`: o tipo do alvo da escrita.
                        return self.tipo_do_no(f[0]);
                    }
                    return self.parametro_do_operador(pai);
                }
            }
            "BinaryExpression" => {
                if self.filhos(pai).get(1) == Some(&e)
                    && let Some(t) = self.parametro_do_operador(pai)
                {
                    return Some(t);
                }
            }
            "ArgumentList" | "NamedExpression" => return self.parametro_correspondente(e),
            _ => {}
        }
        match self.especie(pai) {
            "AssertStatement" | "IfStatement" | "WhileStatement" if self.filhos(pai).first() == Some(&e) => bool_,
            "DoStatement" if self.filhos(pai).last() == Some(&e) => bool_,
            "PrefixExpression" if self.texto_do_no(pai).starts_with('!') => bool_,
            "BinaryExpression" => {
                let esq = *self.filhos(pai).first()?;
                let op = self.token_seguinte(self.arvore.nos[esq].fim)?;
                matches!(&self.fonte[op.start..op.end], "&&" | "||").then_some(core.bool_)
            }
            _ => None,
        }
    }

    /// O tipo do único parâmetro do operador de um binário ou de um `op=`.
    fn parametro_do_operador(&self, n: usize) -> Option<TypeId> {
        let x = self.expr_do_no(n)?;
        let f = match self.corpos.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => *f,
            _ => return None,
        };
        let d = self.p.consulta.outline.functions.get(f.0 as usize)?;
        (d.parameters.len() == 1).then(|| d.parameters[0].ty)
    }

    /// `getMembers(clazz)`: os membros não sintéticos (acessores, campos,
    /// métodos) de cada supertipo (`allSupertypes`) e da própria classe:
    /// (nome comparado, nome exibido, espécie).
    fn membros(&self, c: ClassId) -> Vec<(String, String, Membro)> {
        let prog = self.p.programa();
        let mut ordem = todos_os_supertipos(self.p, c);
        ordem.push(c);
        let mut v = Vec::new();
        for k in ordem {
            let cl = prog.class(k);
            let mut fs: Vec<FunctionElementId> = cl.instance_members.values().chain(cl.static_members.values()).copied().collect();
            fs.sort_by_key(|f| f.0);
            // Acessores explícitos.
            for &f in &fs {
                let fe = prog.function(f);
                match fe.kind {
                    FunctionKind::Getter => v.push((self.p.nome(fe.name).to_string(), self.p.nome(fe.name).to_string(), Membro::Getter)),
                    FunctionKind::Setter => {
                        let n = self.p.nome(fe.name).to_string();
                        let base = crate::projeto::nome_base(&n).to_string();
                        v.push((format!("{base}="), base, Membro::Setter));
                    }
                    _ => {}
                }
            }
            // Campos.
            for &var in &cl.fields {
                let ve = prog.variable(var);
                let n = self.p.nome(ve.name).to_string();
                v.push((n.clone(), n, Membro::Campo { getter: ve.getter.is_some(), setter: ve.setter.is_some() }));
            }
            // Métodos.
            for &f in &fs {
                let fe = prog.function(f);
                match fe.kind {
                    FunctionKind::Function => v.push((self.p.nome(fe.name).to_string(), self.p.nome(fe.name).to_string(), Membro::Metodo)),
                    FunctionKind::Operator => v.push((self.p.nome(fe.name).to_string(), self.p.nome(fe.name).to_string(), Membro::Operador)),
                    _ => {}
                }
            }
        }
        v
    }

    /// `getExtensionMembers(extension)`.
    fn membros_da_extensao(&self, x: ExtensionId) -> Vec<(String, String, Membro)> {
        let prog = self.p.programa();
        let e = prog.extension(x);
        let mut fs: Vec<FunctionElementId> = e.instance_members.values().chain(e.static_members.values()).copied().collect();
        fs.sort_by_key(|f| f.0);
        let mut v = Vec::new();
        for &f in &fs {
            let fe = prog.function(f);
            let n = self.p.nome(fe.name).to_string();
            match fe.kind {
                FunctionKind::Getter => v.push((n.clone(), n, Membro::Getter)),
                FunctionKind::Setter => {
                    let base = crate::projeto::nome_base(&n).to_string();
                    v.push((format!("{base}="), base, Membro::Setter));
                }
                _ => {}
            }
        }
        for &var in &e.fields {
            let ve = prog.variable(var);
            let n = self.p.nome(ve.name).to_string();
            v.push((n.clone(), n, Membro::Campo { getter: ve.getter.is_some(), setter: ve.setter.is_some() }));
        }
        for &f in &fs {
            let fe = prog.function(f);
            let n = self.p.nome(fe.name).to_string();
            match fe.kind {
                FunctionKind::Function => v.push((n.clone(), n, Membro::Metodo)),
                FunctionKind::Operator => v.push((n.clone(), n, Membro::Operador)),
                _ => {}
            }
        }
        v
    }
}

/// A espécie de um membro candidato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Membro {
    Getter,
    Setter,
    Campo { getter: bool, setter: bool },
    Metodo,
    Operador,
}

/// `allSupertypes` (`ClassHierarchy.implementedInterfaces`): o supertipo e os
/// dele, as restrições `on`, as interfaces e os mixins, na ordem do
/// `InterfacesMerger`; as aplicações de mixin sintéticas se desfazem no
/// supertipo e nos mixins.
fn todos_os_supertipos(p: &Projeto, c: ClassId) -> Vec<ClassId> {
    fn diretos(p: &Projeto, c: ClassId) -> (Option<ClassId>, Vec<ClassId>, Vec<ClassId>, Vec<ClassId>) {
        let prog = p.programa();
        let cl = prog.class(c);
        let mut sup = cl.supertype_class;
        let mut mixins: Vec<ClassId> = Vec::new();
        // `S with M1, M2`: a cadeia sintética `S&M1&M2`.
        while let Some(s) = sup {
            let k = prog.class(s);
            if k.kind == ClassKind::MixinApplication && k.decl.is_none() {
                let mut m = k.mixin_classes.clone();
                m.extend(mixins);
                mixins = m;
                sup = k.supertype_class;
            } else {
                break;
            }
        }
        mixins.extend(cl.mixin_classes.iter().copied());
        (sup, cl.on_classes.clone(), cl.interface_classes.clone(), mixins)
    }
    fn implementados(p: &Projeto, c: ClassId, memo: &mut std::collections::HashMap<ClassId, Vec<ClassId>>, pilha: &mut Vec<ClassId>) -> Vec<ClassId> {
        if let Some(v) = memo.get(&c) {
            return v.clone();
        }
        if pilha.contains(&c) {
            return Vec::new();
        }
        pilha.push(c);
        let mut v: Vec<ClassId> = Vec::new();
        let (sup, on, interfaces, mixins) = diretos(p, c);
        let juntar = |t: ClassId, v: &mut Vec<ClassId>, memo: &mut std::collections::HashMap<ClassId, Vec<ClassId>>, pilha: &mut Vec<ClassId>| {
            if !v.contains(&t) {
                v.push(t);
            }
            for x in implementados(p, t, memo, pilha) {
                if !v.contains(&x) {
                    v.push(x);
                }
            }
        };
        if let Some(s) = sup {
            juntar(s, &mut v, memo, pilha);
        }
        for t in on.into_iter().chain(interfaces).chain(mixins) {
            juntar(t, &mut v, memo, pilha);
        }
        pilha.pop();
        memo.insert(c, v.clone());
        v
    }
    implementados(p, c, &mut std::collections::HashMap::new(), &mut Vec::new())
}

/// `nameOfType`: o nome de um `SimpleIdentifier` filho de `NamedType` ou
/// com a primeira letra igual à maiúscula dela.
fn nome_de_tipo(cx: &Contexto<'_>, n: usize) -> Option<String> {
    if cx.especie(n) != "SimpleIdentifier" {
        return None;
    }
    let nome = cx.texto_do_no(n).to_string();
    let pai_tipo = cx.pai(n).is_some_and(|p| cx.especie(p) == "NamedType");
    let primeira = nome.chars().next()?;
    let maiuscula = primeira.to_uppercase().next() == Some(primeira);
    (pai_tipo || maiuscula).then_some(nome)
}

/// O resultado de um produtor: título, espécie, edições (por URI) e
/// bibliotecas a importar na unidade de destino.
struct Proposta {
    titulo: String,
    especie: &'static str,
    unidade: UnitId,
    edicoes: Vec<(Span, String)>,
    importar: BTreeSet<LibraryId>,
}

/// O `CompilationUnitMember` de topo que contém o nó.
fn membro_de_topo(cx: &Contexto<'_>, n: usize) -> Option<usize> {
    let mut atual = Some(n);
    while let Some(k) = atual {
        if cx.pai(k) == Some(0) {
            return Some(k);
        }
        atual = cx.pai(k);
    }
    None
}

/// O `CompilationUnitMember` mais interno (uma função local também é).
fn membro_mais_interno(cx: &Contexto<'_>, n: usize) -> Option<usize> {
    let mut atual = Some(n);
    while let Some(k) = atual {
        if matches!(
            cx.especie(k),
            "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "FunctionDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias" | "ClassTypeAlias" | "TopLevelVariableDeclaration"
        ) {
            return Some(k);
        }
        atual = cx.pai(k);
    }
    None
}

/// O nó da declaração de uma classe ou extensão na unidade dela, com o
/// contexto daquela unidade.
fn declaracao_na_unidade<'p>(p: &'p Projeto, unidade: UnitId, decl: ast::DeclId) -> Option<(Contexto<'p>, usize)> {
    let cx = Contexto::novo(p, unidade);
    let n = cx.arvore.nos.iter().position(|no| matches!(no.marca, Marca::Decl(d) if d == decl) && no.especie.ends_with("Declaration"))?;
    Some((cx, n))
}

impl Projeto {
    /// As propostas de um produtor para o nó.
    fn propor(&self, cx: &Contexto<'_>, produtor: Produtor, node: usize) -> Option<Proposta> {
        use Produtor::*;
        let eol = Texto::novo(cx.fonte).eol();
        let prog = self.programa();
        match produtor {
            TrocarClasse | TrocarAnotacao => {
                let mut alvo = node;
                if produtor == TrocarAnotacao {
                    // `Annotation` com nome sem elemento e com argumentos.
                    let anotacao = cx.este_ou_ancestral_pub(node, "Annotation")?;
                    let nome = *cx.filhos(anotacao).first()?;
                    if cx.resolvido(nome).is_some() || cx.filhos_da_especie(anotacao, "ArgumentList").is_empty() {
                        return None;
                    }
                    alvo = nome;
                }
                let (prefixo, token) = match cx.especie(alvo) {
                    "NamedType" => {
                        let pr = cx.filhos_da_especie(alvo, "ImportPrefixReference").first().map(|&p| cx.texto_do_no(p).trim_end_matches('.').trim().to_string());
                        let inicio = cx.filhos_da_especie(alvo, "ImportPrefixReference").first().map_or(cx.arvore.nos[alvo].inicio, |&p| cx.arvore.nos[p].fim);
                        (pr, crate::projeto::palavra(cx.fonte, cx.token_seguinte(inicio)?.start)?)
                    }
                    "PrefixedIdentifier" => {
                        let f = cx.filhos(alvo);
                        let p = *f.first()?;
                        if !matches!(cx.resolvido(p), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) {
                            return None;
                        }
                        (Some(cx.texto_do_no(p).to_string()), cx.arvore.span(*f.get(1)?))
                    }
                    "SimpleIdentifier" => (None, cx.arvore.span(alvo)),
                    _ => return None,
                };
                let lib = prog.unit(cx.unidade).library;
                let mut melhor = MaisProximo::novo(&cx.fonte[token.start..token.end]);
                if prefixo.is_none() {
                    // `unit.classes` de cada unidade (só classes).
                    for &u in prog.library(lib).units.iter() {
                        let mut cs: Vec<(usize, ClassId)> = (0..prog.classes.len())
                            .map(|i| ClassId(i as u32))
                            .filter_map(|c| {
                                let d = prog.class(c).decl?;
                                (d.unit == u && matches!(prog.class(c).kind, ClassKind::Class | ClassKind::MixinApplication)).then(|| (prog.unit(u).ast.decl(d.decl).span.start, c))
                            })
                            .collect();
                        cs.sort();
                        for (_, c) in cs {
                            let n = self.nome(prog.class(c).name);
                            melhor.ver(n, n);
                        }
                    }
                }
                for imp in prog.library(lib).imports.iter().filter(|i| prog.library(lib).units.first() == Some(&i.unit)) {
                    let pr = imp.prefix.map(|s| self.nome(s).to_string());
                    if pr != prefixo {
                        continue;
                    }
                    let mut nomes: Vec<(&str, Element)> = prog
                        .library(imp.library)
                        .exported
                        .iter()
                        .filter_map(|(s, b)| b.getter.map(|g| (self.nome(*s), g)))
                        .filter(|(n, _)| {
                            imp.combinators.iter().all(|c| match c {
                                ast::Combinator::Show(ns) => ns.iter().any(|x| self.nome(x.sym) == *n),
                                ast::Combinator::Hide(ns) => !ns.iter().any(|x| self.nome(x.sym) == *n),
                            })
                        })
                        .collect();
                    nomes.sort_by(|a, b| a.0.cmp(b.0));
                    for (n, el) in nomes {
                        if matches!(el, Element::Class(_)) {
                            melhor.ver(n, n);
                        }
                    }
                }
                let proposto = melhor.nome?;
                Some(Proposta { titulo: format!("Change to '{proposto}'"), especie: "quickfix.change.to", unidade: cx.unidade, edicoes: vec![(token, proposto)], importar: BTreeSet::new() })
            }
            TrocarFuncao => {
                if cx.especie(node) != "SimpleIdentifier" {
                    return None;
                }
                let mut prefixo = None;
                if let Some(pai) = cx.pai(node)
                    && cx.especie(pai) == "MethodInvocation"
                    && cx.nome_do_metodo(pai) == Some(node)
                    && let Some(&alvo) = cx.filhos(pai).first()
                    && alvo != node
                    && matches!(cx.resolvido(alvo), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))))
                {
                    prefixo = Some(cx.texto_do_no(alvo).to_string());
                }
                let lib = prog.unit(cx.unidade).library;
                let mut melhor = MaisProximo::novo(cx.texto_do_no(node));
                if prefixo.is_none() {
                    for &u in prog.library(lib).units.iter() {
                        let mut fs: Vec<(usize, &str)> = prog
                            .unit(u)
                            .ast
                            .decls
                            .iter()
                            .filter_map(|d| match &d.kind {
                                ast::DeclKind::Function(f) => {
                                    let func = prog.unit(u).ast.function(*f);
                                    (func.kind == ast::FunctionKind::Function).then(|| func.name.map(|n| (n.span.start, self.nome(n.sym))))?
                                }
                                _ => None,
                            })
                            .collect();
                        fs.sort();
                        for (_, n) in fs {
                            melhor.ver(n, n);
                        }
                    }
                }
                for imp in prog.library(lib).imports.iter().filter(|i| prog.library(lib).units.first() == Some(&i.unit)) {
                    if imp.prefix.map(|s| self.nome(s).to_string()) != prefixo {
                        continue;
                    }
                    let mut nomes: Vec<&str> = prog
                        .library(imp.library)
                        .exported
                        .iter()
                        .filter(|(_, b)| matches!(b.getter, Some(Element::Function(f)) if prog.function(f).kind == FunctionKind::Function))
                        .map(|(s, _)| self.nome(*s))
                        .filter(|n| {
                            imp.combinators.iter().all(|c| match c {
                                ast::Combinator::Show(ns) => ns.iter().any(|x| self.nome(x.sym) == *n),
                                ast::Combinator::Hide(ns) => !ns.iter().any(|x| self.nome(x.sym) == *n),
                            })
                        })
                        .collect();
                    nomes.sort();
                    for n in nomes {
                        melhor.ver(n, n);
                    }
                }
                let proposto = melhor.nome?;
                Some(Proposta { titulo: format!("Change to '{proposto}'"), especie: "quickfix.change.to", unidade: cx.unidade, edicoes: vec![(cx.arvore.span(node), proposto)], importar: BTreeSet::new() })
            }
            TrocarGetterOuSetter | TrocarMetodo => {
                if cx.especie(node) != "SimpleIdentifier" {
                    return None;
                }
                let pai = cx.pai(node)?;
                let (alvo, predicado): (Option<usize>, Box<dyn Fn(Membro) -> bool>) = if produtor == TrocarMetodo {
                    if cx.especie(pai) != "MethodInvocation" || cx.nome_do_metodo(pai) != Some(node) {
                        return None;
                    }
                    let alvo = cx.filhos(pai).first().copied().filter(|&a| a != node);
                    (alvo, Box::new(|m| m == Membro::Metodo))
                } else {
                    let le = em_contexto_de_leitura(cx, node);
                    let escreve = em_contexto_de_escrita(cx, node);
                    (
                        alvo_qualificado(cx, node),
                        Box::new(move |m| match m {
                            Membro::Getter => le,
                            Membro::Setter => escreve,
                            Membro::Campo { getter, setter } => (le && getter) || (escreve && setter),
                            _ => false,
                        }),
                    )
                };
                let membros = match alvo {
                    None => {
                        // Só a `ClassDeclaration` envolvente.
                        let classe = cx.este_ou_ancestral_pub(node, "ClassDeclaration")?;
                        let Marca::Decl(d) = cx.arvore.nos[classe].marca else { return None };
                        cx.membros(cx.classe_da_declaracao(cx.unidade, d)?)
                    }
                    Some(a) => match cx.extensao_do_alvo(a) {
                        Some(x) => cx.membros_da_extensao(x),
                        None => cx.membros(cx.interface_do_alvo(a)?),
                    },
                };
                let mut melhor = MaisProximo::novo(cx.texto_do_no(node));
                for (comparado, exibido, m) in &membros {
                    if predicado(*m) {
                        melhor.ver(comparado, exibido);
                    }
                }
                let proposto = melhor.nome?;
                Some(Proposta { titulo: format!("Change to '{proposto}'"), especie: "quickfix.change.to", unidade: cx.unidade, edicoes: vec![(cx.arvore.span(node), proposto)], importar: BTreeSet::new() })
            }
            CriarClasse | CriarMixin => {
                let mut alvo = node;
                let mut argumentos = None;
                let mut exige_const = false;
                if cx.especie(alvo) == "Annotation" {
                    if produtor == CriarMixin {
                        return None;
                    }
                    let nome = *cx.filhos(alvo).first()?;
                    argumentos = cx.filhos_da_especie(alvo, "ArgumentList").first().copied();
                    if cx.resolvido(nome).is_some() || argumentos.is_none() {
                        return None;
                    }
                    alvo = nome;
                    exige_const = true;
                }
                let (nome, prefixo): (String, Option<usize>) = match cx.especie(alvo) {
                    "NamedType" => {
                        let pr = cx.filhos_da_especie(alvo, "ImportPrefixReference").first().copied();
                        if let Some(p) = pr {
                            let s = cx.texto_do_no(p).trim_end_matches('.').trim();
                            let simbolo = self.consulta.nomes.lookup(s)?;
                            if !crate::projeto::eh_prefixo(prog, prog.unit(cx.unidade).library, simbolo) {
                                return None;
                            }
                        }
                        let inicio = pr.map_or(cx.arvore.nos[alvo].inicio, |p| cx.arvore.nos[p].fim);
                        let s = crate::projeto::palavra(cx.fonte, cx.token_seguinte(inicio)?.start)?;
                        exige_const |= exige_const_construtor(cx, alvo);
                        (cx.fonte[s.start..s.end].to_string(), pr)
                    }
                    "SimpleIdentifier" => {
                        if produtor == CriarMixin {
                            // Não o identificador de `a.b` nem a propriedade.
                            if alvo_qualificado(cx, alvo).is_some() {
                                return None;
                            }
                            (cx.texto_do_no(alvo).to_string(), None)
                        } else {
                            exige_const |= exige_const_construtor(cx, alvo);
                            (nome_de_tipo(cx, alvo)?, None)
                        }
                    }
                    "PrefixedIdentifier" => {
                        if produtor == CriarMixin && cx.pai(alvo).is_some_and(|p| cx.especie(p) == "InstanceCreationExpression") {
                            return None;
                        }
                        let f = cx.filhos(alvo);
                        let p = *f.first()?;
                        if !matches!(cx.resolvido(p), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) {
                            return None;
                        }
                        let id = *f.get(1)?;
                        let n = if produtor == CriarMixin { cx.texto_do_no(id).to_string() } else { nome_de_tipo(cx, id)? };
                        (n, Some(p))
                    }
                    _ => return None,
                };
                // Onde.
                let (unidade, offset, antes, depois) = match prefixo {
                    None => {
                        let membro = membro_de_topo(cx, alvo)?;
                        (cx.unidade, cx.arvore.nos[membro].fim, format!("{eol}{eol}"), String::new())
                    }
                    Some(p) => {
                        let s = cx.texto_do_no(p).trim_end_matches('.').trim().to_string();
                        let lib = prog.unit(cx.unidade).library;
                        let imp = prog.library(lib).imports.iter().find(|i| i.prefix.is_some_and(|x| self.nome(x) == s))?;
                        let &u = prog.library(imp.library).units.first()?;
                        let fonte = &prog.unit(u).source;
                        let eol_alvo = Texto::novo(fonte).eol();
                        (u, fonte.len(), eol_alvo.to_string(), eol_alvo.to_string())
                    }
                };
                let eol_d = Texto::novo(&prog.unit(unidade).source).eol().to_string();
                let texto = if produtor == CriarMixin {
                    format!("{antes}mixin {nome} {{{eol_d}}}{depois}")
                } else if argumentos.is_none() && !exige_const {
                    format!("{antes}class {nome} {{{eol_d}}}{depois}")
                } else {
                    let destino = Contexto::novo(self, unidade);
                    let mut escritor = Escritor::novo(&destino, offset);
                    let params = argumentos.map(|a| cx.parametros_para_argumentos(a, &mut escritor)).unwrap_or_default();
                    let c = if exige_const { "const " } else { "" };
                    let t = format!("{antes}class {nome} {{{eol_d}  {c}{nome}({params});{eol_d}}}{depois}");
                    return Some(Proposta {
                        titulo: format!("Create class '{nome}'"),
                        especie: "quickfix.create.class",
                        unidade,
                        edicoes: vec![(Span { start: offset, end: offset }, t)],
                        importar: escritor.importar,
                    });
                };
                let (titulo, especie) = if produtor == CriarMixin {
                    (format!("Create mixin '{nome}'"), "quickfix.create.mixin")
                } else {
                    (format!("Create class '{nome}'"), "quickfix.create.class")
                };
                Some(Proposta { titulo, especie, unidade, edicoes: vec![(Span { start: offset, end: offset }, texto)], importar: BTreeSet::new() })
            }
            CriarCampo | CriarGetter | CriarSetter => self.criar_campo_getter_setter(cx, produtor, node),
            CriarLocal => {
                if cx.especie(node) != "SimpleIdentifier" {
                    return None;
                }
                let nome = cx.texto_do_no(node).to_string();
                if let Some(a) = cx.pai(node)
                    && cx.especie(a) == "AssignmentExpression"
                    && cx.filhos(a).first() == Some(&node)
                    && operador_de_atribuicao(cx, a) == "="
                    && cx.pai(a).is_some_and(|s| cx.especie(s) == "ExpressionStatement")
                {
                    let o = cx.arvore.nos[node].inicio;
                    return Some(Proposta { titulo: format!("Create local variable '{nome}'"), especie: "quickfix.create.localVariable", unidade: cx.unidade, edicoes: vec![(Span { start: o, end: o }, "var ".into())], importar: BTreeSet::new() });
                }
                if alvo_qualificado(cx, node).is_some() || cx.pai(node).is_some_and(|p| matches!(cx.especie(p), "PrefixedIdentifier" | "PropertyAccess")) {
                    return None;
                }
                let comando = (0..).scan(Some(node), |a, _| {
                    let k = (*a)?;
                    *a = cx.pai(k);
                    Some(k)
                })
                .find(|&k| cx.e_comando(k))?;
                let prefixo = cx.prefixo_do_no(comando);
                let tipo = cx.tipo_inferido_indefinido(node);
                if let Some(t) = tipo
                    && !matches!(self.consulta.tabela.get(t), Type::Interface { .. } | Type::Function { .. } | Type::Record { .. } | Type::FutureOr { .. } | Type::ExtensionType { .. })
                {
                    return None;
                }
                let o = cx.arvore.nos[comando].inicio;
                let mut escritor = Escritor::novo(cx, o);
                let declaracao = match tipo.and_then(|t| escritor.escrever(t, false)) {
                    Some(t) => format!("{t} {nome};"),
                    None if tipo.is_some() => format!(" {nome};"),
                    None => format!("var {nome};"),
                };
                Some(Proposta {
                    titulo: format!("Create local variable '{nome}'"),
                    especie: "quickfix.create.localVariable",
                    unidade: cx.unidade,
                    edicoes: vec![(Span { start: o, end: o }, format!("{declaracao}{eol}{prefixo}"))],
                    importar: escritor.importar,
                })
            }
            CriarParametro => self.criar_parametro(cx, node),
            CriarMetodoOuFuncao => self.criar_metodo_ou_funcao(cx, node),
            CriarFuncao => {
                if cx.especie(node) != "SimpleIdentifier" {
                    return None;
                }
                let inv = cx.pai(node).filter(|&p| cx.especie(p) == "MethodInvocation")?;
                if cx.nome_do_metodo(inv) != Some(node) || cx.filhos(inv).first() != Some(&node) {
                    return None;
                }
                let membro = membro_mais_interno(cx, node)?;
                let o = cx.arvore.nos[membro].fim;
                let nome = cx.texto_do_no(node).to_string();
                let mut escritor = Escritor::novo(cx, o);
                let ret = cx.tipo_inferido_indefinido(inv).and_then(|t| escritor.escrever_tipo(Some(t), false));
                let lista = cx.filhos_da_especie(inv, "ArgumentList").first().copied()?;
                let params = cx.parametros_para_argumentos(lista, &mut escritor);
                let r = ret.map(|t| format!("{t} ")).unwrap_or_default();
                let texto = format!("{eol}{eol}{r}{nome}({params}) {{{eol}}}");
                Some(Proposta { titulo: format!("Create function '{nome}'"), especie: "quickfix.create.function", unidade: cx.unidade, edicoes: vec![(Span { start: o, end: o }, texto)], importar: escritor.importar })
            }
            CriarMetodo => self.criar_metodo(cx, node),
            CriarGetterDeExtensao | CriarMetodoDeExtensao | CriarSetterDeExtensao => self.criar_membro_de_extensao(cx, produtor, node),
        }
    }

    /// `CreateField`/`CreateGetter`/`CreateSetter` (`create_field.dart`,
    /// `create_getter.dart`, `create_setter.dart`).
    fn criar_campo_getter_setter(&self, cx: &Contexto<'_>, produtor: Produtor, node: usize) -> Option<Proposta> {
        let prog = self.programa();
        // `this.x` sem campo (só o `CreateField`).
        if produtor == Produtor::CriarCampo
            && let Some(param) = cx.este_ou_ancestral_pub(node, "FieldFormalParameter")
        {
            let construtor = cx.este_ou_ancestral_pub(param, "ConstructorDeclaration")?;
            let conteiner = cx.pai(construtor)?;
            if !matches!(cx.especie(conteiner), "ClassDeclaration" | "EnumDeclaration") {
                return None;
            }
            let nome = crate::projeto::palavra(cx.fonte, cx.arvore.nos[param].fim.checked_sub(1)?)?;
            let nome = cx.fonte[nome.start..nome.end].to_string();
            let constante = cx.tokens_iniciais(construtor).iter().any(|s| s == "const");
            let o = cx.inserir_no_membro(conteiner, Filtro::Campo, "")?.0;
            let mut escritor = Escritor::novo(cx, o);
            let tipo = cx.ast.members.iter().find_map(|m| match &m.kind {
                ast::MemberKind::Constructor(k) => k.parameters.iter().find(|q| q.this_ && q.span.start >= cx.arvore.nos[param].inicio && q.span.end <= cx.arvore.nos[param].fim),
                _ => None,
            });
            let tipo = tipo.and_then(|q| q.ty).and_then(|t| self.consulta.outline.tipos_escritos.get(&(cx.unidade, t)).copied());
            let decl = declaracao_de_campo(&mut escritor, &nome, constante, false, tipo);
            let (o, texto) = cx.inserir_no_membro(conteiner, Filtro::Campo, &decl)?;
            return Some(Proposta { titulo: format!("Create field '{nome}'"), especie: "quickfix.create.field", unidade: cx.unidade, edicoes: vec![(Span { start: o, end: o }, texto)], importar: escritor.importar });
        }
        if cx.especie(node) != "SimpleIdentifier" {
            return None;
        }
        let nome = cx.texto_do_no(node).to_string();
        if produtor == Produtor::CriarGetter && !em_contexto_de_leitura(cx, node) {
            return None;
        }
        if produtor == Produtor::CriarSetter && !em_contexto_de_escrita(cx, node) {
            return None;
        }
        let alvo = alvo_qualificado(cx, node);
        // O destino: classe ou extensão, e o `static`.
        enum Destino {
            Classe(ClassId),
            Extensao(ExtensionId),
        }
        let (destino, estatico) = match alvo {
            Some(a) if produtor != Produtor::CriarCampo && cx.e_override_de_extensao(a) => (Destino::Extensao(cx.extensao_do_alvo(a)?), false),
            Some(a) if produtor != Produtor::CriarCampo && cx.extensao_do_alvo(a).is_some() => (Destino::Extensao(cx.extensao_do_alvo(a)?), true),
            Some(a) => {
                let c = if produtor == Produtor::CriarCampo {
                    let c = cx.interface_do_alvo(a);
                    if cx.especie(a) == "SimpleIdentifier" && cx.resolvido(a).is_none() {
                        return None;
                    }
                    c?
                } else if let Some(k) = cx.classe_nomeada(a) {
                    k
                } else {
                    match self.consulta.tabela.get(cx.tipo_do_no(a)?) {
                        Type::Interface { class, .. } => *class,
                        _ => return None,
                    }
                };
                let estatico = cx.classe_nomeada(a).is_some_and(|k| prog.class(k).kind == ClassKind::Class);
                (Destino::Classe(c), estatico)
            }
            None => {
                let d = match cx.interface_envolvente(node) {
                    Some((c, _)) => Destino::Classe(c),
                    None => {
                        if produtor == Produtor::CriarCampo {
                            // Também enum (`enclosingInterfaceElement2` só dá
                            // classe e mixin).
                            return None;
                        }
                        Destino::Extensao(cx.extensao_envolvente(node)?.0)
                    }
                };
                (d, cx.em_contexto_estatico(node))
            }
        };
        let tipo_no = subir_acesso(cx, node);
        if produtor == Produtor::CriarCampo
            && let Destino::Classe(c) = &destino
            && prog.class(*c).kind == ClassKind::Enum
            && cx.pai(tipo_no).is_some_and(|p| cx.especie(p) == "AssignmentExpression" && cx.filhos(p).first() == Some(&tipo_no))
        {
            return None;
        }
        let tipo = cx.tipo_inferido_indefinido(tipo_no);
        // A declaração de destino (fora do SDK), na unidade dela.
        let (lib, decl) = match &destino {
            Destino::Classe(c) => (prog.class(*c).library, prog.class(*c).decl?),
            Destino::Extensao(x) => (prog.extension(*x).library, prog.extension(*x).decl),
        };
        if prog.library(lib).is_sdk {
            return None;
        }
        let (dcx, conteiner) = declaracao_na_unidade(self, decl.unit, decl.decl)?;
        let permitido = match produtor {
            Produtor::CriarCampo => matches!(dcx.especie(conteiner), "ClassDeclaration" | "EnumDeclaration" | "MixinDeclaration"),
            _ => matches!(dcx.especie(conteiner), "ClassDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration"),
        };
        if !permitido {
            return None;
        }
        let filtro = if produtor == Produtor::CriarCampo { Filtro::Campo } else { Filtro::Getter };
        let o = dcx.inserir_no_membro(conteiner, filtro, "")?.0;
        let mut escritor = Escritor::novo(&dcx, o);
        let s = if estatico { "static " } else { "" };
        let (decl, titulo, especie) = match produtor {
            Produtor::CriarCampo => {
                let final_ = dcx.especie(conteiner) == "EnumDeclaration";
                (format!("{s}{}", declaracao_de_campo(&mut escritor, &nome, false, final_, tipo)), format!("Create field '{nome}'"), "quickfix.create.field")
            }
            Produtor::CriarGetter => {
                let t = tipo.filter(|t| !matches!(self.consulta.tabela.get(*t), Type::Dynamic)).and_then(|t| escritor.escrever(t, false));
                let t = t.map(|x| format!("{x} ")).unwrap_or_default();
                (format!("{s}{t}get {nome} => null;"), format!("Create getter '{nome}'"), "quickfix.create.getter")
            }
            _ => {
                let t = tipo.filter(|t| !matches!(self.consulta.tabela.get(*t), Type::Dynamic)).and_then(|t| escritor.escrever(t, false));
                let t = t.map(|x| format!("{x} ")).unwrap_or_default();
                (format!("{s}set {nome}({t}{nome}) {{}}"), format!("Create setter '{nome}'"), "quickfix.create.setter")
            }
        };
        let (o, texto) = dcx.inserir_no_membro(conteiner, filtro, &decl)?;
        Some(Proposta { titulo, especie, unidade: dcx.unidade, edicoes: vec![(Span { start: o, end: o }, texto)], importar: escritor.importar })
    }

    /// `CreateParameter` (`create_parameter.dart`).
    fn criar_parametro(&self, cx: &Contexto<'_>, node: usize) -> Option<Proposta> {
        if cx.especie(node) != "SimpleIdentifier" {
            return None;
        }
        let nome = cx.texto_do_no(node).to_string();
        // A lista: a da `FunctionExpression` mais interna, senão do método,
        // senão do construtor.
        let dono = ["FunctionExpression", "MethodDeclaration", "ConstructorDeclaration"].iter().find_map(|e| cx.este_ou_ancestral_pub(node, e))?;
        let lista = cx.filhos_da_especie(dono, "FormalParameterList").first().copied()?;
        let tipo = cx.tipo_inferido_indefinido(node);
        let o0 = cx.arvore.nos[lista].inicio;
        let mut escritor = Escritor::novo(cx, o0);
        let t = escritor.escrever(tipo.unwrap_or(self.consulta.core.dynamic_), true).unwrap_or_else(|| "dynamic".into());
        let parametros: Vec<usize> = cx.filhos(lista).to_vec();
        let obrigatorio = |p: usize| {
            let s = cx.arvore.span(p);
            cx.ast
                .functions
                .iter()
                .flat_map(|f| f.parameters.iter().flatten())
                .chain(cx.ast.members.iter().flat_map(|m| match &m.kind {
                    ast::MemberKind::Constructor(k) => k.parameters.iter().collect::<Vec<_>>(),
                    _ => Vec::new(),
                }))
                .find(|q| q.span.start >= s.start && q.span.end <= s.end)
                .map(|q| q.kind)
        };
        let r: Vec<usize> = parametros.iter().copied().filter(|&p| obrigatorio(p) == Some(ast::ParameterKind::Required)).collect();
        let n: Vec<usize> = parametros.iter().copied().filter(|&p| obrigatorio(p) == Some(ast::ParameterKind::Named)).collect();
        let ultimo = r.last().or(n.last()).copied();
        let virgula_final = parametros.last().and_then(|&p| cx.token_seguinte(cx.arvore.nos[p].fim)).is_some_and(|t| &cx.fonte[t.start..t.end] == ",");
        let ha_seguinte = (!r.is_empty() && parametros.iter().any(|&p| obrigatorio(p) != Some(ast::ParameterKind::Required)))
            || (r.is_empty() && parametros.iter().any(|&p| obrigatorio(p) != Some(ast::ParameterKind::Named)));
        let requerido = r.is_empty() && ultimo.is_some() && !tipo.is_some_and(|x| self.consulta.tabela.get(x).is_declared_nullable());
        let req = if requerido { "required " } else { "" };
        let tx = Texto::novo(cx.fonte);
        let (offset, texto) = match ultimo {
            None => {
                let mut s = format!("{t} {nome}");
                if virgula_final {
                    s.push(',');
                } else if ha_seguinte {
                    s.push_str(", ");
                }
                (o0 + 1, s)
            }
            Some(u) if virgula_final => {
                let v = cx.token_seguinte(cx.arvore.nos[u].fim)?;
                (v.end, format!("{}{}{req}{t} {nome},", tx.eol(), tx.prefixo_da_linha(cx.arvore.nos[u].inicio)))
            }
            Some(u) if ha_seguinte => {
                let v = cx.token_seguinte(cx.arvore.nos[u].fim)?;
                (v.end + 1, format!("{t} {nome}, "))
            }
            Some(u) => (cx.arvore.nos[u].fim, format!(", {req}{t} {nome}")),
        };
        Some(Proposta {
            titulo: format!("Create required positional parameter '{nome}'"),
            especie: "quickfix.create.parameter",
            unidade: cx.unidade,
            edicoes: vec![(Span { start: offset, end: offset }, texto)],
            importar: escritor.importar,
        })
    }

    /// `CreateMethodOrFunction` (`create_method_or_function.dart`).
    fn criar_metodo_ou_funcao(&self, cx: &Contexto<'_>, node: usize) -> Option<Proposta> {
        if cx.especie(node) != "SimpleIdentifier" {
            return None;
        }
        let prog = self.programa();
        let nome = cx.texto_do_no(node).to_string();
        let (classe, mut argumento) = match alvo_qualificado(cx, node) {
            Some(a) => match self.consulta.tabela.get(cx.tipo_do_no(a)?) {
                Type::Interface { class, .. } => (Some(*class), cx.pai(node)?),
                _ => return None,
            },
            None => (cx.interface_envolvente(node).map(|(c, _)| c), node),
        };
        if let Some(p) = cx.pai(argumento)
            && cx.especie(p) == "NamedExpression"
        {
            argumento = p;
        }
        let mut parametro = cx.parametro_correspondente(argumento);
        if let Some(p) = cx.pai(argumento)
            && cx.especie(p) == "ConditionalExpression"
        {
            if cx.filhos(p).first() == Some(&argumento) {
                return None;
            }
            parametro = cx.parametro_correspondente(p);
        }
        let tipo_f = parametro?;
        // `Function` vale `dynamic Function()`.
        let (ret, positional, optional, named): (TypeId, Vec<TypeId>, Vec<TypeId>, Vec<(dartforge_intern::SymbolId, TypeId, bool)>) = match self.consulta.tabela.get(tipo_f).clone() {
            Type::Interface { class, .. } if Some(class) == self.consulta.core.function_class => (self.consulta.core.dynamic_, Vec::new(), Vec::new(), Vec::new()),
            Type::Function { ret, positional, optional, named, .. } => (ret, positional.to_vec(), optional.to_vec(), named.to_vec()),
            _ => return None,
        };
        let escrever = |escritor: &mut Escritor<'_, '_>| -> String {
            let mut s = String::new();
            if let Some(r) = escritor.escrever(ret, false) {
                s.push_str(&r);
                s.push(' ');
            }
            s.push_str(&nome);
            // `writeFormalParameters`: os posicionais sem nome viram `p1`…
            s.push('(');
            let mut i = 0usize;
            let mut usados: BTreeSet<String> = named.iter().map(|(n, _, _)| self.nome(*n).to_string()).collect();
            let gerar = |usados: &mut BTreeSet<String>| {
                let mut k = 1;
                while usados.contains(&format!("p{k}")) {
                    k += 1;
                }
                let n = format!("p{k}");
                usados.insert(n.clone());
                n
            };
            let mut abriu_o = false;
            let mut abriu_n = false;
            for (q, opcional) in positional.iter().map(|q| (*q, false)).chain(optional.iter().map(|q| (*q, true))) {
                if i > 0 {
                    s.push_str(", ");
                }
                if opcional && !abriu_o {
                    s.push('[');
                    abriu_o = true;
                }
                let n = gerar(&mut usados);
                if let Some(t) = escritor.escrever(q, false) {
                    s.push_str(&t);
                    s.push(' ');
                }
                s.push_str(&n);
                i += 1;
            }
            for (n, q, requerido) in named.iter() {
                if i > 0 {
                    s.push_str(", ");
                }
                if !abriu_n {
                    s.push('{');
                    abriu_n = true;
                }
                if *requerido {
                    s.push_str("required ");
                }
                if let Some(t) = escritor.escrever(*q, false) {
                    s.push_str(&t);
                    s.push(' ');
                }
                s.push_str(self.nome(*n));
                i += 1;
            }
            if abriu_n {
                s.push('}');
            }
            if abriu_o {
                s.push(']');
            }
            s.push(')');
            s
        };
        match classe {
            Some(c) if matches!(prog.class(c).kind, ClassKind::Class | ClassKind::Mixin) => {
                let decl = prog.class(c).decl?;
                let (dcx, conteiner) = declaracao_na_unidade(self, decl.unit, decl.decl)?;
                let o = dcx.arvore.nos[conteiner].fim - 1;
                let eol = Texto::novo(dcx.fonte).eol();
                let mut escritor = Escritor::novo(&dcx, o);
                let antes = if dcx.membros_do_conteiner(conteiner).is_empty() { "" } else { eol };
                let s = if cx.em_contexto_estatico(node) { "static " } else { "" };
                let corpo = escrever(&mut escritor);
                let texto = format!("{antes}  {s}{corpo} {{{eol}  }}{eol}");
                Some(Proposta { titulo: format!("Create method '{nome}'"), especie: "quickfix.create.method", unidade: dcx.unidade, edicoes: vec![(Span { start: o, end: o }, texto)], importar: escritor.importar })
            }
            Some(_) => None,
            None => {
                let o = cx.fonte.len();
                let eol = Texto::novo(cx.fonte).eol();
                let mut escritor = Escritor::novo(cx, o);
                let corpo = escrever(&mut escritor);
                let texto = format!("{eol}{corpo} {{{eol}}}{eol}");
                Some(Proposta { titulo: format!("Create function '{nome}'"), especie: "quickfix.create.function", unidade: cx.unidade, edicoes: vec![(Span { start: o, end: o }, texto)], importar: escritor.importar })
            }
        }
    }

    /// `CreateMethod.method` (`create_method.dart:92-205`).
    fn criar_metodo(&self, cx: &Contexto<'_>, node: usize) -> Option<Proposta> {
        let prog = self.programa();
        if cx.especie(node) != "SimpleIdentifier" {
            return None;
        }
        let inv = cx.pai(node).filter(|&p| cx.especie(p) == "MethodInvocation")?;
        let nome = cx.texto_do_no(node).to_string();
        let alvo = cx.filhos(inv).first().copied().filter(|&a| a != node);
        let (unidade, decl, estatico): (UnitId, Option<ast::DeclId>, bool) = match alvo {
            Some(a) if cx.e_override_de_extensao(a) => {
                let x = prog.extension(cx.extensao_do_alvo(a)?).decl;
                (x.unit, Some(x.decl), false)
            }
            Some(a) if cx.extensao_do_alvo(a).is_some() => {
                let x = prog.extension(cx.extensao_do_alvo(a)?).decl;
                (x.unit, Some(x.decl), true)
            }
            None => {
                let membro = (0..)
                    .scan(Some(node), |s, _| {
                        let k = (*s)?;
                        *s = cx.pai(k);
                        Some(k)
                    })
                    .find(|&k| matches!(cx.especie(k), "MethodDeclaration" | "FieldDeclaration" | "ConstructorDeclaration"))?;
                let conteiner = cx.pai(membro)?;
                let Marca::Decl(d) = cx.arvore.nos[conteiner].marca else { return None };
                (cx.unidade, Some(d), cx.em_contexto_estatico(node))
            }
            Some(a) => {
                let c = cx.interface_do_alvo(a)?;
                let k = prog.class(c);
                if prog.library(k.library).is_sdk {
                    return None;
                }
                if !matches!(k.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::ExtensionType | ClassKind::MixinApplication) {
                    return None;
                }
                let d = k.decl?;
                let estatico = cx.classe_nomeada(a).is_some();
                (d.unit, Some(d.decl), estatico)
            }
        };
        let (dcx, conteiner) = declaracao_na_unidade(self, unidade, decl?)?;
        let o = dcx.inserir_no_membro(conteiner, Filtro::Metodo, "")?.0;
        let mut escritor = Escritor::novo(&dcx, o);
        let s = if estatico { "static " } else { "" };
        let ret = cx.tipo_inferido_indefinido(inv).and_then(|t| escritor.escrever_tipo(Some(t), false)).map(|t| format!("{t} ")).unwrap_or_default();
        let lista = cx.filhos_da_especie(inv, "ArgumentList").first().copied()?;
        let params = cx.parametros_para_argumentos(lista, &mut escritor);
        let (o, texto) = dcx.inserir_no_membro(conteiner, Filtro::Metodo, &format!("{s}{ret}{nome}({params}) {{}}"))?;
        Some(Proposta { titulo: format!("Create method '{nome}'"), especie: "quickfix.create.method", unidade: dcx.unidade, edicoes: vec![(Span { start: o, end: o }, texto)], importar: escritor.importar })
    }

    /// `CreateExtensionGetter`/`Method`/`Setter`
    /// (`create_extension_member.dart`).
    fn criar_membro_de_extensao(&self, cx: &Contexto<'_>, produtor: Produtor, node: usize) -> Option<Proposta> {
        if cx.especie(node) != "SimpleIdentifier" {
            return None;
        }
        let nome = cx.texto_do_no(node).to_string();
        let (alvo, inv) = match produtor {
            Produtor::CriarMetodoDeExtensao => {
                let inv = cx.pai(node).filter(|&p| cx.especie(p) == "MethodInvocation")?;
                if cx.nome_do_metodo(inv) != Some(node) {
                    return None;
                }
                (cx.filhos(inv).first().copied().filter(|&a| a != node)?, Some(inv))
            }
            Produtor::CriarGetterDeExtensao => {
                if !em_contexto_de_leitura(cx, node) {
                    return None;
                }
                (alvo_qualificado(cx, node)?, None)
            }
            _ => {
                if !em_contexto_de_escrita(cx, node) {
                    return None;
                }
                (alvo_qualificado(cx, node)?, None)
            }
        };
        let tipo_alvo = cx.tipo_do_no(alvo)?;
        if matches!(self.consulta.tabela.get(tipo_alvo), Type::Dynamic) || matches!(self.consulta.tabela.exibicao(tipo_alvo), Some(dartforge_types::table::Exibicao::Invalido)) {
            return None;
        }
        let eol = Texto::novo(cx.fonte).eol();
        // A extensão existente aplicável (a primeira da unidade).
        let existente = self.extensao_aplicavel(cx, tipo_alvo);
        let membro = |escritor: &mut Escritor<'_, '_>| -> String {
            match produtor {
                Produtor::CriarGetterDeExtensao => {
                    let t = cx.tipo_inferido_indefinido(subir_acesso(cx, node)).and_then(|t| escritor.escrever_tipo(Some(t), false)).map(|t| format!("{t} ")).unwrap_or_default();
                    format!("{t}get {nome} => null;")
                }
                Produtor::CriarSetterDeExtensao => {
                    let t = cx.tipo_inferido_indefinido(subir_acesso(cx, node)).and_then(|t| escritor.escrever_tipo(Some(t), false)).map(|t| format!("{t} ")).unwrap_or_default();
                    format!("set {nome}({t}{nome}) {{}}")
                }
                _ => {
                    let inv = inv.unwrap_or(node);
                    let t = cx.tipo_inferido_indefinido(inv).and_then(|t| escritor.escrever_tipo(Some(t), false)).map(|t| format!("{t} ")).unwrap_or_default();
                    let lista = cx.filhos_da_especie(inv, "ArgumentList").first().copied();
                    let params = lista.map(|l| cx.parametros_para_argumentos(l, escritor)).unwrap_or_default();
                    format!("{t}{nome}({params}) {{}}")
                }
            }
        };
        let (titulo, especie) = match produtor {
            Produtor::CriarGetterDeExtensao => (format!("Create extension getter '{nome}'"), "quickfix.create.extension.getter"),
            Produtor::CriarSetterDeExtensao => (format!("Create extension setter '{nome}'"), "quickfix.create.extension.setter"),
            _ => (format!("Create extension method '{nome}'"), "quickfix.create.extension.method"),
        };
        let (o, texto, importar) = match existente {
            Some(ext) => {
                let filtro = if produtor == Produtor::CriarMetodoDeExtensao { Filtro::Metodo } else { Filtro::Getter };
                let o0 = cx.inserir_no_membro(ext, filtro, "")?.0;
                let mut escritor = Escritor::novo(cx, o0);
                let m = membro(&mut escritor);
                let (o, t) = cx.inserir_no_membro(ext, filtro, &m)?;
                (o, t, escritor.importar)
            }
            None => {
                let topo = membro_de_topo(cx, node)?;
                let o = cx.arvore.nos[topo].fim;
                let mut escritor = Escritor::novo(cx, o);
                let tipo = escritor.escrever(tipo_alvo, true).unwrap_or_else(|| "dynamic".into());
                let m = membro(&mut escritor);
                (o, format!("{eol}{eol}extension on {tipo} {{{eol}{UM_RECUO}{m}{eol}}}"), escritor.importar)
            }
        };
        Some(Proposta { titulo, especie, unidade: cx.unidade, edicoes: vec![(Span { start: o, end: o }, texto)], importar })
    }

    /// `_existingExtension`: a primeira `ExtensionDeclaration` da unidade que
    /// se aplica ao tipo.
    fn extensao_aplicavel(&self, cx: &Contexto<'_>, tipo: TypeId) -> Option<usize> {
        let prog = self.programa();
        for (k, no) in cx.arvore.nos.iter().enumerate() {
            if no.especie != "ExtensionDeclaration" || cx.pai(k) != Some(0) {
                continue;
            }
            let Marca::Decl(d) = no.marca else { continue };
            let Some(x) = (0..prog.extensions.len()).map(|i| ExtensionId(i as u32)).find(|&x| prog.extension(x).decl.unit == cx.unidade && prog.extension(x).decl.decl == d) else { continue };
            if extensao_se_aplica(self, x, tipo) {
                return Some(k);
            }
        }
        None
    }

    /// As propostas de um código para o nó, como ações.
    fn acoes_do_codigo(&self, cx: &Contexto<'_>, codigo: Codigo, node: usize, d: Option<&Diagnostic>) -> Vec<AcaoDeCodigo> {
        let mut saida = Vec::new();
        for &pr in produtores(codigo) {
            let Some(p) = self.propor(cx, pr, node) else { continue };
            let Some(uri) = self.uri_da_unidade(p.unidade) else { continue };
            let mut m = Mudanca::default();
            for (s, t) in p.edicoes {
                m.adicionar(&uri, s, t);
            }
            if !p.importar.is_empty() {
                let destino = Contexto::novo(self, p.unidade);
                crate::refatoracoes_mover::imports_do_builder(&destino, &mut m, &p.importar);
            }
            if m.conflito.is_some() {
                continue;
            }
            let edicoes: Vec<Edicao> = m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect();
            saida.push(AcaoDeCodigo { titulo: p.titulo, especie: p.especie.into(), edicoes, diagnostico: d.cloned(), criar_arquivo: None });
        }
        saida
    }

    /// As correções de criação e troca dos códigos publicados (o `node` do
    /// diagnóstico).
    pub(crate) fn criar_para_diagnostico(&self, unidade: UnitId, d: &Diagnostic, codigo: Codigo) -> Vec<AcaoDeCodigo> {
        let cx = Contexto::novo(self, unidade);
        let Some(node) = cx.arvore.localizar(d.span.start, d.span.end) else { return Vec::new() };
        self.acoes_do_codigo(&cx, codigo, node, Some(d))
    }

    /// As correções de criação e troca para os nomes sem resolução das
    /// linhas `inicio..fim` de `unidade`, cada um com o código que o
    /// analyzer daria.
    pub(crate) fn criar_indefinidos(&mut self, _uri: &str, unidade: UnitId, inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
        let cx = Contexto::novo(self, unidade);
        let prog = self.programa();
        let lib = prog.unit(unidade).library;
        let mut usos: Vec<(Codigo, usize)> = Vec::new();
        for (n, no) in cx.arvore.nos.iter().enumerate() {
            if !(no.inicio <= fim && inicio <= no.fim) {
                continue;
            }
            match no.especie {
                "NamedType" => {
                    // Tipo escrito sem resolução: `UNDEFINED_CLASS`.
                    let Some(t) = cx.ast.types.iter().position(|t| t.span.start == no.inicio && t.span.end == no.fim) else { continue };
                    let ast::TypeKind::Named { name, .. } = &cx.ast.ty(ast::TypeId(t as u32)).kind else { continue };
                    let [nm] = &name[..] else { continue };
                    let texto = self.nome(nm.sym);
                    if prog.lookup(lib, nm.sym).is_some()
                        || ["dynamic", "Never", "void", "Function", "Record"].contains(&texto)
                        || crate::projeto::declaracao_de_parametro_de_tipo(cx.ast, nm.span.start, nm.sym).is_some()
                    {
                        continue;
                    }
                    usos.push((Codigo::Classe, n));
                }
                "SimpleIdentifier" => {
                    let Marca::Expr(x) = no.marca else { continue };
                    if cx.corpos.get_resolved(x).is_some() || cx.corpos.declaracao_local(x).is_some() {
                        continue;
                    }
                    let Some(pai) = cx.pai(n) else { continue };
                    let qualificado = alvo_qualificado(&cx, n);
                    let codigo = if cx.especie(pai) == "MethodInvocation" && cx.nome_do_metodo(pai) == Some(n) {
                        match cx.filhos(pai).first().copied().filter(|&a| a != n) {
                            None => {
                                if self.consulta.nomes.lookup(cx.texto_do_no(n)).is_some_and(|s| prog.lookup(lib, s).is_some()) {
                                    continue;
                                }
                                if cx.este_ou_ancestral_pub(n, "ClassDeclaration").is_some()
                                    || cx.este_ou_ancestral_pub(n, "MixinDeclaration").is_some()
                                    || cx.este_ou_ancestral_pub(n, "EnumDeclaration").is_some()
                                    || cx.este_ou_ancestral_pub(n, "ExtensionTypeDeclaration").is_some()
                                    || cx.este_ou_ancestral_pub(n, "ExtensionDeclaration").is_some()
                                {
                                    Codigo::Metodo
                                } else {
                                    Codigo::Funcao
                                }
                            }
                            Some(a) => {
                                if matches!(cx.resolvido(a), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) {
                                    Codigo::Funcao
                                } else if cx.extensao_do_alvo(a).is_some() {
                                    Codigo::MetodoDeExtensao
                                } else if cx.tipo_do_no(a).is_some_and(|t| matches!(self.consulta.tabela.get(t), Type::Dynamic)) {
                                    continue;
                                } else {
                                    Codigo::Metodo
                                }
                            }
                        }
                    } else if let Some(a) = qualificado {
                        if matches!(cx.resolvido(a), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) {
                            // `UNDEFINED_PREFIXED_NAME`: só o `DataDriven`.
                            continue;
                        }
                        if cx.tipo_do_no(a).is_some_and(|t| matches!(self.consulta.tabela.get(t), Type::Dynamic)) && cx.classe_nomeada(a).is_none() {
                            continue;
                        }
                        let ext = cx.extensao_do_alvo(a).is_some();
                        match (em_contexto_de_escrita(&cx, n), ext) {
                            (true, true) => Codigo::SetterDeExtensao,
                            (true, false) => Codigo::Setter,
                            (false, true) => Codigo::GetterDeExtensao,
                            (false, false) => Codigo::Getter,
                        }
                    } else {
                        let simbolo = self.consulta.nomes.lookup(cx.texto_do_no(n));
                        if simbolo.is_some_and(|s| prog.lookup(lib, s).is_some()) {
                            continue;
                        }
                        Codigo::Identificador
                    };
                    usos.push((codigo, n));
                }
                _ => {}
            }
        }
        let mut saida = Vec::new();
        for (codigo, n) in usos {
            saida.extend(self.acoes_do_codigo(&cx, codigo, n, None));
        }
        saida
    }
}

/// `writeFieldDeclaration(name, isFinal, isStatic, type)` (com `static`
/// posto pelo chamador).
fn declaracao_de_campo(escritor: &mut Escritor<'_, '_>, nome: &str, final_: bool, enum_final: bool, tipo: Option<TypeId>) -> String {
    let final_ = final_ || enum_final;
    let mut s = String::new();
    if final_ {
        s.push_str("final ");
    }
    match tipo {
        Some(t) => {
            let escrito = if final_ { escritor.escrever(t, false).or_else(|| Some("dynamic".into())) } else { escritor.escrever_tipo(Some(t), true) };
            if let Some(x) = escrito {
                s.push_str(&x);
                s.push(' ');
            }
        }
        None if !final_ => s.push_str("var "),
        None => {}
    }
    s.push_str(nome);
    s.push(';');
    s
}

/// `_requiresConstConstructor`.
fn exige_const_construtor(cx: &Contexto<'_>, n: usize) -> bool {
    let Some(pai) = cx.pai(n) else { return false };
    match (cx.especie(n), cx.especie(pai)) {
        ("SimpleIdentifier", "NamedType") => exige_const_construtor(cx, pai),
        ("SimpleIdentifier", "MethodInvocation") => em_contexto_constante(cx, pai),
        ("NamedType", "ConstructorName") => exige_const_construtor(cx, pai),
        ("ConstructorName", "InstanceCreationExpression") => cx.token_seguinte(cx.arvore.nos[pai].inicio).is_some_and(|t| &cx.fonte[t.start..t.end] == "const"),
        _ => false,
    }
}

/// `inConstantContext`: dentro de criação `const`, literal `const`,
/// inicializador de variável `const`, anotação ou padrão constante.
pub(crate) fn em_contexto_constante(cx: &Contexto<'_>, n: usize) -> bool {
    let mut atual = cx.pai(n);
    while let Some(k) = atual {
        match cx.especie(k) {
            "InstanceCreationExpression" | "ListLiteral" | "SetOrMapLiteral" | "RecordLiteral" => {
                if cx.token_seguinte(cx.arvore.nos[k].inicio).is_some_and(|t| &cx.fonte[t.start..t.end] == "const") {
                    return true;
                }
            }
            "VariableDeclarationList" => {
                if cx.tokens_iniciais_pub(k).iter().any(|s| s == "const") {
                    return true;
                }
            }
            "Annotation" | "ConstantPattern" | "SwitchCase" => return true,
            "FunctionBody" | "BlockFunctionBody" | "ExpressionFunctionBody" | "FunctionExpression" => return false,
            _ => {}
        }
        atual = cx.pai(k);
    }
    false
}

/// `extension.applicableTo(targetType, strictCasts: true)`: o receptor é
/// subtipo do tipo estendido com os parâmetros da extensão nos limites.
fn extensao_se_aplica(p: &Projeto, x: ExtensionId, recv: TypeId) -> bool {
    let consulta = &p.consulta;
    let Some(dados) = consulta.outline.extensions.get(x.0 as usize) else { return false };
    let mut tabela = consulta.tabela.clone();
    let on = if dados.type_params.is_empty() {
        dados.on
    } else {
        let mapa: std::collections::HashMap<dartforge_types::TypeParamId, TypeId> = dados.type_params.iter().map(|&tp| (tp, tabela.param(tp).bound)).collect();
        dartforge_types::substitute(dados.on, &mapa, &mut tabela)
    };
    let mut env = dartforge_types::SubtypeEnv::new(&mut tabela, &consulta.outline.hierarchy, &consulta.core);
    dartforge_types::is_subtype(recv, on, &mut env)
}
