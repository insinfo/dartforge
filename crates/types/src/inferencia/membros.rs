//! Busca de membros: de instância (interface, com substituição dos argumentos
//! de tipo do receptor), estáticos (literal de classe) e de extensão (com
//! inferência dos argumentos da extensão e desempate por especificidade).
//!
//! Chaves no modelo de elementos: getters e métodos pelo nome, setters por
//! `nome_=`, operadores pelo texto (`[]`, `[]=`, `unary-`).

use super::BodyInferrer;
use crate::constraints::GenericInferrer;
use crate::resolved::{MemberRef, Resolved};
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, ExtensionId, FunctionElementId, FunctionKind, LibraryId};
use dartforge_intern::SymbolId;

/// Um membro encontrado, já com o tipo instanciado para o receptor.
#[derive(Debug, Clone)]
pub(crate) struct Membro {
    pub resolved: Resolved,
    /// Getter/campo: tipo do valor; setter: tipo do parâmetro; método: tipo de função.
    pub tipo: TypeId,
    /// Método ou operador (invocável/tear-off), em oposição a getter/campo/setter.
    pub metodo: bool,
    /// A função declarada (para quem precisar do elemento).
    #[allow(dead_code)]
    pub funcao: Option<FunctionElementId>,
    pub de_extensao: bool,
}

/// Resultado de uma busca de membro num receptor.
#[derive(Debug, Clone)]
pub(crate) enum Busca {
    /// Receptor `dynamic` (ou inválido): despacho dinâmico.
    Dinamico,
    /// Receptor `Never`: o acesso tem tipo `Never`.
    Nunca,
    Achado(Membro),
    Ausente,
}

impl<'a> BodyInferrer<'a> {
    /// Chave de setter (`nome_=`), se existir algum setter com esse nome.
    pub(crate) fn chave_setter(&self, nome: SymbolId) -> Option<SymbolId> {
        let s = format!("{}_=", self.interner.resolve(nome));
        self.interner.lookup(&s)
    }

    /// Tipo de um membro declarado, antes da substituição da classe.
    /// Devolve `(tipo, é_método)`.
    pub(crate) fn tipo_do_membro_declarado(&mut self, f: FunctionElementId, setter: bool) -> (TypeId, bool) {
        let fe = self.program.function(f);
        match fe.kind {
            FunctionKind::ImplicitAccessor => {
                let t = match fe.variable {
                    Some(v) => self.tipo_variavel(v),
                    None => self.core.dynamic_,
                };
                (t, false)
            }
            FunctionKind::Getter => (self.outline.functions[f.0 as usize].return_type, false),
            FunctionKind::Setter => {
                let t = self.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(self.core.dynamic_);
                (t, false)
            }
            _ => {
                let _ = setter;
                (self.outline.functions[f.0 as usize].signature, true)
            }
        }
    }

    /// Declaração do membro `chave` em `classe` ou nos seus supertipos, na
    /// ordem de busca (superclasses e mixins, depois interfaces).
    fn declaracao_em_classe(&self, classe: ClassId, chave: SymbolId) -> Option<(ClassId, FunctionElementId)> {
        let c = self.program.class(classe);
        if let Some(&f) = c.instance_members.get(&chave) {
            return Some((classe, f));
        }
        for (sup, _) in crate::scope::supertipos_ordenados(self.program, &self.outline.hierarchy, classe) {
            if let Some(&f) = self.program.class(sup).instance_members.get(&chave) {
                return Some((sup, f));
            }
        }
        None
    }

    /// Membro de instância pela interface do receptor (sem extensões).
    pub(crate) fn membro_de_interface(&mut self, recv: TypeId, nome: SymbolId, setter: bool) -> Option<Membro> {
        // `Null` só tem os membros de `Object` (`null.hashCode`).
        let recv = if matches!(self.table.get(recv), Type::Null) { self.core.object } else { recv };
        let recv = self.nao_nulo(recv);
        let recv = self.completar_args(recv);
        if setter {
            if let Some(m) = self.chave_setter(nome).and_then(|_| self.membro_de_interface_chave(recv, nome, true)) {
                return Some(m);
            }
            // `late final x;` sem inicializador tem setter implícito (uma
            // única atribuição); o modelo de elementos só o cria para
            // campos não finais.
            let m = self.membro_de_interface_chave(recv, nome, false)?;
            let e_late_final = match m.funcao.map(|f| self.program.function(f)) {
                Some(fe) if fe.kind == FunctionKind::ImplicitAccessor => fe.variable.is_some_and(|v| {
                    let ve = self.program.variable(v);
                    ve.late && ve.final_ && self.inicializador(v).is_none()
                }),
                _ => false,
            };
            return if e_late_final { Some(m) } else { None };
        }
        self.membro_de_interface_chave(recv, nome, false)
    }

