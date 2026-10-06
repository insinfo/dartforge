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
    fn dados_da_classe(&self, c: ClassId) -> crate::resolve::ClassTypeData {
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
                    && self.declarados_sem_tipar.is_some()
                    && self.program.variable(v).class == self.declarados_sem_tipar =>
            {
                self.heranca_provisoria = true;
                self.core.dynamic_
            }
            // O campo em inferência (`late final a = m();` busca `m` na
            // interface da própria classe): a interface do analyzer guarda
            // o elemento, e o tipo dele só é pedido por quem o lê. Aqui a
            // interface o leva provisório (sai do cache depois da busca), e
            // quem o lê de fato passa pelo `tipo_variavel` (o ciclo).
            Some(v) if self.program.function(f).kind == FunctionKind::ImplicitAccessor && matches!(self.estado_vars[v.0 as usize], super::EstadoVar::EmCurso) => {
                self.heranca_provisoria = true;
                self.core.dynamic_
            }
            Some(v) if self.program.function(f).kind == FunctionKind::ImplicitAccessor => self.tipo_variavel(v),
            _ => self.core.dynamic_,
        };
        crate::heranca::tipo_de_funcao_do_membro(self.program, &*self.outline, self.core, &mut *self.table, f, tv)
    }
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        BodyInferrer::sub(self, a, b)
    }
    fn normalizar(&mut self, t: TypeId) -> TypeId {
        crate::ops::normalize(t, &mut *self.table, self.core)
    }
}

impl<'a> BodyInferrer<'a> {
    /// `getMember2` na interface de `classe`, pelo `Name` da biblioteca em
    /// que o acesso está.
    pub(crate) fn membro_da_heranca(&mut self, classe: ClassId, chave: SymbolId, concreto: bool, para_super: bool) -> Option<crate::heranca::Membro> {
        let lib = match self.unidade_corrente {
            Some(u) => self.program.unit(u).library,
            None => self.program.class(classe).library,
        };
        let nome = crate::heranca::Nome::novo(self.interner, lib, chave);
        // A busca pode inferir um campo, e a inferência dele buscar membros:
        // o gerenciador sai do lugar durante a chamada (uma busca aninhada
        // monta as interfaces que precisar num gerenciador próprio).
        let mut h = std::mem::take(&mut self.heranca);
        let achado = h.membro(self, classe, nome, concreto, None, para_super);
        self.heranca = h;
        self.depois_da_busca_provisoria(achado.as_ref());
        achado
    }

    /// Depois de uma busca que montou interfaces com um campo em inferência
    /// provisório: fora da busca de sobrescritos (que limpa sozinha), as
    /// interfaces saem do cache; e o membro achado que é o próprio campo em
    /// inferência pede o tipo dele (o `TOP_LEVEL_CYCLE` de quem o lê).
    fn depois_da_busca_provisoria(&mut self, achado: Option<&crate::heranca::Membro>) {
        if !self.heranca_provisoria || self.declarados_sem_tipar.is_some() {
            return;
        }
        self.heranca = crate::heranca::Heranca::default();
        self.heranca_provisoria = false;
        if let Some(m) = achado
            && self.program.function(m.funcao).kind == FunctionKind::ImplicitAccessor
            && let Some(v) = self.program.function(m.funcao).variable
            && matches!(self.estado_vars[v.0 as usize], super::EstadoVar::EmCurso)
        {
            self.tipo_variavel(v);
        }
    }

    /// `getInherited2` na interface de `classe`.
    pub(crate) fn herdado_da_heranca(&mut self, classe: ClassId, chave: SymbolId) -> Option<crate::heranca::Membro> {
        let lib = match self.unidade_corrente {
            Some(u) => self.program.unit(u).library,
            None => self.program.class(classe).library,
        };
        let nome = crate::heranca::Nome::novo(self.interner, lib, chave);
        let mut h = std::mem::take(&mut self.heranca);
        let achado = h.herdado(self, classe, nome);
        self.heranca = h;
        self.depois_da_busca_provisoria(achado.as_ref());
        achado
    }

