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
    pub of_extension: bool,
}

/// Resultado de uma busca de membro num receptor.
#[derive(Debug, Clone)]
pub(crate) enum Busca {
    /// Receptor `dynamic` (ou inválido): despacho dinâmico.
    Dinamico,
    /// Receptor `Never`: o acesso tem tipo `Never`.
    NeverType,
    Achado(Membro),
    Missing,
}

/// A inferência como provedor da herança: o tipo de um acessor implícito é
/// o do campo, inferido sob demanda.
impl<'a> crate::heranca::Provedor<'a> for BodyInferrer<'a> {
    fn programa(&self) -> &'a dartforge_elements::model::Program {
        self.program
    }
    fn interner(&self) -> &'a dartforge_intern::Interner {
        self.interner
    }
    fn core(&self) -> &'a crate::table::CoreTypes {
        self.core
    }
    fn tabela(&mut self) -> &mut crate::table::TypeTable {
        &mut *self.table
    }
    fn class_data(&self, c: ClassId) -> crate::resolve::ClassTypeData {
        self.outline.classes[c.0 as usize].clone()
    }
    fn tipo_do_membro(&mut self, f: FunctionElementId) -> TypeId {
        let tv = match self.program.function(f).variable {
            // Na busca dos sobrescritos de um campo da classe `c` (o
            // `getOverridden2` do `_inferAccessorOrField`), os membros
            // declarados em `c` não são tipados: a interface do analyzer
            // guarda elementos, e o tipo do campo em curso não entra (aqui
            // ele faria um falso ciclo de inferência). A interface montada
            // assim é provisória e sai do cache depois da busca.
            Some(v)
                if self.program.function(f).kind == FunctionKind::ImplicitAccessor
                    && self.declared_untyped.is_some()
                    && self.program.variable(v).class == self.declared_untyped =>
            {
                self.provisional_inheritance = true;
                self.core.dynamic_
            }
            // O campo em inferência (`late final a = m();` busca `m` na
            // interface da própria classe): a interface do analyzer guarda
            // o elemento, e o tipo dele só é pedido por quem o lê. Aqui a
            // interface o leva provisório (sai do cache depois da busca), e
            // quem o lê de fato passa pelo `tipo_variavel` (o ciclo).
            Some(v) if self.program.function(f).kind == FunctionKind::ImplicitAccessor && matches!(self.vars_state[v.0 as usize], super::VarState::InProgress) => {
                self.provisional_inheritance = true;
                self.core.dynamic_
            }
            // Outro campo ainda não inferido, numa busca feita durante a
            // inferência de topo (`late final c = m(); late final d = c.x;`:
            // inferir `c` busca `m`): o analyzer só infere o que o
            // inicializador lê, e a interface dele guarda elementos. Inferir
            // `d` aqui faria um falso `TOP_LEVEL_CYCLE`. Só os membros com o
            // nome buscado decidem o resultado; os outros ficam provisórios.
            Some(v)
                if self.program.function(f).kind == FunctionKind::ImplicitAccessor
                    && self.nome_da_busca.is_some_and(|n| n != self.program.variable(v).name)
                    && !self.variable_stack.is_empty()
                    && matches!(self.vars_state[v.0 as usize], super::VarState::Pendente)
                    && self.outline.variables.get(v.0 as usize).is_some_and(|d| d.declared_type.is_none()) =>
            {
                self.provisional_inheritance = true;
                self.core.dynamic_
            }
            Some(v) if self.program.function(f).kind == FunctionKind::ImplicitAccessor => self.variable_type(v),
            _ => self.core.dynamic_,
        };
        crate::heranca::member_function_type(self.program, &*self.outline, self.core, &mut *self.table, f, tv)
    }
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        BodyInferrer::sub(self, a, b)
    }
    fn normalize_(&mut self, t: TypeId) -> TypeId {
        crate::ops::normalize(t, &mut *self.table, self.core)
    }
}

impl<'a> BodyInferrer<'a> {
    /// `getMember2` na interface de `classe`, pelo `Name` da biblioteca em
    /// que o acesso está.
    pub(crate) fn inheritance_member(&mut self, cls: ClassId, key: SymbolId, concrete: bool, to_super: bool) -> Option<crate::heranca::Membro> {
        let lib = match self.current_unit {
            Some(u) => self.program.unit(u).library,
            None => self.program.class(cls).library,
        };
        let name = crate::heranca::Nome::novo(self.interner, lib, key);
        // A busca pode inferir um campo, e a inferência dele buscar membros:
        // o gerenciador sai do lugar durante a chamada (uma busca aninhada
        // monta as interfaces que precisar num gerenciador próprio).
        let base = self.interner.resolve(key);
        let base = base.strip_suffix('=').map_or(Some(key), |b| self.interner.lookup(b));
        let saved_name = std::mem::replace(&mut self.nome_da_busca, base);
        let mut h = std::mem::take(&mut self.heranca);
        let finding = h.membro(self, cls, name, concrete, None, to_super);
        self.heranca = h;
        self.nome_da_busca = saved_name;
        self.after_provisional_lookup(finding.as_ref());
        finding
    }

