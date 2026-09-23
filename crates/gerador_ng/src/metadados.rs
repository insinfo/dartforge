//! Os metadados de uma `@Directive`/`@Component`, lidos do programa
//! carregado — o que o `_ComponentVisitor` do oficial
//! (`source_gen/template_compiler/find_components.dart`) tira do analyzer:
//!
//! * a anotação avaliada como constante: `selector`, `exportAs`,
//!   `visibility`, `providers` (listas e constantes de topo achatadas, como o
//!   `ModuleReader.extractProviderObjects`);
//! * `@Input`/`@Output` da classe **e dos supertipos**
//!   (`_collectInheritableMetadata`: `allSupertypes` ao contrário, depois a
//!   própria classe; em cada uma os campos antes dos setters nas entradas, os
//!   acessores antes dos campos nas saídas);
//! * `@HostListener`/`@HostBinding` pela mesma ordem (`DirectiveVisitor`:
//!   acessores, métodos, campos);
//! * os ganchos de ciclo de vida (`extractLifecycleHooks`: qualquer supertipo
//!   que seja a interface do `lifecycle_hooks.dart`);
//! * as dependências do construtor (`_getCompileDiDependencyMetadata`), com
//!   `@Inject`, `@Optional`, `@Self`, `@Host`, `@SkipSelf`.
//!
//! As anotações são reconhecidas pela classe que designam
//! (`TypeChecker.fromUrl('package:ngdart/src/meta/...')`), não pelo nome
//! escrito. O que não se consegue ler vai para `Diretiva::fora`, e quem usa
//! a diretiva recusa.
use crate::componente::Ganchos;
use crate::diretivas::{
    DI_TOKENS, Dependencia, Diretiva, Entrada, Ouvinte, Provedor, TipoDeToken, Token,
};
use crate::resolucao::Resolvedor;
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, UnitId, VariableRef};
use dartforge_frontend::ast;

const DIRECTIVES: &str = "package:ngdart/src/meta/directives.dart";
const DI_ARGUMENTS: &str = "package:ngdart/src/meta/di_arguments.dart";
const DI_PROVIDERS: &str = "package:ngdart/src/meta/di_providers.dart";
const LIFECYCLE_HOOKS: &str = "package:ngdart/src/meta/lifecycle_hooks.dart";
const VISIBILITY: &str = "package:ngdart/src/meta/visibility.dart";

/// Profundidade máxima de constantes que citam constantes.
const PROFUNDIDADE: u32 = 16;

/// Uma classe já resolvida.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Classe {
    uri: String,
    nome: String,
    id: ClassId,
}

impl Classe {
    fn e(&self, uri: &str, nome: &str) -> bool {
        self.uri == uri && self.nome == nome
    }
}

/// Um tipo escrito, resolvido: a classe e os argumentos; `None` na classe é
/// `dynamic`.
#[derive(Debug, Clone)]
struct Tipo {
    classe: Option<Classe>,
    args: Vec<Tipo>,
}

/// O valor constante de uma expressão, na parte que os metadados usam.
#[derive(Debug, Clone)]
enum Valor {
    Texto(String),
    Nulo,
    Lista(Vec<Valor>),
    /// Um literal de tipo (`NgModel` numa lista de provedores).
    Tipo(Classe),
    /// `C<T>.ctor(a, b, nome: c)`.
    Objeto {
        classe: Classe,
        construtor: Option<String>,
        tipos: Vec<Tipo>,
        posicionais: Vec<Valor>,
        nomeados: Vec<(String, Valor)>,
    },
    /// `C.nome`: constante de enum ou campo estático.
    Membro { classe: Classe, nome: String },
}

/// Lê os metadados da classe `id`, se ela tem `@Directive` ou `@Component`.
pub fn ler(r: &Resolvedor, id: ClassId) -> Option<Diretiva> {
    Leitor { r }.diretiva(id)
}

struct Leitor<'r, 'a> {
    r: &'r Resolvedor<'a>,
}

/// Onde uma classe está declarada: a unidade, a biblioteca, os membros e os
/// nomes dos parâmetros de tipo.
struct Declaracao<'a> {
    unidade: UnitId,
    lib: LibraryId,
    ast: &'a ast::Ast,
    metadados: &'a [ast::Annotation],
    membros: &'a [ast::MemberId],
    parametros_de_tipo: Vec<String>,
    abstrata: bool,
}