    /// O valor de um membro da interface vindo do tipo de função dele: o
    /// retorno do getter, o parâmetro do setter, o tipo do método.
    fn tipo_do_membro_da_heranca(&self, m: &crate::heranca::Membro) -> (TypeId, bool) {
        match (m.especie, self.table.get(m.tipo)) {
            (crate::heranca::Especie::Getter, Type::Function { ret, .. }) => (*ret, false),
            (crate::heranca::Especie::Setter, Type::Function { positional, .. }) => (positional.first().copied().unwrap_or(self.core.dynamic_), false),
            (crate::heranca::Especie::Metodo, _) => (m.tipo, true),
            _ => (self.core.dynamic_, false),
        }
    }

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
                // A interface do `InheritanceManager3`. O membro declarado tem
                // o tipo calculado agora (o campo pode ter terminado a
                // inferência depois de a interface ser montada); o sintético
                // (`topMerge`) tem o da interface, nos parâmetros da classe.
                let m = self.membro_da_heranca(class, chave, false, false)?;
                let f = m.funcao;
                let dono = self.program.function(f).class.unwrap_or(m.classe);
                let (t, metodo) = if m.sintetico {
                    let (t, metodo) = self.tipo_do_membro_da_heranca(&m);
                    (self.substituir_do_dono(recv, class, class, t), metodo)
                } else {
                    let (t, metodo) = self.tipo_do_membro_declarado(f, setter);
                    (self.substituir_do_dono(recv, class, dono, t), metodo)
                };
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
        self.ambiguidade_de_extensao = None;
        let b = self.buscar_membro_sem_ambiguidade(lib, recv, nome, setter);
        // Extensões ambíguas: o analyzer não resolve (tipo inválido) e só
        // relata a ambiguidade.
        if matches!(b, Busca::Ausente) && self.ambiguidade_de_extensao.is_some() {
            return Busca::Dinamico;
        }
        b
    }

    fn buscar_membro_sem_ambiguidade(&mut self, lib: LibraryId, recv: TypeId, nome: SymbolId, setter: bool) -> Busca {
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
        // `_lookupInterfaceType` procura também o par (o setter `nome=` de um
        // getter, o getter de um setter; `[]=` de `[]` e vice-versa): se a
        // interface tem um dos dois, `_hasGetterOrSetter` e as extensões não
        // são consultadas (`type_property_resolver.dart:181-185, 251-281`).
        let par = if Some(nome) == self.sym.indice {
            self.sym.indice_set.and_then(|s| self.membro_de_interface(recv, s, false))
        } else if Some(nome) == self.sym.indice_set {
            self.sym.indice.and_then(|s| self.membro_de_interface(recv, s, false))
        } else {
            self.membro_de_interface(recv, nome, !setter)
        };
        if par.is_some() {
            return Busca::Ausente;
        }
        if let Some(m) = self.membro_de_extensao(lib, recv, nome, setter) {
            return Busca::Achado(m);
        }
        // O último recurso do `TypePropertyResolver`: a interface de
        // `Object` (o tipo de extensão sem `implements` não a tem na dele).
        if !anulavel {
            let o = self.core.object;
            if let Some(m) = self.membro_de_interface(o, nome, setter) {
                return Busca::Achado(m);
            }
        }
        // `f.call` com `f` do tipo `Function` (de `dart:core`): sem erro e
        // dinâmico (`TypePropertyResolver`,
        // an611:src/dart/resolver/type_property_resolver.dart:186-191).
        if !anulavel
            && Some(nome) == self.sym.call
            && matches!(self.table.get(recv), Type::Interface { class, .. } if Some(*class) == self.core.function_class)
        {
            return Busca::Dinamico;
        }
        Busca::Ausente
    }

    /// O acesso a `nome` num receptor potencialmente anulável exige checagem
    /// de nulo: o membro não é de `Object` nem de uma extensão que se aplica
    /// ao tipo anulável. O analyzer (`TypePropertyResolver`) relata então
    /// `unchecked_use_of_nullable_value` — nunca `undefined_*`, mesmo quando
    /// o membro também não existe no tipo não anulável. `void` fica de fora
    /// (é `use_of_void_result`, conferido antes), como `dynamic` e `Never`.
    pub(crate) fn exige_checagem_de_nulo(&mut self, lib: LibraryId, recv: TypeId, nome: SymbolId, setter: bool) -> bool {
        match self.table.get(recv) {
            Type::Dynamic | Type::Never | Type::Void => return false,
            _ => {}
        }
        if self.e_desconhecido(recv) || self.e_nao_anulavel(recv) {
            return false;
        }
        let o = self.core.object;
        if self.membro_de_interface(o, nome, setter).is_some() {
            return false;
        }
        self.membro_de_extensao(lib, recv, nome, setter).is_none()
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
        if !self.sub(recv, on) {
            return None;
        }
        // Os argumentos inferidos respeitam os limites (`T extends Struct`):
        // senão a extensão não se aplica (§13.2), e outra — a de
        // `Array<Array<T>>`, e não a de `Array<T extends AbiSpecificInteger>`
        // — é a escolhida.
        for (i, &p) in dados.type_params.iter().enumerate() {
            let limite = self.subst(self.table.param(p).bound, &mapa);
            if !self.sub(args[i], limite) {
                return None;
            }
        }
        Some(args)
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
        // `_chooseMostSpecific` (`an611:src/dart/resolver/extension_member_resolver.dart:273-312`).
        let mut melhor_ate_aqui: Option<usize> = None;
        let mut empatados: Vec<usize> = Vec::new();
        for i in 0..candidatos.len() {
            if !empatados.is_empty() {
                let mut e_o_mais = true;
                let mut ha_mais = false;
                for &o in &empatados.clone() {
                    if !self.extensao_mais_especifica(&candidatos[i], &candidatos[o]) {
                        e_o_mais = false;
                    }
                    if self.extensao_mais_especifica(&candidatos[o], &candidatos[i]) {
                        ha_mais = true;
                    }
                }
                if e_o_mais {
                    melhor_ate_aqui = Some(i);
                    empatados.clear();
                } else if !ha_mais {
                    empatados.push(i);
                }
            } else if let Some(b) = melhor_ate_aqui {
                if self.extensao_mais_especifica(&candidatos[b], &candidatos[i]) {
                } else if self.extensao_mais_especifica(&candidatos[i], &candidatos[b]) {
                    melhor_ate_aqui = Some(i);
                } else {
                    empatados.push(b);
                    empatados.push(i);
                    melhor_ate_aqui = None;
                }
            } else {
                melhor_ate_aqui = Some(i);
            }
        }
        let Some(melhor) = melhor_ate_aqui else {
            // Ambíguo: `AMBIGUOUS_EXTENSION_MEMBER_ACCESS` fica pendente
            // para quem tem o nome (`relatar_ambiguidade_de_extensao`).
            let nomes: Vec<String> = empatados
                .iter()
                .map(|&i| {
                    let x = self.program.extension(candidatos[i].0);
                    match x.name {
                        Some(n) => format!("extension '{}'", self.interner.resolve(n)),
                        None => {
                            let on = self.outline.extensions[candidatos[i].0 .0 as usize].on;
                            format!("unnamed extension on '{}'", self.table.format(on, self.interner, self.program))
                        }
                    }
                })
                .collect();
            let lista = match nomes.len() {
                0 | 1 => nomes.join(""),
                2 => format!("{} and {}", nomes[0], nomes[1]),
                n => format!("{}, and {}", nomes[..n - 1].join(", "), nomes[n - 1]),
            };
            // Com exatamente duas, o 3.13.4 escreve as extensões por
            // extenso (`extension E1 on int`; T2, caso c13). Só as nomeadas
            // e sem parâmetros de tipo: a exibição das outras no 3.13.4 não
            // foi conferida, e sem os dois argumentos o texto fica o do 3.6.
            let exibicao = |inf: &Self, i: usize| -> Option<String> {
                let x = inf.program.extension(candidatos[i].0);
                let dados = &inf.outline.extensions[candidatos[i].0 .0 as usize];
                let n = x.name?;
                dados.type_params.is_empty().then(|| {
                    format!("extension {} on {}", inf.interner.resolve(n), inf.table.format(dados.on, inf.interner, inf.program))
                })
            };
            let duas = match empatados[..] {
                [a, b] => exibicao(self, a).zip(exibicao(self, b)),
                _ => None,
            };
            self.ambiguidade_de_extensao = Some((self.interner.resolve(nome).to_string(), lista, duas));
            return None;
        };
        let (e, f, args, _) = candidatos.swap_remove(melhor);
        // `notifyExtensionUsed`: a extensão escolhida usa os imports dela.
        self.body_types.extensoes_usadas.insert((lib, e));
        let (t, metodo) = self.tipo_do_membro_declarado(f, setter);
        let dados = self.outline.extensions[e.0 as usize].clone();
        let mapa = self.mapa(&dados.type_params, &args);
        let t = self.subst(t, &mapa);
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, tipo: t, metodo, funcao: Some(f), de_extensao: true })
    }

    /// `_isMoreSpecific` (`extension_member_resolver.dart:382-418`): fora da
    /// plataforma vence a da plataforma; senão o tipo estendido instanciado
    /// é subtipo do outro e não o inverso, ou, empatando, o mesmo para os
    /// tipos estendidos instanciados para os limites.
    fn extensao_mais_especifica(
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
        let la = self.on_nos_limites(a.0);
        let lb = self.on_nos_limites(b.0);
        self.sub(la, lb) && !self.sub(lb, la)
    }

    /// O tipo estendido de `e` instanciado para os limites.
    fn on_nos_limites(&mut self, e: ExtensionId) -> TypeId {
        let dados = self.outline.extensions[e.0 as usize].clone();
        let args = self.instanciar_para_limites(&dados.type_params);
        let mapa = self.mapa(&dados.type_params, &args);
        self.subst(dados.on, &mapa)
    }

    /// Relata, no nome, a ambiguidade de extensão deixada pela última busca
    /// (`AMBIGUOUS_EXTENSION_MEMBER_ACCESS`,
    /// `extension_member_resolver.dart:115-128`).
    pub(crate) fn relatar_ambiguidade_de_extensao(&mut self, nome: dartforge_diagnostics::Span) {
        if let Some((n, lista, duas)) = self.ambiguidade_de_extensao.take() {
            let codigo = dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_EXTENSION_MEMBER_ACCESS;
            match duas {
                Some((a, b)) => self.aviso_com_codigo(codigo, nome, &[&n, &lista, &a, &b]),
                None => self.aviso_com_codigo(codigo, nome, &[&n, &lista]),
            }
        }
    }

    /// Membro de instância da extensão `e` já instanciada (`E(x).m`,
    /// sobreposição explícita, R-EXT-02): só essa extensão é consultada.
    pub(crate) fn membro_de_extensao_explicita(&mut self, e: ExtensionId, args: &[TypeId], nome: SymbolId, setter: bool) -> Option<Membro> {
        let chave = if setter { self.chave_setter(nome)? } else { nome };
        let &f = self.program.extension(e).instance_members.get(&chave)?;
        let (t, metodo) = self.tipo_do_membro_declarado(f, setter);
        let dados = self.outline.extensions[e.0 as usize].clone();
        let t = if dados.type_params.len() == args.len() {
            let mapa = self.mapa(&dados.type_params, args);
            self.subst(t, &mapa)
        } else {
            t
        };
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, tipo: t, metodo, funcao: Some(f), de_extensao: true })
    }

    /// `lookupStaticGetter` (e, sem ele, `lookupStaticMethod`) ou
    /// `lookupStaticSetter` de `classe`
    /// (an362:src/dart/element/element.dart:5378-5403): o primeiro membro
    /// estático acessível em `lib` na cadeia de implementações (a classe, os
    /// mixins do último ao primeiro, a superclasse). `(classe que o declara,
    /// é método)`.
    pub(crate) fn recuperacao_estatica(&mut self, lib: LibraryId, classe: ClassId, nome: SymbolId, setter: bool) -> Option<(ClassId, bool)> {
        let cadeia = crate::heranca::Heranca::cadeia_de_implementacoes(self, classe);
        let privado = self.interner.resolve(nome).starts_with('_');
        let acessiveis: Vec<ClassId> = cadeia.into_iter().filter(|&k| !privado || self.program.class(k).library == lib).collect();
        if setter {
            let chave = self.chave_setter(nome)?;
            return acessiveis.into_iter().find_map(|k| {
                let &f = self.program.class(k).static_members.get(&chave)?;
                matches!(self.program.function(f).kind, FunctionKind::Setter | FunctionKind::ImplicitAccessor).then_some((k, false))
            });
        }
        let getter = acessiveis.iter().copied().find(|&k| {
            let ke = self.program.class(k);
            ke.enum_constants.iter().any(|&v| self.program.variable(v).name == nome)
                || ke.static_members.get(&nome).is_some_and(|&f| matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor))
        });
        if let Some(k) = getter {
            return Some((k, false));
        }
        acessiveis.into_iter().find(|&k| {
            self.program.class(k).static_members.get(&nome).is_some_and(|&f| {
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
    pub(crate) fn recuperacao_estatica_do_receptor(&mut self, lib: LibraryId, recv: TypeId, nome: SymbolId, setter: bool) -> Option<(ClassId, bool)> {
        let objeto = self.core.object_class;
        let mut classes = Vec::new();
        if self.e_anulavel(recv) {
            classes.extend(objeto);
        }
        classes.extend(self.classe_do_limite(recv));
        classes.extend(objeto);
        classes.into_iter().find_map(|c| self.recuperacao_estatica(lib, c, nome, setter))
    }

    /// A classe de `resolveToBound(recv)` quando ele é um tipo de interface
    /// (classe, enum, mixin ou tipo de extensão), sem o `?`.
    fn classe_do_limite(&mut self, recv: TypeId) -> Option<ClassId> {
        let mut t = self.nao_nulo(recv);
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
            t = self.nao_nulo(t);
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
    pub(crate) fn acesso_de_instancia_a_estatico(&mut self, lib: LibraryId, recv: TypeId, nome: SymbolId, setter: bool, span: dartforge_diagnostics::Span) -> Option<Membro> {
        let (dono, metodo) = self.recuperacao_estatica_do_receptor(lib, recv, nome, setter)?;
        let especie = if setter {
            "setter"
        } else if metodo {
            "method"
        } else {
            "getter"
        };
        let c = self.program.class(dono);
        let tipo_do_dono = match c.kind {
            dartforge_elements::model::ClassKind::Mixin => "mixin",
            dartforge_elements::model::ClassKind::Enum => "enum",
            dartforge_elements::model::ClassKind::ExtensionType => "extension type",
            _ => "class",
        };
        let texto = self.interner.resolve(nome).to_string();
        let classe = self.interner.resolve(c.name).to_string();
        self.aviso_com_codigo(
            dartforge_diagnostics::codigos::compile_time_error::INSTANCE_ACCESS_TO_STATIC_MEMBER,
            span,
            &[&texto, especie, &classe, tipo_do_dono],
        );
        self.membro_estatico(dono, nome, setter)
    }

    /// Membro estático de uma classe (literal de classe como receptor):
    /// estáticos declarados, constantes de enum e tear-off de construtor.
    pub(crate) fn membro_estatico(&mut self, classe: ClassId, nome: SymbolId, setter: bool) -> Option<Membro> {
        let c = self.program.class(classe);
        if !setter {
            if let Some(&v) = c.enum_constants.iter().find(|&&v| self.program.variable(v).name == nome) {
                // O tipo da constante (`E<int>` em `a<int>()`), não o `E<T>`.
                let t = self.tipo_variavel(v);
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