    /// Depois de uma busca que montou interfaces com um campo em inferência
    /// provisório: fora da busca de sobrescritos (que limpa sozinha), as
    /// interfaces saem do cache; e o membro achado que é o próprio campo em
    /// inferência pede o tipo dele (o `TOP_LEVEL_CYCLE` de quem o lê).
    fn after_provisional_lookup(&mut self, finding: Option<&crate::heranca::Membro>) {
        if !self.provisional_inheritance || self.declared_untyped.is_some() {
            return;
        }
        self.heranca = crate::heranca::Heranca::default();
        self.provisional_inheritance = false;
        if let Some(m) = finding
            && self.program.function(m.funcao).kind == FunctionKind::ImplicitAccessor
            && let Some(v) = self.program.function(m.funcao).variable
            && matches!(self.vars_state[v.0 as usize], super::VarState::InProgress)
        {
            self.variable_type(v);
        }
    }

    /// `getInherited2` na interface de `classe`.
    pub(crate) fn inherited_from_inheritance(&mut self, cls: ClassId, key: SymbolId) -> Option<crate::heranca::Membro> {
        let lib = match self.current_unit {
            Some(u) => self.program.unit(u).library,
            None => self.program.class(cls).library,
        };
        let name = crate::heranca::Nome::novo(self.interner, lib, key);
        let base = self.interner.resolve(key);
        let base = base.strip_suffix('=').map_or(Some(key), |b| self.interner.lookup(b));
        let saved_name = std::mem::replace(&mut self.nome_da_busca, base);
        let mut h = std::mem::take(&mut self.heranca);
        let finding = h.herdado(self, cls, name);
        self.heranca = h;
        self.nome_da_busca = saved_name;
        self.after_provisional_lookup(finding.as_ref());
        finding
    }

    /// O valor de um membro da interface vindo do tipo de função dele: o
    /// retorno do getter, o parâmetro do setter, o tipo do método.
    fn inheritance_member_type(&self, m: &crate::heranca::Membro) -> (TypeId, bool) {
        match (m.especie, self.table.get(m.tipo)) {
            (crate::heranca::Especie::Getter, Type::Function { ret, .. }) => (*ret, false),
            (crate::heranca::Especie::Setter, Type::Function { positional, .. }) => (positional.first().copied().unwrap_or(self.core.dynamic_), false),
            (crate::heranca::Especie::Metodo, _) => (m.tipo, true),
            _ => (self.core.dynamic_, false),
        }
    }

    /// Chave de setter (`nome_=`), se existir algum setter com esse nome.
    pub(crate) fn setter_key(&self, name: SymbolId) -> Option<SymbolId> {
        let s = format!("{}_=", self.interner.resolve(name));
        self.interner.lookup(&s)
    }