impl<'r, 'a> Leitor<'r, 'a> {
    fn nome(&self, n: &ast::Name) -> &'a str {
        self.r.interner().resolve(n.sym)
    }

    fn declaracao(&self, id: ClassId) -> Option<Declaracao<'a>> {
        let p = self.r.programa();
        let c = p.class(id);
        let d = c.decl?;
        let u = p.unit(d.unit);
        let decl = u.ast.decl(d.decl);
        let (membros, tps, abstrata): (&[ast::MemberId], &[ast::TypeParameter], bool) =
            match &decl.kind {
                ast::DeclKind::Class(k) => (&k.members[..], &k.type_params[..], k.modifiers.abstract_),
                ast::DeclKind::Mixin(m) => (&m.members[..], &m.type_params[..], true),
                _ => return None,
            };
        Some(Declaracao {
            unidade: d.unit,
            lib: u.library,
            ast: &u.ast,
            metadados: &decl.metadata,
            membros,
            parametros_de_tipo: tps.iter().map(|t| self.nome(&t.name).to_string()).collect(),
            abstrata,
        })
    }

    fn classe(&self, id: ClassId) -> Classe {
        let p = self.r.programa();
        let c = p.class(id);
        Classe {
            uri: p.library(c.library).uri.clone(),
            nome: self.r.interner().resolve(c.name).to_string(),
            id,
        }
    }

    /// `allSupertypes` do analyzer (`ClassHierarchy._getHierarchy`): para o
    /// supertipo, as restrições `on`, as interfaces e os mixins, nessa ordem,
    /// o próprio tipo e depois os supertipos dele, sem repetir.
    fn supertipos(&self, id: ClassId, profundidade: u32) -> Vec<ClassId> {
        let mut saida: Vec<ClassId> = Vec::new();
        if profundidade > 64 {
            return saida;
        }
        let c = self.r.programa().class(id);
        let mut diretos: Vec<ClassId> = Vec::new();
        diretos.extend(c.supertype_class);
        if c.kind == ClassKind::Mixin {
            diretos.extend(c.on_classes.iter().copied());
        }
        diretos.extend(c.interface_classes.iter().copied());
        diretos.extend(c.mixin_classes.iter().copied());
        for t in diretos {
            if !saida.contains(&t) {
                saida.push(t);
            }
            for x in self.supertipos(t, profundidade + 1) {
                if !saida.contains(&x) {
                    saida.push(x);
                }
            }
        }
        saida
    }

    /// A classe que o nome de uma anotação designa (`Input`, `ng.Input`).
    fn classe_da_anotacao(&self, lib: LibraryId, a: &ast::Annotation) -> Option<Classe> {
        let partes: Vec<&str> = a.name.iter().map(|n| self.nome(n)).collect();
        let el = match partes.as_slice() {
            [n] => self.r.elemento_em(lib, None, n),
            [p, n] | [p, n, _] if self.r.e_prefixo(lib, p) => self.r.elemento_em(lib, Some(p), n),
            [n, _] => self.r.elemento_em(lib, None, n),
            _ => None,
        }?;
        match el {
            Element::Class(id) => Some(self.classe(id)),
            _ => None,
        }
    }

    /// Se a anotação é uma das do `directives.dart` do ngdart, o nome dela.
    /// Uma anotação com um desses nomes que não se resolve para lá é
    /// registrada em `fora`: o analyzer a teria resolvido, e perdê-la em
    /// silêncio daria metadados errados.
    fn anotacao_do_ngdart(
        &self,
        lib: LibraryId,
        a: &ast::Annotation,
        fora: &mut Vec<String>,
    ) -> Option<&'static str> {
        const NOMES: &[&str] = &[
            "Input",
            "Output",
            "HostListener",
            "HostBinding",
            "ContentChild",
            "ContentChildren",
            "ViewChild",
            "ViewChildren",
        ];
        let escrito = crate::nome_da_anotacao(a, self.r.interner());
        let conhecido = NOMES.iter().copied().find(|n| *n == escrito);
        match self.classe_da_anotacao(lib, a) {
            Some(c) if c.uri == DIRECTIVES => NOMES.iter().copied().find(|n| *n == c.nome),
            Some(_) => None,
            None => {
                if let Some(n) = conhecido {
                    fora.push(format!("@{n} não resolvida"));
                }
                None
            }
        }
    }

    fn diretiva(&self, id: ClassId) -> Option<Diretiva> {
        let decl = self.declaracao(id)?;
        let mut anotacao = None;
        for a in decl.metadados {
            if let Some(c) = self.classe_da_anotacao(decl.lib, a)
                && c.uri == DIRECTIVES
                && (c.nome == "Directive" || c.nome == "Component")
            {
                anotacao = Some((a, c.nome == "Component"));
                break;
            }
        }
        let (anotacao, e_componente) = anotacao?;
        let eu = self.classe(id);
        let mut d = Diretiva {
            classe: eu.nome.clone(),
            uri: eu.uri.clone(),
            e_componente,
            ..Default::default()
        };
        self.argumentos(&decl, anotacao, &mut d);
        // `_collectInheritableMetadata` e `DirectiveVisitor.visitDirective`.
        let mut ordem: Vec<ClassId> = self.supertipos(id, 0);
        ordem.reverse();
        ordem.push(id);
        let mut entradas: Vec<(String, Entrada)> = Vec::new();
        for &c in &ordem {
            if self.r.programa().library(self.r.programa().class(c).library).is_sdk {
                continue;
            }
            let Some(dc) = self.declaracao(c) else {
                continue;
            };
            self.membros(&dc, &mut d, &mut entradas);
        }
        d.entradas = entradas.into_iter().map(|(_, e)| e).collect();
        d.ganchos = self.ganchos(&ordem);
        self.dependencias(&decl, &mut d);
        Some(d)
    }

    /// Os argumentos da anotação.
    fn argumentos(&self, decl: &Declaracao, a: &ast::Annotation, d: &mut Diretiva) {
        let Some(args) = &a.arguments else {
            return;
        };
        for arg in args.args.iter() {
            let Some(nome) = arg.name.as_ref().map(|n| self.nome(n)) else {
                continue;
            };
            let valor = || self.valor(decl.unidade, arg.value, 0);
            match nome {
                "selector" => match valor() {
                    Ok(Valor::Texto(s)) => d.seletor = s,
                    _ => d.fora.push("selector: que não é texto constante".into()),
                },
                "exportAs" => match valor() {
                    Ok(Valor::Texto(s)) => d.export_as = Some(s),
                    Ok(Valor::Nulo) => {}
                    _ => d.fora.push("exportAs: que não é texto constante".into()),
                },
                "visibility" => match valor() {
                    Ok(Valor::Membro { classe, nome })
                        if classe.e(VISIBILITY, "Visibility") =>
                    {
                        d.visivel = nome == "all";
                    }
                    _ => d.fora.push("visibility: ilegível".into()),
                },
                "providers" => match valor() {
                    Ok(v) => self.provedores(v, d),
                    Err(f) => d.fora.push(format!("providers: {f}")),
                },
                "viewProviders" => match valor() {
                    Ok(Valor::Lista(l)) if l.is_empty() => {}
                    _ => d.fora.push("viewProviders:".into()),
                },
                _ => {}
            }
        }
    }

    /// `ModuleReader.extractProviderObjects`: listas achatadas; cada item um
    /// provedor. Só `ExistingProvider` tem caso no corpus.
    fn provedores(&self, v: Valor, d: &mut Diretiva) {
        match v {
            Valor::Lista(itens) => {
                for i in itens {
                    self.provedores(i, d);
                }
            }
            Valor::Nulo => {}
            Valor::Objeto {
                classe,
                construtor,
                posicionais,
                nomeados,
                ..
            } if classe.e(DI_PROVIDERS, "ExistingProvider")
                && matches!(construtor.as_deref(), None | Some("forToken")) =>
            {
                if !nomeados.is_empty() {
                    d.fora.push("ExistingProvider com argumento nomeado".into());
                    return;
                }
                let (Some(t), Some(e)) = (posicionais.first(), posicionais.get(1)) else {
                    d.fora.push("ExistingProvider sem argumentos".into());
                    return;
                };
                let token = match self.token_do_valor(t) {
                    Ok(t) => t,
                    Err(f) => {
                        d.fora.push(f);
                        return;
                    }
                };
                let existente = match e {
                    Valor::Tipo(c) => Token::Classe {
                        uri: c.uri.clone(),
                        classe: c.nome.clone(),
                    },
                    _ => {
                        d.fora.push("ExistingProvider de algo que não é classe".into());
                        return;
                    }
                };
                d.provedores.push(Provedor {
                    multi: matches!(token, Token::Multi { .. }),
                    token,
                    existente,
                });
            }
            Valor::Objeto { classe, .. } => {
                d.fora.push(format!("provedor {}", classe.nome));
            }
            Valor::Tipo(c) => d.fora.push(format!("provedor de classe ({})", c.nome)),
            _ => d.fora.push("provedor ilegível".into()),
        }
    }

    /// O token de um valor constante: uma classe ou um `MultiToken`.
    fn token_do_valor(&self, v: &Valor) -> Result<Token, String> {
        match v {
            Valor::Tipo(c) => Ok(Token::Classe {
                uri: c.uri.clone(),
                classe: c.nome.clone(),
            }),
            Valor::Objeto {
                classe,
                construtor: None,
                tipos,
                posicionais,
                ..
            } if classe.e(DI_TOKENS, "MultiToken") => {
                let nome = match posicionais.first() {
                    Some(Valor::Texto(s)) => s.clone(),
                    None => String::new(),
                    _ => return Err("MultiToken de nome ilegível".into()),
                };
                let [t] = tipos.as_slice() else {
                    return Err("MultiToken sem argumento de tipo".into());
                };
                let Some(c) = &t.classe else {
                    return Err("MultiToken<dynamic>".into());
                };
                let parametros = self.r.programa().class(c.id).type_params.len();
                // `fromDartType` escreve os argumentos; só `dynamic` tem caso.
                if t.args.len() != parametros || t.args.iter().any(|a| a.classe.is_some()) {
                    return Err("MultiToken de tipo com argumentos".into());
                }
                Ok(Token::Multi {
                    nome,
                    tipo: TipoDeToken {
                        uri: c.uri.clone(),
                        classe: c.nome.clone(),
                        genericos: parametros,
                    },
                })
            }
            Valor::Objeto { classe, .. } => Err(format!("token {}", classe.nome)),
            _ => Err("token ilegível".into()),
        }
    }

    /// Os membros anotados de uma classe da hierarquia.
    fn membros(&self, dc: &Declaracao, d: &mut Diretiva, entradas: &mut Vec<(String, Entrada)>) {
        let mut de_campo: Vec<(String, Entrada)> = Vec::new();
        let mut de_setter: Vec<(String, Entrada)> = Vec::new();
        let mut saidas_acessor: Vec<(String, String)> = Vec::new();
        let mut saidas_campo: Vec<(String, String)> = Vec::new();
        let mut ligacoes_acessor = Vec::new();
        let mut ligacoes_metodo = Vec::new();
        let mut ligacoes_campo = Vec::new();
        for &m in dc.membros {
            let membro = dc.ast.member(m);
            for a in membro.metadata.iter() {
                let Some(qual) = self.anotacao_do_ngdart(dc.lib, a, &mut d.fora) else {
                    continue;
                };
                let apelido = self.primeiro_texto(dc, a);
                match (qual, &membro.kind) {
                    ("Input", ast::MemberKind::Field(l)) if !l.static_ => {
                        if l.final_ || l.const_ {
                            // Sem setter: o oficial avisa e ignora.
                            continue;
                        }
                        for v in l.variables.iter() {
                            let nome = self.nome(&v.name).to_string();
                            if nome.starts_with('_') {
                                continue;
                            }
                            let booleana = match l.ty {
                                Some(t) => self.booleana(dc, t),
                                None => match v.initializer.map(|e| &dc.ast.expr(e).kind) {
                                    Some(ast::ExprKind::Bool(_)) => Some(true),
                                    None | Some(ast::ExprKind::Null) => Some(false),
                                    _ => None,
                                },
                            };
                            de_campo.push((
                                nome.clone(),
                                Entrada {
                                    nome: apelido.clone().unwrap_or_else(|| nome.clone()),
                                    membro: nome,
                                    booleana,
                                },
                            ));
                        }
                    }
                    ("Input", ast::MemberKind::Method(f)) => {
                        let funcao = dc.ast.function(*f);
                        let (ast::FunctionKind::Setter, Some(n), false) =
                            (funcao.kind, funcao.name, funcao.static_)
                        else {
                            continue;
                        };
                        let nome = self.nome(&n).to_string();
                        if nome.starts_with('_') {
                            continue;
                        }
                        let booleana = funcao
                            .parameters
                            .as_deref()
                            .and_then(|ps| ps.first())
                            .and_then(|p| p.ty)
                            .and_then(|t| self.booleana(dc, t));
                        de_setter.push((
                            nome.clone(),
                            Entrada {
                                nome: apelido.clone().unwrap_or_else(|| nome.clone()),
                                membro: nome,
                                booleana,
                            },
                        ));
                    }
                    ("Output", ast::MemberKind::Field(l)) if !l.static_ => {
                        for v in l.variables.iter() {
                            let nome = self.nome(&v.name).to_string();
                            if nome.starts_with('_') {
                                continue;
                            }
                            saidas_campo.push((apelido.clone().unwrap_or_else(|| nome.clone()), nome));
                        }
                    }
                    ("Output", ast::MemberKind::Method(f)) => {
                        let funcao = dc.ast.function(*f);
                        if let (ast::FunctionKind::Getter, Some(n), false) =
                            (funcao.kind, funcao.name, funcao.static_)
                        {
                            let nome = self.nome(&n).to_string();
                            if !nome.starts_with('_') {
                                saidas_acessor
                                    .push((apelido.clone().unwrap_or_else(|| nome.clone()), nome));
                            }
                        }
                    }
                    ("HostListener", ast::MemberKind::Method(f)) => {
                        match self.ouvinte(dc, *f, a) {
                            Ok(o) => match d.ouvintes.iter_mut().find(|x| x.evento == o.evento) {
                                // O mapa do oficial: o último vence, na
                                // posição do primeiro.
                                Some(x) => *x = o,
                                None => d.ouvintes.push(o),
                            },
                            Err(e) => d.fora.push(e),
                        }
                    }
                    ("HostListener", _) => d.fora.push("@HostListener fora de método".into()),
                    ("HostBinding", k) => {
                        let membro_nome = match k {
                            ast::MemberKind::Field(l) => {
                                l.variables.first().map(|v| self.nome(&v.name).to_string())
                            }
                            ast::MemberKind::Method(f) => {
                                dc.ast.function(*f).name.map(|n| self.nome(&n).to_string())
                            }
                            _ => None,
                        }
                        .unwrap_or_default();
                        let ligacao = (apelido.clone().unwrap_or_else(|| membro_nome.clone()), membro_nome);
                        match k {
                            ast::MemberKind::Field(_) => ligacoes_campo.push(ligacao),
                            ast::MemberKind::Method(f)
                                if matches!(
                                    dc.ast.function(*f).kind,
                                    ast::FunctionKind::Getter | ast::FunctionKind::Setter
                                ) =>
                            {
                                ligacoes_acessor.push(ligacao)
                            }
                            _ => ligacoes_metodo.push(ligacao),
                        }
                    }
                    ("ContentChild" | "ContentChildren" | "ViewChild" | "ViewChildren", _) => {
                        d.consultas = true;
                    }
                    _ => {}
                }
            }
        }
        // `_inputs..addAll(_fieldInputs)..addAll(_setterInputs)`: a chave é o
        // membro; repetir a chave troca o valor e mantém a posição.
        for (chave, e) in de_campo.into_iter().chain(de_setter) {
            match entradas.iter_mut().find(|(k, _)| *k == chave) {
                Some(x) => x.1 = e,
                None => entradas.push((chave, e)),
            }
        }
        for (nome, membro) in saidas_acessor.into_iter().chain(saidas_campo) {
            match d.saidas.iter_mut().find(|(_, m)| *m == membro) {
                Some(x) => x.0 = nome,
                None => d.saidas.push((nome, membro)),
            }
        }
        for (nome, membro) in ligacoes_acessor
            .into_iter()
            .chain(ligacoes_metodo)
            .chain(ligacoes_campo)
        {
            match d.ligacoes_do_hospedeiro.iter_mut().find(|(n, _)| *n == nome) {
                Some(x) => x.1 = membro,
                None => d.ligacoes_do_hospedeiro.push((nome, membro)),
            }
        }
    }

    /// O primeiro argumento posicional de uma anotação, se é texto.
    fn primeiro_texto(&self, dc: &Declaracao, a: &ast::Annotation) -> Option<String> {
        let args = a.arguments.as_ref()?;
        let x = args.args.iter().find(|x| x.name.is_none())?;
        match self.valor(dc.unidade, x.value, 0) {
            Ok(Valor::Texto(s)) => Some(s),
            _ => None,
        }
    }

    /// O tipo de uma entrada é `bool`? (`_isBoolType` do nome do tipo, que
    /// ignora o `?`). Parâmetro de tipo da classe: `None` (o oficial usa o
    /// limite, que daqui não se lê).
    fn booleana(&self, dc: &Declaracao, t: ast::TypeId) -> Option<bool> {
        match &dc.ast.ty(t).kind {
            ast::TypeKind::Named { name, .. } => {
                let partes: Vec<&str> = name.iter().map(|n| self.nome(n)).collect();
                let simples = *partes.last()?;
                if partes.len() == 1 && dc.parametros_de_tipo.iter().any(|p| p == simples) {
                    return None;
                }
                let tipo = self.tipo(dc.lib, t, dc.ast);
                Some(
                    tipo.and_then(|t| t.classe)
                        .is_some_and(|c| c.e("dart:core", "bool")),
                )
            }
            _ => Some(false),
        }
    }

    /// Um `@HostListener`, como o `_addHostListener` o monta.
    fn ouvinte(
        &self,
        dc: &Declaracao,
        f: ast::FunctionId,
        a: &ast::Annotation,
    ) -> Result<Ouvinte, String> {
        let funcao = dc.ast.function(f);
        if funcao.static_ || !matches!(funcao.kind, ast::FunctionKind::Function) {
            return Err("@HostListener fora de método de instância".into());
        }
        let metodo = funcao
            .name
            .map(|n| self.nome(&n).to_string())
            .ok_or("@HostListener sem nome")?;
        let args = a.arguments.as_ref().ok_or("@HostListener sem argumentos")?;
        let mut evento = None;
        let mut lista = None;
        let mut posicao = 0;
        for x in args.args.iter() {
            let campo = match x.name.as_ref().map(|n| self.nome(n)) {
                Some(n) => n,
                None => {
                    posicao += 1;
                    if posicao == 1 { "eventName" } else { "args" }
                }
            };
            let v = self.valor(dc.unidade, x.value, 0)?;
            match (campo, v) {
                ("eventName", Valor::Texto(s)) => evento = Some(s),
                ("args", Valor::Lista(itens)) => {
                    let mut textos = Vec::new();
                    for i in itens {
                        let Valor::Texto(s) = i else {
                            return Err("@HostListener com args que não são texto".into());
                        };
                        textos.push(s);
                    }
                    lista = Some(textos);
                }
                _ => return Err("@HostListener ilegível".into()),
            }
        }
        let evento = evento.ok_or("@HostListener sem evento")?;
        let mut argumentos = lista.unwrap_or_default();
        let parametros = funcao.parameters.as_ref().map_or(0, |p| p.len());
        if argumentos.is_empty() && parametros == 1 {
            argumentos.push("$event".into());
        }
        Ok(Ouvinte {
            evento,
            metodo,
            args: argumentos.join(", "),
        })
    }

    /// `extractLifecycleHooks`: a classe é atribuível à interface.
    fn ganchos(&self, classes: &[ClassId]) -> Ganchos {
        let mut g = Ganchos::default();
        for &c in classes {
            let k = self.classe(c);
            if k.uri != LIFECYCLE_HOOKS {
                continue;
            }
            match k.nome.as_str() {
                "OnInit" => g.on_init = true,
                "OnDestroy" => g.on_destroy = true,
                "DoCheck" => g.do_check = true,
                "AfterChanges" => g.after_changes = true,
                "AfterContentInit" => g.after_content_init = true,
                "AfterContentChecked" => g.after_content_checked = true,
                "AfterViewInit" => g.after_view_init = true,
                "AfterViewChecked" => g.after_view_checked = true,
                _ => {}
            }
        }
        g
    }

    /// As dependências do construtor sem nome (ou o primeiro), como o
    /// `CompileTypeMetadataVisitor.unnamedConstructor`.
    fn dependencias(&self, decl: &Declaracao<'a>, d: &mut Diretiva) {
        if decl.abstrata {
            return;
        }
        let mut construtores = Vec::new();
        for &m in decl.membros {
            if let ast::MemberKind::Constructor(c) = &decl.ast.member(m).kind {
                construtores.push(c);
            }
        }
        let Some(ctor) = construtores
            .iter()
            .find(|c| c.name.is_none())
            .or(construtores.first())
        else {
            return;
        };
        if ctor.name.is_some_and(|n| self.nome(&n).starts_with('_')) {
            d.fora.push("construtor privado".into());
            return;
        }
        for p in ctor.parameters.iter() {
            if matches!(p.kind, ast::ParameterKind::Named) {
                continue;
            }
            match self.dependencia(decl, p) {
                Ok(x) => d.dependencias.push(x),
                Err(f) => {
                    d.fora.push(f);
                    return;
                }
            }
        }
    }

    fn dependencia(&self, decl: &Declaracao<'a>, p: &ast::Parameter) -> Result<Dependencia, String> {
        let mut dep = Dependencia {
            token: Token::Elemento,
            opcional: matches!(p.kind, ast::ParameterKind::Optional),
            proprio: false,
            hospedeiro: false,
            pular: false,
        };
        let mut injetado = None;
        for a in p.metadata.iter() {
            match self.classe_da_anotacao(decl.lib, a) {
                Some(c) if c.uri == DI_ARGUMENTS => match c.nome.as_str() {
                    "Optional" => dep.opcional = true,
                    "Self" => dep.proprio = true,
                    "Host" => dep.hospedeiro = true,
                    "SkipSelf" => dep.pular = true,
                    "Inject" => {
                        let x = a
                            .arguments
                            .as_ref()
                            .and_then(|args| args.args.first())
                            .ok_or("@Inject sem token")?;
                        let v = self.valor(decl.unidade, x.value, 0)?;
                        injetado = Some(self.token_do_valor(&v)?);
                    }
                    outro => return Err(format!("@{outro} no construtor da diretiva")),
                },
                Some(c) if c.uri == DIRECTIVES && c.nome == "Attribute" => {
                    return Err("@Attribute no construtor da diretiva".into());
                }
                Some(_) => {}
                None => {
                    // Um token usado como anotação (`@ngValidators`).
                    let partes: Vec<&str> = a.name.iter().map(|n| self.nome(n)).collect();
                    if let [n] = partes.as_slice()
                        && let Some(Element::Variable(v)) = self.r.elemento_em(decl.lib, None, n)
                    {
                        let v = self.variavel(v, 0)?;
                        injetado = Some(self.token_do_valor(&v)?);
                    } else {
                        return Err("anotação não resolvida no construtor da diretiva".into());
                    }
                }
            }
        }
        dep.token = match injetado {
            Some(t) => t,
            None => {
                let t = match p.ty {
                    Some(t) => Some((t, decl.ast)),
                    None if p.this_ => self.tipo_do_campo(decl, p),
                    None => None,
                }
                .ok_or("parâmetro sem tipo no construtor da diretiva")?;
                let tipo = self
                    .tipo(decl.lib, t.0, t.1)
                    .ok_or("tipo não resolvido no construtor da diretiva")?;
                let c = tipo
                    .classe
                    .ok_or("parâmetro dynamic no construtor da diretiva")?;
                if c.uri == "dart:html" && (c.nome == "HtmlElement" || c.nome == "Element") {
                    Token::Elemento
                } else if c.uri.starts_with("package:ngdart/") && c.nome == "ChangeDetectorRef" {
                    Token::Detector
                } else {
                    Token::Classe {
                        uri: c.uri,
                        classe: c.nome,
                    }
                }
            }
        };
        Ok(dep)
    }

    /// O tipo escrito do campo de um parâmetro `this.x`.
    fn tipo_do_campo(
        &self,
        decl: &Declaracao<'a>,
        p: &ast::Parameter,
    ) -> Option<(ast::TypeId, &'a ast::Ast)> {
        let alvo = self.nome(p.name.as_ref()?);
        for &m in decl.membros {
            if let ast::MemberKind::Field(l) = &decl.ast.member(m).kind
                && l.variables.iter().any(|v| self.nome(&v.name) == alvo)
            {
                return l.ty.map(|t| (t, decl.ast));
            }
        }
        None
    }

    /// Resolve um tipo escrito no escopo de `lib`.
    fn tipo(&self, lib: LibraryId, t: ast::TypeId, arvore: &ast::Ast) -> Option<Tipo> {
        let ast::TypeKind::Named { name, args } = &arvore.ty(t).kind else {
            return None;
        };
        let partes: Vec<&str> = name.iter().map(|n| self.nome(n)).collect();
        let el = match partes.as_slice() {
            ["dynamic"] => {
                return Some(Tipo {
                    classe: None,
                    args: Vec::new(),
                });
            }
            [n] => self.r.elemento_em(lib, None, n),
            [p, n] => self.r.elemento_em(lib, Some(p), n),
            _ => None,
        }?;
        let Element::Class(id) = el else {
            return None;
        };
        let mut argumentos = Vec::new();
        for a in args.iter() {
            argumentos.push(self.tipo(lib, *a, arvore)?);
        }
        Some(Tipo {
            classe: Some(self.classe(id)),
            args: argumentos,
        })
    }

    /// O valor de uma variável de topo `const`.
    fn variavel(
        &self,
        v: dartforge_elements::model::VariableId,
        profundidade: u32,
    ) -> Result<Valor, String> {
        let p = self.r.programa();
        let var = p.variable(v);
        if !var.const_ {
            return Err("variável que não é const".into());
        }
        let VariableRef::TopLevel { unit, decl, index } = var.node else {
            return Err("constante que não é de topo".into());
        };
        let u = p.unit(unit);
        let ast::DeclKind::Variables(l) = &u.ast.decl(decl).kind else {
            return Err("constante ilegível".into());
        };
        let e = l
            .variables
            .get(index)
            .and_then(|x| x.initializer)
            .ok_or("constante sem valor")?;
        self.valor(unit, e, profundidade + 1)
    }

    /// Avalia uma expressão constante escrita na unidade `unidade`.
    fn valor(&self, unidade: UnitId, e: ast::ExprId, profundidade: u32) -> Result<Valor, String> {
        if profundidade > PROFUNDIDADE {
            return Err("constante profunda demais".into());
        }
        let p = self.r.programa();
        let u = p.unit(unidade);
        let lib = u.library;
        let arvore = &u.ast;
        let elemento = |el: Element| -> Result<Valor, String> {
            match el {
                Element::Class(id) => Ok(Valor::Tipo(self.classe(id))),
                Element::Variable(v) => self.variavel(v, profundidade),
                _ => Err("nome que não é classe nem constante".into()),
            }
        };
        match &arvore.expr(e).kind {
            ast::ExprKind::String(s) => s
                .constant_value()
                .map(|t| Valor::Texto(t.to_string_lossy()))
                .ok_or_else(|| "texto com interpolação".into()),
            ast::ExprKind::Null => Ok(Valor::Nulo),
            ast::ExprKind::Parenthesized(x) => self.valor(unidade, *x, profundidade),
            ast::ExprKind::List { elements, .. } => {
                let mut itens = Vec::new();
                for el in elements.iter() {
                    let ast::CollectionElement::Expression(x) = el else {
                        return Err("lista com espalhamento ou `if`".into());
                    };
                    itens.push(self.valor(unidade, *x, profundidade + 1)?);
                }
                Ok(Valor::Lista(itens))
            }
            ast::ExprKind::Identifier(n) => {
                let el = self
                    .r
                    .elemento_em(lib, None, self.nome(n))
                    .ok_or("nome não resolvido")?;
                elemento(el)
            }
            ast::ExprKind::Property {
                target,
                name,
                null_aware: false,
            } => {
                let ast::ExprKind::Identifier(alvo) = &arvore.expr(*target).kind else {
                    return Err("propriedade de expressão".into());
                };
                let alvo = self.nome(alvo);
                if self.r.e_prefixo(lib, alvo) {
                    let el = self
                        .r
                        .elemento_em(lib, Some(alvo), self.nome(name))
                        .ok_or("nome não resolvido")?;
                    return elemento(el);
                }
                match self.r.elemento_em(lib, None, alvo) {
                    Some(Element::Class(id)) => Ok(Valor::Membro {
                        classe: self.classe(id),
                        nome: self.nome(name).to_string(),
                    }),
                    _ => Err("propriedade que não é de classe".into()),
                }
            }
            ast::ExprKind::Call { target, arguments } => {
                let (classe, construtor) = self.construtor(lib, arvore, *target)?;
                let tipos = arguments
                    .type_args
                    .iter()
                    .map(|t| self.tipo(lib, *t, arvore).ok_or("tipo não resolvido"))
                    .collect::<Result<Vec<_>, _>>()?;
                self.objeto(unidade, classe, construtor, tipos, arguments, profundidade)
            }
            ast::ExprKind::InstanceCreation {
                ty,
                constructor,
                arguments,
                ..
            } => {
                let t = self.tipo(lib, *ty, arvore).ok_or("tipo não resolvido")?;
                let classe = t.classe.ok_or("instância de dynamic")?;
                self.objeto(
                    unidade,
                    classe,
                    constructor.map(|n| self.nome(&n).to_string()),
                    t.args,
                    arguments,
                    profundidade,
                )
            }
            _ => Err("expressão que não é constante conhecida".into()),
        }
    }

    /// A classe e o construtor de `C(..)`, `C.nome(..)`, `p.C(..)` e
    /// `p.C.nome(..)`.
    fn construtor(
        &self,
        lib: LibraryId,
        arvore: &ast::Ast,
        alvo: ast::ExprId,
    ) -> Result<(Classe, Option<String>), String> {
        let classe = |el: Option<Element>| match el {
            Some(Element::Class(id)) => Ok(self.classe(id)),
            _ => Err("chamada que não é construtor".to_string()),
        };
        match &arvore.expr(alvo).kind {
            ast::ExprKind::Identifier(n) => Ok((classe(self.r.elemento_em(lib, None, self.nome(n)))?, None)),
            ast::ExprKind::Property {
                target,
                name,
                null_aware: false,
            } => match &arvore.expr(*target).kind {
                ast::ExprKind::Identifier(a) if self.r.e_prefixo(lib, self.nome(a)) => Ok((
                    classe(self.r.elemento_em(lib, Some(self.nome(a)), self.nome(name)))?,
                    None,
                )),
                ast::ExprKind::Identifier(a) => Ok((
                    classe(self.r.elemento_em(lib, None, self.nome(a)))?,
                    Some(self.nome(name).to_string()),
                )),
                ast::ExprKind::Property {
                    target: p,
                    name: c,
                    null_aware: false,
                } => match &arvore.expr(*p).kind {
                    ast::ExprKind::Identifier(pre) if self.r.e_prefixo(lib, self.nome(pre)) => Ok((
                        classe(self.r.elemento_em(lib, Some(self.nome(pre)), self.nome(c)))?,
                        Some(self.nome(name).to_string()),
                    )),
                    _ => Err("chamada que não é construtor".into()),
                },
                _ => Err("chamada que não é construtor".into()),
            },
            _ => Err("chamada que não é construtor".into()),
        }
    }

    fn objeto(
        &self,
        unidade: UnitId,
        classe: Classe,
        construtor: Option<String>,
        tipos: Vec<Tipo>,
        arguments: &ast::Arguments,
        profundidade: u32,
    ) -> Result<Valor, String> {
        let mut posicionais = Vec::new();
        let mut nomeados = Vec::new();
        for a in arguments.args.iter() {
            let v = self.valor(unidade, a.value, profundidade + 1)?;
            match a.name {
                Some(n) => nomeados.push((self.nome(&n).to_string(), v)),
                None => posicionais.push(v),
            }
        }
        Ok(Valor::Objeto {
            classe,
            construtor,
            tipos,
            posicionais,
            nomeados,
        })
    }
}