    fn membro_de_interface_chave(&mut self, recv: TypeId, nome: SymbolId, setter: bool) -> Option<Membro> {
        let chave = if setter { self.chave_setter(nome)? } else { nome };
        match self.table.get(recv).clone() {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                if !setter
                    && let Some(m) = self.membro_representacao(recv, class, chave)
                {
                    return Some(m);
                }
                let (dono, f) = self.declaracao_em_classe(class, chave)?;
                let (t, metodo) = self.tipo_do_membro_declarado(f, setter);
                let t = self.substituir_do_dono(recv, class, dono, t);
                // Membros de instância (inclusive o getter implícito de um
                // campo) são referidos pela função, como o emissor espera.
                let member = MemberRef::Function(f);
                Some(Membro {
                    resolved: Resolved::Member { class: dono, member, via_super: false },
                    tipo: t,
                    metodo,
                    funcao: Some(f),
                    de_extensao: false,
                })
            }
            Type::Intersection { bound, .. } => self.membro_de_interface(bound, nome, setter),
            Type::TypeParameter { param, .. } => {
                if param == self.core.unknown_param {
                    return None;
                }
                let b = self.table.param(param).bound;
                if b == recv {
                    return None;
                }
                let b = if b == self.core.object_nullable { self.core.object } else { b };
                self.membro_de_interface(b, nome, setter)
            }
            Type::Function { .. } => {
                if !setter && Some(nome) == self.sym.call {
                    return Some(Membro { resolved: Resolved::Dynamic, tipo: recv, metodo: true, funcao: None, de_extensao: false });
                }
                let f = self.core.function;
                self.membro_de_interface(f, nome, setter)
            }
            Type::Record { positional, named, .. } => {
                if !setter {
                    for (n, t) in named.iter() {
                        if *n == nome {
                            return Some(Membro { resolved: Resolved::Dynamic, tipo: *t, metodo: false, funcao: None, de_extensao: false });
                        }
                    }
                    let s = self.interner.resolve(nome);
                    if let Some(d) = s.strip_prefix('$') {
                        if let Ok(i) = d.parse::<usize>() {
                            if i >= 1 && i <= positional.len() {
                                return Some(Membro { resolved: Resolved::Dynamic, tipo: positional[i - 1], metodo: false, funcao: None, de_extensao: false });
                            }
                        }
                    }
                }
                let r = self.core.record;
                self.membro_de_interface(r, nome, setter)
            }
            Type::FutureOr { .. } | Type::Null | Type::Void => {
                let o = self.core.object;
                self.membro_de_interface(o, nome, setter)
            }
            _ => None,
        }
    }

    /// Campo de representação de um tipo de extensão (`extension type
    /// Id(int v)`: `i.v`, `v` no corpo; R-EXT-04). O modelo de elementos não
    /// cria o getter implícito.
    fn membro_representacao(&mut self, recv: TypeId, class: ClassId, nome: SymbolId) -> Option<Membro> {
        let cl = self.program.class(class);
        if cl.kind != dartforge_elements::model::ClassKind::ExtensionType {
            return None;
        }
        let rep = cl.representation?;
        if self.program.variable(rep).name != nome {
            return None;
        }
        let t = self.outline.variables[rep.0 as usize].declared_type?;
        let tipo = self.substituir_do_dono(recv, class, class, t);
        Some(Membro {
            resolved: Resolved::Member { class, member: MemberRef::Variable(rep), via_super: false },
            tipo,
            metodo: false,
            funcao: None,
            de_extensao: false,
        })
    }

    /// Substitui os parâmetros de tipo da classe dona pelo que o receptor
    /// instancia (`List<int>` → membro de `Iterable<E>` com `E = int`).
    pub(crate) fn substituir_do_dono(&mut self, recv: TypeId, classe_recv: ClassId, dono: ClassId, t: TypeId) -> TypeId {
        let params = self.outline.classes[dono.0 as usize].type_params.clone();
        if params.is_empty() {
            return t;
        }
        let args: Vec<TypeId> = if dono == classe_recv {
            match self.table.get(recv) {
                Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
                _ => return t,
            }
        } else {
            match self.outline.hierarchy.supertype_of(recv, dono, self.table, self.core) {
                Some(s) => match self.table.get(s) {
                    Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
                    _ => return t,
                },
                None => return t,
            }
        };
        if args.len() != params.len() {
            return t;
        }
        let mapa = self.mapa(&params, &args);
        self.subst(t, &mapa)
    }

    /// Busca completa: interface, depois extensões acessíveis em `lib`.
    pub(crate) fn buscar_membro(&mut self, lib: LibraryId, recv: TypeId, nome: SymbolId, setter: bool) -> Busca {
        match self.table.get(recv) {
            Type::Dynamic => return Busca::Dinamico,
            Type::Never => return Busca::Nunca,
            _ => {}
        }
        if self.e_desconhecido(recv) {
            return Busca::Dinamico;
        }
        let anulavel = self.e_anulavel(recv) && !matches!(self.table.get(recv), Type::Null);
        // Receptor anulável: só membros de `Object` pela interface; o resto
        // pode vir de extensões sobre o tipo anulável.
        if anulavel {
            let o = self.core.object;
            if let Some(m) = self.membro_de_interface(o, nome, setter) {
                return Busca::Achado(m);
            }
            if let Some(m) = self.membro_de_extensao(lib, recv, nome, setter) {
                return Busca::Achado(m);
            }
        }
        if let Some(m) = self.membro_de_interface(recv, nome, setter) {
            return Busca::Achado(m);
        }
        if let Some(m) = self.membro_de_extensao(lib, recv, nome, setter) {
            return Busca::Achado(m);
        }
        Busca::Ausente
    }

    /// Argumentos da extensão `e` para o receptor, se ela se aplica.
    pub(crate) fn extensao_aplicavel(&mut self, e: ExtensionId, recv: TypeId) -> Option<Vec<TypeId>> {
        let dados = self.outline.extensions[e.0 as usize].clone();
        if dados.type_params.is_empty() {
            return if self.sub(recv, dados.on) { Some(Vec::new()) } else { None };
        }
        // Parâmetros novos: dentro da própria extensão o receptor menciona
        // os parâmetros dela (`this` é `Iterable<T>`), e `Iterable<T> <#
        // Iterable<T>` com `T` em `L` não restringiria nada.
        let novos = self.parametros_novos(&dados.type_params);
        let tipos: Vec<TypeId> = novos.iter().map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let m0 = self.mapa(&dados.type_params, &tipos);
        let on_novo = self.subst(dados.on, &m0);
        let mut inf = GenericInferrer::new(&novos);
        let mut env = self.env();
        inf.constrain_argument(recv, on_novo, &mut env);
        let args = inf.choose_final(&mut env);
        drop(env);
        let mapa = self.mapa(&dados.type_params, &args);
        let on = self.subst(dados.on, &mapa);
        if self.sub(recv, on) {
            Some(args)
        } else {
            None
        }
    }

    /// Membro de extensão aplicável ao receptor, com desempate por especificidade.
    pub(crate) fn membro_de_extensao(&mut self, lib: LibraryId, recv: TypeId, nome: SymbolId, setter: bool) -> Option<Membro> {
        let chave = if setter { self.chave_setter(nome)? } else { nome };
        let exts = self.extensoes_acessiveis(lib);
        let mut candidatos: Vec<(ExtensionId, FunctionElementId, Vec<TypeId>, TypeId)> = Vec::new();
        for &e in exts.iter() {
            let Some(&f) = self.program.extension(e).instance_members.get(&chave) else { continue };
            if let Some(args) = self.extensao_aplicavel(e, recv) {
                let dados = self.outline.extensions[e.0 as usize].clone();
                let mapa = self.mapa(&dados.type_params, &args);
                let on = self.subst(dados.on, &mapa);
                candidatos.push((e, f, args, on));
            }
        }
        if candidatos.is_empty() {
            return None;
        }
        let mut melhor = 0;
        if candidatos.len() > 1 {
            let n = candidatos.len();
            let sdk: Vec<bool> = candidatos.iter().map(|c| self.program.library(self.program.extension(c.0).library).is_sdk).collect();
            'fora: for i in 0..n {
                for j in 0..n {
                    if i == j {
                        continue;
                    }
                    let mais = if sdk[i] != sdk[j] {
                        !sdk[i]
                    } else {
                        let (a, b) = (candidatos[i].3, candidatos[j].3);
                        self.sub(a, b)
                    };
                    if !mais {
                        continue 'fora;
                    }
                }
                melhor = i;
                break;
            }
        }
        let (e, f, args, _) = candidatos.swap_remove(melhor);
        let (t, metodo) = self.tipo_do_membro_declarado(f, setter);
        let dados = self.outline.extensions[e.0 as usize].clone();
        let mapa = self.mapa(&dados.type_params, &args);
        let t = self.subst(t, &mapa);
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, tipo: t, metodo, funcao: Some(f), de_extensao: true })
    }

    /// Membro estático de uma classe (literal de classe como receptor):
    /// estáticos declarados, constantes de enum e tear-off de construtor.
    pub(crate) fn membro_estatico(&mut self, classe: ClassId, nome: SymbolId, setter: bool) -> Option<Membro> {
        let c = self.program.class(classe);
        if !setter {
            if let Some(&v) = c.enum_constants.iter().find(|&&v| self.program.variable(v).name == nome) {
                let t = self.tipo_this_classe(classe);
                return Some(Membro {
                    resolved: Resolved::Member { class: classe, member: MemberRef::Variable(v), via_super: false },
                    tipo: t,
                    metodo: false,
                    funcao: None,
                    de_extensao: false,
                });
            }
        }
        let chave = if setter { self.chave_setter(nome) } else { Some(nome) };
        if let Some(chave) = chave {
            if let Some(&f) = c.static_members.get(&chave) {
                let fe = self.program.function(f);
                let member = match (fe.kind, fe.variable) {
                    (FunctionKind::ImplicitAccessor, Some(v)) => MemberRef::Variable(v),
                    _ => MemberRef::Function(f),
                };
                let (t, metodo) = self.tipo_do_membro_declarado(f, setter);
                return Some(Membro {
                    resolved: Resolved::Member { class: classe, member, via_super: false },
                    tipo: t,
                    metodo,
                    funcao: Some(f),
                    de_extensao: false,
                });
            }
        }
        None
    }

    /// Membro estático de uma extensão (`Ext.m`).
    pub(crate) fn membro_estatico_de_extensao(&mut self, e: ExtensionId, nome: SymbolId, setter: bool) -> Option<Membro> {
        // Campos de extensão (sempre estáticos) não têm acessores no modelo.
        if let Some(&v) = self.program.extension(e).fields.iter().find(|&&v| self.program.variable(v).name == nome) {
            let t = self.tipo_variavel(v);
            return Some(Membro {
                resolved: Resolved::Element(dartforge_elements::model::Element::Variable(v)),
                tipo: t,
                metodo: false,
                funcao: None,
                de_extensao: true,
            });
        }
        let chave = if setter { self.chave_setter(nome)? } else { nome };
        let &f = self.program.extension(e).static_members.get(&chave)?;
        let (t, metodo) = self.tipo_do_membro_declarado(f, setter);
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, tipo: t, metodo, funcao: Some(f), de_extensao: true })
    }

    /// Membro declarado diretamente no corpo de uma classe/extensão (escopo
    /// léxico): instância ou estático.
    pub(crate) fn membro_declarado_lexico(&self, classe: Option<ClassId>, ext: Option<ExtensionId>, nome: SymbolId, setter_chave: Option<SymbolId>) -> Option<(FunctionElementId, bool)> {
        if let Some(c) = classe {
            let ce = self.program.class(c);
            for chave in [Some(nome), setter_chave].into_iter().flatten() {
                if let Some(&f) = ce.instance_members.get(&chave) {
                    return Some((f, false));
                }
                if let Some(&f) = ce.static_members.get(&chave) {
                    return Some((f, true));
                }
            }
            if ce.enum_constants.iter().any(|&v| self.program.variable(v).name == nome) {
                return None;
            }
        }
        if let Some(e) = ext {
            let ee = self.program.extension(e);
            for chave in [Some(nome), setter_chave].into_iter().flatten() {
                if let Some(&f) = ee.instance_members.get(&chave) {
                    return Some((f, false));
                }
                if let Some(&f) = ee.static_members.get(&chave) {
                    return Some((f, true));
                }
            }
        }
        None
    }
}