    /// Tipo de um membro declarado, antes da substituição da classe.
    /// Devolve `(tipo, é_método)`.
    pub(crate) fn declared_member_type(&mut self, f: FunctionElementId, setter: bool) -> (TypeId, bool) {
        let fe = self.program.function(f);
        match fe.kind {
            FunctionKind::ImplicitAccessor => {
                let t = match fe.variable {
                    Some(v) => self.variable_type(v),
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

    /// Membro de instância pela interface do receptor (sem extensões).
    pub(crate) fn interface_member(&mut self, recv: TypeId, name: SymbolId, setter: bool) -> Option<Membro> {
        // `Null` só tem os membros de `Object` (`null.hashCode`).
        let recv = if matches!(self.table.get(recv), Type::Null) { self.core.object } else { recv };
        let recv = self.non_null(recv);
        let recv = self.complete_args(recv);
        if setter {
            if let Some(m) = self.setter_key(name).and_then(|_| self.interface_member_key(recv, name, true)) {
                return Some(m);
            }
            // `late final x;` sem inicializador tem setter implícito (uma
            // única atribuição); o modelo de elementos só o cria para
            // campos não finais.
            let m = self.interface_member_key(recv, name, false)?;
            let e_late_final = match m.funcao.map(|f| self.program.function(f)) {
                Some(fe) if fe.kind == FunctionKind::ImplicitAccessor => fe.variable.is_some_and(|v| {
                    let ve = self.program.variable(v);
                    ve.late && ve.final_ && self.inicializador(v).is_none()
                }),
                _ => false,
            };
            return if e_late_final { Some(m) } else { None };
        }
        self.interface_member_key(recv, name, false)
    }

    fn interface_member_key(&mut self, recv: TypeId, name: SymbolId, setter: bool) -> Option<Membro> {
        let key = if setter { self.setter_key(name)? } else { name };
        match self.table.get(recv).clone() {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                if !setter
                    && let Some(m) = self.representation_member(recv, class, key)
                {
                    return Some(m);
                }
                // A interface do `InheritanceManager3`. O membro declarado tem
                // o tipo calculado agora (o campo pode ter terminado a
                // inferência depois de a interface ser montada); o sintético
                // (`topMerge`) tem o da interface, nos parâmetros da classe.
                let m = self.inheritance_member(class, key, false, false)?;
                let f = m.funcao;
                let owner = self.program.function(f).class.unwrap_or(m.classe);
                let (t, method) = if m.sintetico {
                    let (t, method) = self.inheritance_member_type(&m);
                    (self.substitute_from_owner(recv, class, class, t), method)
                } else {
                    let (t, method) = self.declared_member_type(f, setter);
                    (self.substitute_from_owner(recv, class, owner, t), method)
                };
                // Membros de instância (inclusive o getter implícito de um
                // campo) são referidos pela função, como o emissor espera.
                let member = MemberRef::Function(f);
                Some(Membro {
                    resolved: Resolved::Member { class: owner, member, via_super: false },
                    tipo: t,
                    metodo: method,
                    funcao: Some(f),
                    of_extension: false,
                })
            }
            Type::Intersection { bound, .. } => self.interface_member(bound, name, setter),
            Type::TypeParameter { param, .. } => {
                if param == self.core.unknown_param {
                    return None;
                }
                let b = self.table.param(param).bound;
                if b == recv {
                    return None;
                }
                let b = if b == self.core.object_nullable { self.core.object } else { b };
                self.interface_member(b, name, setter)
            }
            Type::Function { .. } => {
                if !setter && Some(name) == self.sym.call {
                    return Some(Membro { resolved: Resolved::Dynamic, tipo: recv, metodo: true, funcao: None, of_extension: false });
                }
                let f = self.core.function;
                self.interface_member(f, name, setter)
            }
            Type::Record { positional, named, .. } => {
                if !setter {
                    for (n, t) in named.iter() {
                        if *n == name {
                            return Some(Membro { resolved: Resolved::Dynamic, tipo: *t, metodo: false, funcao: None, of_extension: false });
                        }
                    }
                    let s = self.interner.resolve(name);
                    if let Some(d) = s.strip_prefix('$') {
                        if let Ok(i) = d.parse::<usize>() {
                            if i >= 1 && i <= positional.len() {
                                return Some(Membro { resolved: Resolved::Dynamic, tipo: positional[i - 1], metodo: false, funcao: None, of_extension: false });
                            }
                        }
                    }
                }
                let r = self.core.record;
                self.interface_member(r, name, setter)
            }
            Type::FutureOr { .. } | Type::Null | Type::Void => {
                let o = self.core.object;
                self.interface_member(o, name, setter)
            }
            _ => None,
        }
    }

    /// Campo de representação de um tipo de extensão (`extension type
    /// Id(int v)`: `i.v`, `v` no corpo; R-EXT-04). O modelo de elementos não
    /// cria o getter implícito.
    fn representation_member(&mut self, recv: TypeId, class: ClassId, name: SymbolId) -> Option<Membro> {
        let cl = self.program.class(class);
        if cl.kind != dartforge_elements::model::ClassKind::ExtensionType {
            return None;
        }
        let rep = cl.representation?;
        if self.program.variable(rep).name != name {
            return None;
        }
        let t = self.outline.variables[rep.0 as usize].declared_type?;
        let ty = self.substitute_from_owner(recv, class, class, t);
        Some(Membro {
            resolved: Resolved::Member { class, member: MemberRef::Variable(rep), via_super: false },
            tipo: ty,
            metodo: false,
            funcao: None,
            of_extension: false,
        })
    }

    /// Substitui os parâmetros de tipo da classe dona pelo que o receptor
    /// instancia (`List<int>` → membro de `Iterable<E>` com `E = int`).
    pub(crate) fn substitute_from_owner(&mut self, recv: TypeId, recv_class: ClassId, owner: ClassId, t: TypeId) -> TypeId {
        let params = self.outline.classes[owner.0 as usize].type_params.clone();
        if params.is_empty() {
            return t;
        }
        let args: Vec<TypeId> = if owner == recv_class {
            match self.table.get(recv) {
                Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
                _ => return t,
            }
        } else {
            match self.outline.hierarchy.supertype_of(recv, owner, self.table, self.core) {
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
        let map = self.mapa(&params, &args);
        self.subst(t, &map)
    }

    /// Busca completa: interface, depois extensões acessíveis em `lib`.
    pub(crate) fn lookup_member(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> Busca {
        self.extension_ambiguity = None;
        let b = self.lookup_member_unambiguous(lib, recv, name, setter);
        // Extensões ambíguas: o analyzer não resolve (tipo inválido) e só
        // relata a ambiguidade.
        if matches!(b, Busca::Missing) && self.extension_ambiguity.is_some() {
            return Busca::Dinamico;
        }
        b
    }

    fn lookup_member_unambiguous(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> Busca {
        match self.table.get(recv) {
            Type::Dynamic => return Busca::Dinamico,
            Type::Never => return Busca::NeverType,
            _ => {}
        }
        if self.the_is_unknown(recv) {
            return Busca::Dinamico;
        }
        // Parâmetro de tipo de limite `dynamic` (`isDynamicBounded`,
        // `type_property_resolver.dart:81-90`): só os membros de `Object`, e o
        // resto sem erro (dinâmico).
        if matches!(self.table.get(recv), Type::TypeParameter { .. } | Type::Intersection { .. }) {
            let mut b = recv;
            for _ in 0..16 {
                match self.table.get(b) {
                    Type::TypeParameter { param, .. } => {
                        let l = self.table.param(*param).bound;
                        if l == b {
                            break;
                        }
                        b = l;
                    }
                    Type::Intersection { bound, .. } => b = *bound,
                    _ => break,
                }
            }
            if matches!(self.table.get(b), Type::Dynamic) {
                let o = self.core.object;
                if let Some(m) = self.interface_member(o, name, setter) {
                    return Busca::Achado(m);
                }
                return Busca::Dinamico;
            }
        }
        let nullable = self.the_is_nullable(recv) && !matches!(self.table.get(recv), Type::Null);
        // Receptor anulável: só membros de `Object` pela interface; o resto
        // pode vir de extensões sobre o tipo anulável.
        if nullable {
            let o = self.core.object;
            if let Some(m) = self.interface_member(o, name, setter) {
                return Busca::Achado(m);
            }
            if let Some(m) = self.extension_member(lib, recv, name, setter) {
                return Busca::Achado(m);
            }
        }
        if let Some(m) = self.interface_member(recv, name, setter) {
            return Busca::Achado(m);
        }
        // `_lookupInterfaceType` procura também o par (o setter `nome=` de um
        // getter, o getter de um setter; `[]=` de `[]` e vice-versa): se a
        // interface tem um dos dois, `_hasGetterOrSetter` e as extensões não
        // são consultadas (`type_property_resolver.dart:181-185, 251-281`).
        let par = if Some(name) == self.sym.indice {
            self.sym.set_index.and_then(|s| self.interface_member(recv, s, false))
        } else if Some(name) == self.sym.set_index {
            self.sym.indice.and_then(|s| self.interface_member(recv, s, false))
        } else {
            self.interface_member(recv, name, !setter)
        };
        if par.is_some() {
            return Busca::Missing;
        }
        if let Some(m) = self.extension_member(lib, recv, name, setter) {
            return Busca::Achado(m);
        }
        // O último recurso do `TypePropertyResolver`: a interface de
        // `Object` (o tipo de extensão sem `implements` não a tem na dele).
        if !nullable {
            let o = self.core.object;
            if let Some(m) = self.interface_member(o, name, setter) {
                return Busca::Achado(m);
            }
        }
        // `f.call` com `f` do tipo `Function` (de `dart:core`): sem erro e
        // dinâmico (`TypePropertyResolver`,
        // an611:src/dart/resolver/type_property_resolver.dart:186-191).
        if !nullable
            && Some(name) == self.sym.call
            && matches!(self.table.get(recv), Type::Interface { class, .. } if Some(*class) == self.core.function_class)
        {
            return Busca::Dinamico;
        }
        Busca::Missing
    }

    /// O acesso a `nome` num receptor potencialmente anulável exige checagem
    /// de nulo: o membro não é de `Object` nem de uma extensão que se aplica
    /// ao tipo anulável. O analyzer (`TypePropertyResolver`) relata então
    /// `unchecked_use_of_nullable_value` — nunca `undefined_*`, mesmo quando
    /// o membro também não existe no tipo não anulável. `void` fica de fora
    /// (é `use_of_void_result`, conferido antes), como `dynamic` e `Never`.
    pub(crate) fn requires_null_check(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> bool {
        match self.table.get(recv) {
            Type::Dynamic | Type::Never | Type::Void => return false,
            // `type_property_resolver.dart:91-96`: o tipo de extensão é
            // anulável só pelo `?` escrito (não por não ser subtipo de
            // `Object`).
            Type::ExtensionType { nullable: false, .. } => return false,
            _ => {}
        }
        // `isDynamicBounded`/`isInvalidBounded` (`type_property_resolver.dart:81-90`):
        // parâmetro de tipo cujo limite (resolvido) é `dynamic` ou inválido.
        {
            let mut b = recv;
            for _ in 0..16 {
                match self.table.get(b) {
                    Type::TypeParameter { param, .. } => {
                        let l = self.table.param(*param).bound;
                        if l == b {
                            break;
                        }
                        b = l;
                    }
                    Type::Intersection { bound, .. } => b = *bound,
                    _ => break,
                }
            }
            if b != recv && (matches!(self.table.get(b), Type::Dynamic) || self.table.e_invalido(b)) {
                return false;
            }
        }
        if self.the_is_unknown(recv) || self.is_non_nullable(recv) {
            return false;
        }
        let o = self.core.object;
        if self.interface_member(o, name, setter).is_some() {
            return false;
        }
        self.extension_member(lib, recv, name, setter).is_none()
    }

    /// Argumentos da extensão `e` para o receptor, se ela se aplica.
    pub(crate) fn extensao_aplicavel(&mut self, e: ExtensionId, recv: TypeId) -> Option<Vec<TypeId>> {
        let data = self.outline.extensions[e.0 as usize].clone();
        if data.type_params.is_empty() {
            return if self.sub(recv, data.on) { Some(Vec::new()) } else { None };
        }
        // Parâmetros novos: dentro da própria extensão o receptor menciona
        // os parâmetros dela (`this` é `Iterable<T>`), e `Iterable<T> <#
        // Iterable<T>` com `T` em `L` não restringiria nada.
        let new_ = self.new_parameters(&data.type_params);
        let types: Vec<TypeId> = new_.iter().map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let m0 = self.mapa(&data.type_params, &types);
        let on_novo = self.subst(data.on, &m0);
        let mut inf = GenericInferrer::new(&new_);
        let mut env = self.env();
        inf.constrain_argument(recv, on_novo, &mut env);
        let args = inf.choose_final(&mut env);
        drop(env);
        let map = self.mapa(&data.type_params, &args);
        let on = self.subst(data.on, &map);
        if !self.sub(recv, on) {
            return None;
        }
        // Os argumentos inferidos respeitam os limites (`T extends Struct`):
        // senão a extensão não se aplica (§13.2), e outra — a de
        // `Array<Array<T>>`, e não a de `Array<T extends AbiSpecificInteger>`
        // — é a escolhida.
        for (i, &p) in data.type_params.iter().enumerate() {
            let bnd = self.subst(self.table.param(p).bound, &map);
            if !self.sub(args[i], bnd) {
                return None;
            }
        }
        Some(args)
    }

    /// Membro de extensão aplicável ao receptor, com desempate por especificidade.
    pub(crate) fn extension_member(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> Option<Membro> {
        let key = if setter { self.setter_key(name)? } else { name };
        // `findExtension`: a extensão é candidata se declara o getter **ou**
        // o setter do nome base (`extension_member_resolver.dart:70-110`); a
        // escolhida sem o membro pedido não resolve. Operadores só por si.
        let identifier = self.interner.resolve(name).chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$');
        let par = if Some(name) == self.sym.indice {
            self.sym.set_index
        } else if Some(name) == self.sym.set_index {
            self.sym.indice
        } else if !identifier {
            None
        } else if setter {
            Some(name)
        } else {
            self.setter_key(name)
        };
        let exts = self.accessible_extensions(lib);
        let mut candidates: Vec<(ExtensionId, FunctionElementId, Vec<TypeId>, TypeId)> = Vec::new();
        for &e in exts.iter() {
            let members = &self.program.extension(e).instance_members;
            let Some(&f) = members.get(&key).or_else(|| par.and_then(|k| members.get(&k))) else { continue };
            if let Some(args) = self.extensao_aplicavel(e, recv) {
                let data = self.outline.extensions[e.0 as usize].clone();
                let map = self.mapa(&data.type_params, &args);
                let on = self.subst(data.on, &map);
                candidates.push((e, f, args, on));
            }
        }
        if candidates.is_empty() {
            return None;
        }
        // `_chooseMostSpecific` (`an611:src/dart/resolver/extension_member_resolver.dart:273-312`).
        let mut best_so_far: Option<usize> = None;
        let mut tied: Vec<usize> = Vec::new();
        for i in 0..candidates.len() {
            if !tied.is_empty() {
                let mut is_the_most = true;
                let mut has_more = false;
                for &o in &tied.clone() {
                    if !self.most_specific_extension(&candidates[i], &candidates[o]) {
                        is_the_most = false;
                    }
                    if self.most_specific_extension(&candidates[o], &candidates[i]) {
                        has_more = true;
                    }
                }
                if is_the_most {
                    best_so_far = Some(i);
                    tied.clear();
                } else if !has_more {
                    tied.push(i);
                }
            } else if let Some(b) = best_so_far {
                if self.most_specific_extension(&candidates[b], &candidates[i]) {
                } else if self.most_specific_extension(&candidates[i], &candidates[b]) {
                    best_so_far = Some(i);
                } else {
                    tied.push(b);
                    tied.push(i);
                    best_so_far = None;
                }
            } else {
                best_so_far = Some(i);
            }
        }
        let Some(best) = best_so_far else {
            // Ambíguo: `AMBIGUOUS_EXTENSION_MEMBER_ACCESS` fica pendente
            // para quem tem o nome (`relatar_ambiguidade_de_extensao`).
            let names: Vec<String> = tied
                .iter()
                .map(|&i| {
                    let x = self.program.extension(candidates[i].0);
                    match x.name {
                        Some(n) => format!("extension '{}'", self.interner.resolve(n)),
                        None => {
                            let on = self.outline.extensions[candidates[i].0 .0 as usize].on;
                            format!("unnamed extension on '{}'", self.table.format(on, self.interner, self.program))
                        }
                    }
                })
                .collect();
            let list = match names.len() {
                0 | 1 => names.join(""),
                2 => format!("{} and {}", names[0], names[1]),
                n => format!("{}, and {}", names[..n - 1].join(", "), names[n - 1]),
            };
            // Com exatamente duas, o 3.13.4 escreve as extensões por
            // extenso (`extension E1 on int`; T2, caso c13). Só as nomeadas
            // e sem parâmetros de tipo: a exibição das outras no 3.13.4 não
            // foi conferida, e sem os dois argumentos o texto fica o do 3.6.
            let display = |inf: &Self, i: usize| -> Option<String> {
                let x = inf.program.extension(candidates[i].0);
                let data = &inf.outline.extensions[candidates[i].0 .0 as usize];
                let n = x.name?;
                data.type_params.is_empty().then(|| {
                    format!("extension {} on {}", inf.interner.resolve(n), inf.table.format(data.on, inf.interner, inf.program))
                })
            };
            let two = match tied[..] {
                [a, b] => display(self, a).zip(display(self, b)),
                _ => None,
            };
            self.extension_ambiguity = Some((self.interner.resolve(name).to_string(), list, two));
            return None;
        };
        let (e, f, args, _) = candidates.swap_remove(best);
        if self.program.extension(e).instance_members.get(&key) != Some(&f) {
            return None;
        }
        // `notifyExtensionUsed`: a extensão escolhida usa os imports dela.
        self.body_types.extensoes_usadas.insert((lib, e));
        let (t, method) = self.declared_member_type(f, setter);
        let data = self.outline.extensions[e.0 as usize].clone();
        let map = self.mapa(&data.type_params, &args);
        let t = self.subst(t, &map);
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, tipo: t, metodo: method, funcao: Some(f), of_extension: true })
    }

    /// `_isMoreSpecific` (`extension_member_resolver.dart:382-418`): fora da
    /// plataforma vence a da plataforma; senão o tipo estendido instanciado
    /// é subtipo do outro e não o inverso, ou, empatando, o mesmo para os
    /// tipos estendidos instanciados para os limites.
    fn most_specific_extension(
        &mut self,
        a: &(ExtensionId, FunctionElementId, Vec<TypeId>, TypeId),
        b: &(ExtensionId, FunctionElementId, Vec<TypeId>, TypeId),
    ) -> bool {
        let sdk_a = self.program.library(self.program.extension(a.0).library).is_sdk;
        let sdk_b = self.program.library(self.program.extension(b.0).library).is_sdk;
        if sdk_a != sdk_b {
            return !sdk_a;
        }
        if !self.sub(a.3, b.3) {
            return false;
        }
        if !self.sub(b.3, a.3) {
            return true;
        }
        let la = self.on_at_bounds(a.0);
        let lb = self.on_at_bounds(b.0);
        self.sub(la, lb) && !self.sub(lb, la)
    }

    /// O tipo estendido de `e` instanciado para os limites.
    fn on_at_bounds(&mut self, e: ExtensionId) -> TypeId {
        let data = self.outline.extensions[e.0 as usize].clone();
        let args = self.instantiate_to_bounds(&data.type_params);
        let map = self.mapa(&data.type_params, &args);
        self.subst(data.on, &map)
    }

    /// Relata, no nome, a ambiguidade de extensão deixada pela última busca
    /// (`AMBIGUOUS_EXTENSION_MEMBER_ACCESS`,
    /// `extension_member_resolver.dart:115-128`).
    pub(crate) fn report_extension_ambiguity(&mut self, name: dartforge_diagnostics::Span) {
        if let Some((n, list, two)) = self.extension_ambiguity.take() {
            let code = dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_EXTENSION_MEMBER_ACCESS;
            match two {
                Some((a, b)) => self.warning_with_code(code, name, &[&n, &list, &a, &b]),
                None => self.warning_with_code(code, name, &[&n, &list]),
            }
        }
    }

    /// Membro de instância da extensão `e` já instanciada (`E(x).m`,
    /// sobreposição explícita, R-EXT-02): só essa extensão é consultada.
    pub(crate) fn explicit_extension_member(&mut self, e: ExtensionId, args: &[TypeId], name: SymbolId, setter: bool) -> Option<Membro> {
        let key = if setter { self.setter_key(name)? } else { name };
        let &f = self.program.extension(e).instance_members.get(&key)?;
        let (t, method) = self.declared_member_type(f, setter);
        let data = self.outline.extensions[e.0 as usize].clone();
        let t = if data.type_params.len() == args.len() {
            let map = self.mapa(&data.type_params, args);
            self.subst(t, &map)
        } else {
            t
        };
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, tipo: t, metodo: method, funcao: Some(f), of_extension: true })
    }

    /// `lookupStaticGetter` (e, sem ele, `lookupStaticMethod`) ou
    /// `lookupStaticSetter` de `classe`
    /// (an362:src/dart/element/element.dart:5378-5403): o primeiro membro
    /// estático acessível em `lib` na cadeia de implementações (a classe, os
    /// mixins do último ao primeiro, a superclasse). `(classe que o declara,
    /// é método)`.
    pub(crate) fn static_recovery(&mut self, lib: LibraryId, cls: ClassId, name: SymbolId, setter: bool) -> Option<(ClassId, bool)> {
        let chain = crate::heranca::Heranca::implementation_chain(self, cls);
        let private = self.interner.resolve(name).starts_with('_');
        let accessible: Vec<ClassId> = chain.into_iter().filter(|&k| !private || self.program.class(k).library == lib).collect();
        if setter {
            let key = self.setter_key(name)?;
            return accessible.into_iter().find_map(|k| {
                let &f = self.program.class(k).static_members.get(&key)?;
                matches!(self.program.function(f).kind, FunctionKind::Setter | FunctionKind::ImplicitAccessor).then_some((k, false))
            });
        }
        let getter = accessible.iter().copied().find(|&k| {
            let ke = self.program.class(k);
            ke.enum_constants.iter().any(|&v| self.program.variable(v).name == name)
                || ke.static_members.get(&name).is_some_and(|&f| matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor))
        });
        if let Some(k) = getter {
            return Some((k, false));
        }
        accessible.into_iter().find(|&k| {
            self.program.class(k).static_members.get(&name).is_some_and(|&f| {
                !matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor)
            })
        }).map(|k| (k, true))
    }

    /// A recuperação estática do `TypePropertyResolver.resolve` para um
    /// receptor que não tem o membro: no anulável, a de `Object` vem
    /// primeiro; depois a da classe do receptor resolvido ao limite (se for
    /// de interface); por fim a do `_lookupInterfaceType(objectType)` final
    /// (an362:src/dart/resolver/type_property_resolver.dart:97-245). O
    /// `_getterRecovery ??=` guarda a primeira.
    pub(crate) fn receiver_static_recovery(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> Option<(ClassId, bool)> {
        let object = self.core.object_class;
        let mut classes = Vec::new();
        if self.the_is_nullable(recv) {
            classes.extend(object);
        }
        classes.extend(self.bound_class(recv));
        classes.extend(object);
        classes.into_iter().find_map(|c| self.static_recovery(lib, c, name, setter))
    }

    /// `TypeSystemImpl.resolveToBound`: o parâmetro de tipo (sem `?`) pelo
    /// limite, a interseção pelo limite, até um tipo que não é nenhum dos dois.
    pub(crate) fn resolver_ao_limite(&self, t: TypeId) -> TypeId {
        let mut base = t;
        for _ in 0..64 {
            match self.table.get(base) {
                Type::TypeParameter { param, nullable: false } if *param != self.core.unknown_param => {
                    let b = self.table.param(*param).bound;
                    if b == base {
                        break;
                    }
                    base = b;
                }
                Type::Intersection { bound, .. } => base = *bound,
                _ => break,
            }
        }
        base
    }

    /// A classe de `resolveToBound(recv)` quando ele é um tipo de interface
    /// (classe, enum, mixin ou tipo de extensão), sem o `?`.
    fn bound_class(&mut self, recv: TypeId) -> Option<ClassId> {
        let mut t = self.non_null(recv);
        for _ in 0..64 {
            match self.table.get(t).clone() {
                Type::Interface { class, .. } => return Some(class),
                Type::ExtensionType { decl, .. } => return Some(decl),
                Type::Intersection { bound, .. } => t = bound,
                Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                    let b = self.table.param(param).bound;
                    if b == t {
                        return None;
                    }
                    t = b;
                }
                _ => return None,
            }
            t = self.non_null(t);
        }
        None
    }

    /// `INSTANCE_ACCESS_TO_STATIC_MEMBER`: o membro que não existe na
    /// interface do receptor é recuperado como estático
    /// ([`Self::recuperacao_estatica_do_receptor`]); o analyzer resolve o
    /// nome para ele (o tipo é o dele) e relata o acesso pela instância em
    /// `span` (`_checkForStaticMember`,
    /// an362:src/dart/resolver/property_element_resolver.dart:316-358;
    /// `_reportInstanceAccessToStaticMember`,
    /// an362:src/dart/resolver/method_invocation_resolver.dart:208-256).
    pub(crate) fn instance_access_to_static(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool, span: dartforge_diagnostics::Span) -> Option<Membro> {
        let (owner, method) = self.receiver_static_recovery(lib, recv, name, setter)?;
        let kind = if setter {
            "setter"
        } else if method {
            "method"
        } else {
            "getter"
        };
        let c = self.program.class(owner);
        let owner_type = match c.kind {
            dartforge_elements::model::ClassKind::Mixin => "mixin",
            dartforge_elements::model::ClassKind::Enum => "enum",
            dartforge_elements::model::ClassKind::ExtensionType => "extension type",
            _ => "class",
        };
        let text = self.interner.resolve(name).to_string();
        let cls = self.interner.resolve(c.name).to_string();
        self.warning_with_code(
            dartforge_diagnostics::codigos::compile_time_error::INSTANCE_ACCESS_TO_STATIC_MEMBER,
            span,
            &[&text, kind, &cls, owner_type],
        );
        self.static_member(owner, name, setter)
    }

    /// Membro estático de uma classe (literal de classe como receptor):
    /// estáticos declarados, constantes de enum e tear-off de construtor.
    pub(crate) fn static_member(&mut self, cls: ClassId, name: SymbolId, setter: bool) -> Option<Membro> {
        let c = self.program.class(cls);
        if !setter {
            if let Some(&v) = c.enum_constants.iter().find(|&&v| self.program.variable(v).name == name) {
                // O tipo da constante (`E<int>` em `a<int>()`), não o `E<T>`.
                let t = self.variable_type(v);
                return Some(Membro {
                    resolved: Resolved::Member { class: cls, member: MemberRef::Variable(v), via_super: false },
                    tipo: t,
                    metodo: false,
                    funcao: None,
                    of_extension: false,
                });
            }
        }
        let key = if setter { self.setter_key(name) } else { Some(name) };
        if let Some(key) = key {
            if let Some(&f) = c.static_members.get(&key) {
                let fe = self.program.function(f);
                let member = match (fe.kind, fe.variable) {
                    (FunctionKind::ImplicitAccessor, Some(v)) => MemberRef::Variable(v),
                    _ => MemberRef::Function(f),
                };
                let (t, method) = self.declared_member_type(f, setter);
                return Some(Membro {
                    resolved: Resolved::Member { class: cls, member, via_super: false },
                    tipo: t,
                    metodo: method,
                    funcao: Some(f),
                    of_extension: false,
                });
            }
        }
        None
    }

    /// Membro estático de uma extensão (`Ext.m`).
    pub(crate) fn extension_static_member(&mut self, e: ExtensionId, name: SymbolId, setter: bool) -> Option<Membro> {
        // Os campos estáticos de extensão não têm acessores no modelo (os de
        // instância têm, entre os membros de instância).
        if let Some(&v) = self
            .program
            .extension(e)
            .fields
            .iter()
            .find(|&&v| self.program.variable(v).name == name && self.program.variable(v).static_)
        {
            let t = self.variable_type(v);
            return Some(Membro {
                resolved: Resolved::Element(dartforge_elements::model::Element::Variable(v)),
                tipo: t,
                metodo: false,
                funcao: None,
                of_extension: true,
            });
        }
        let key = if setter { self.setter_key(name)? } else { name };
        let &f = self.program.extension(e).static_members.get(&key)?;
        let (t, method) = self.declared_member_type(f, setter);
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, tipo: t, metodo: method, funcao: Some(f), of_extension: true })
    }

    /// Membro declarado diretamente no corpo de uma classe/extensão (escopo
    /// léxico): instância ou estático.
    pub(crate) fn lexical_declared_member(&self, cls: Option<ClassId>, ext: Option<ExtensionId>, name: SymbolId, the_setter_key: Option<SymbolId>) -> Option<(FunctionElementId, bool)> {
        if let Some(c) = cls {
            let ce = self.program.class(c);
            for key in [Some(name), the_setter_key].into_iter().flatten() {
                if let Some(&f) = ce.instance_members.get(&key) {
                    return Some((f, false));
                }
                if let Some(&f) = ce.static_members.get(&key) {
                    return Some((f, true));
                }
            }
            if ce.enum_constants.iter().any(|&v| self.program.variable(v).name == name) {
                return None;
            }
        }
        if let Some(e) = ext {
            let ee = self.program.extension(e);
            for key in [Some(name), the_setter_key].into_iter().flatten() {
                if let Some(&f) = ee.instance_members.get(&key) {
                    return Some((f, false));
                }
                if let Some(&f) = ee.static_members.get(&key) {
                    return Some((f, true));
                }
            }
        }
        None
    }
}
