//! Resolução de anotações de tipo do outline para [`TypeId`].
//!
//! Percorre todo o [`Program`] — classes, mixins, enums, extensions, extension types,
//! typedefs, funções, métodos, getters, setters, construtores e variáveis/campos —
//! e resolve toda anotação de tipo escrita (`TypeAnnotation`) para seu respectivo [`TypeId`].
//!
//! Suporta escopos aninhados (parâmetros de tipo locais sombreiam a biblioteca),
//! expansão transparente de `typedef`, *override inference* de membros herdados
//! sem anotação explícita e acumulação de diagnósticos sem parada prematura.

use crate::hierarchy::{ClassHierarchy, build_class_hierarchy};
use crate::ops::{nullable, substitute};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeParamOwner, TypeTable, Variance};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElement, FunctionElementId, FunctionKind,
    FunctionRef, LibraryId, Program, TypedefId, UnitId, VariableRef,
};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

type ParameterComponents = (Vec<TypeId>, Vec<TypeId>, Vec<(SymbolId, TypeId, bool)>);

/// Dados de tipo de uma função/método/construtor no outline.
#[derive(Debug, Clone)]
pub struct FunctionTypeData {
    /// Assinatura completa em forma de tipo de função (`Type::Function`).
    pub signature: TypeId,
    /// Tipo de retorno.
    pub return_type: TypeId,
    /// Parâmetros de tipo genéricos declarados nesta função.
    pub type_params: Box<[TypeParamId]>,
    /// Metadados e tipos de cada parâmetro formal.
    pub parameters: Box<[ParameterTypeData]>,
}

/// Metadados de um parâmetro formal de função.
#[derive(Debug, Clone)]
pub struct ParameterTypeData {
    /// Nome local (o do escopo do corpo e, em `this.x`, o do campo).
    pub name: Option<SymbolId>,
    /// Nome externo: o da assinatura e da chamada. Difere de `name` só num
    /// nomeado privado da 3.12 (`{this._x}` é passado como `x:`).
    pub externo: Option<SymbolId>,
    pub ty: TypeId,
    pub required: bool,
    pub kind: ParameterKind,
}

/// Dados de tipo de uma variável de topo ou campo.
#[derive(Debug, Clone)]
pub struct VariableTypeData {
    /// Tipo explicitamente anotado na declaração (se houver).
    pub declared_type: Option<TypeId>,
    /// Tipo resolvido / inferido (no outline, `None` se depender do inicializador).
    pub inferred: Option<TypeId>,
}

/// Dados de tipos anotados em uma classe, mixin, enum ou extension type.
#[derive(Debug, Clone)]
pub struct ClassTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub supertype: Option<TypeId>,
    pub mixins: Box<[TypeId]>,
    pub interfaces: Box<[TypeId]>,
    pub on: Box<[TypeId]>,
}

/// Dados de tipos anotados em uma extensão.
#[derive(Debug, Clone)]
pub struct ExtensionTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub on: TypeId,
}

impl OutlineTypes {
    /// Troca os `ast::TypeId` de `u` em [`OutlineTypes::tipos_escritos`] pelo
    /// `mapa` (a troca de texto de uma unidade sem recarga,
    /// `dartforge_elements::incremental`). Recusa, sem mudar nada, se alguma
    /// chave da unidade não está no mapa.
    pub fn remapear_tipos_escritos(&mut self, u: UnitId, mapa: &HashMap<ast::TypeId, ast::TypeId>) -> bool {
        let da_unidade: Vec<((UnitId, ast::TypeId), TypeId)> = self.tipos_escritos.iter().filter(|((un, _), _)| *un == u).map(|(k, v)| (*k, *v)).collect();
        if da_unidade.iter().any(|((_, t), _)| !mapa.contains_key(t)) {
            return false;
        }
        for (k, _) in &da_unidade {
            self.tipos_escritos.remove(k);
        }
        for ((un, t), v) in da_unidade {
            self.tipos_escritos.insert((un, mapa[&t]), v);
        }
        true
    }
}

/// Dados de tipos de um typedef.
#[derive(Debug, Clone)]
pub struct TypedefTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub target_type: TypeId,
}

/// Conjunto de todas as tabelas laterais de tipos do outline, indexadas pelos IDs dos elementos.
#[derive(Debug, Clone)]
pub struct OutlineTypes {
    pub functions: Vec<FunctionTypeData>,
    pub variables: Vec<VariableTypeData>,
    pub classes: Vec<ClassTypeData>,
    pub extensions: Vec<ExtensionTypeData>,
    pub typedefs: Vec<TypedefTypeData>,
    pub hierarchy: ClassHierarchy,
    /// Sobrescritas cujo tipo herdado vem de um campo sem tipo escrito
    /// (`var lista = [1.5];`): o tipo do campo só existe depois da inferência
    /// do inicializador, e a dos corpos completa a assinatura antes de tudo.
    pub sobrescritas_de_campo: Vec<SobrescritaDeCampo>,
    /// O tipo de cada anotação de tipo que o outline resolveu (assinaturas,
    /// cláusulas, limites, e as anotações dentro delas), por unidade e nó:
    /// o `TypeAnnotation.type` do analyzer. As dos corpos ficam em
    /// [`crate::resolved::UnitBodyTypes::tipos_de_anotacoes`].
    pub tipos_escritos: HashMap<(UnitId, ast::TypeId), TypeId>,
    /// A inferência de sobrescrita fiel (`InstanceMemberInferrer`) já rodou
    /// sobre estes tipos (na primeira inferência de corpos).
    pub sobrescritas_inferidas: bool,
}

/// Parte omitida de uma sobrescrita que herda o tipo de um campo sem tipo
/// escrito no supertipo (override inference sobre tipo inferido).
#[derive(Debug, Clone)]
pub struct SobrescritaDeCampo {
    /// O membro sobrescritor (getter, setter ou método).
    pub funcao: FunctionElementId,
    /// O campo sobrescrito.
    pub campo: dartforge_elements::model::VariableId,
    /// Parâmetros de tipo da classe do campo -> argumentos vistos da classe
    /// do sobrescritor.
    pub subst: Vec<(TypeParamId, TypeId)>,
    /// `None`: o retorno; `Some(i)`: o i-ésimo parâmetro.
    pub parametro: Option<usize>,
}

/// Onde um nome de tipo aparece: o `parent` do `NamedType` que o
/// `NamedTypeResolver` consulta para escolher o código de um nome que não
/// resolve para tipo (`an611:src/dart/resolver/named_type_resolver.dart:515-643`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextoDeTipo {
    /// Anotação comum (variável, parâmetro, retorno, limite…).
    #[default]
    Normal,
    /// Elemento de uma lista de argumentos de tipo (`List<X>`, `f<X>()`).
    ArgumentoDeTipo,
    /// O tipo de `on T` de um `catch`.
    Catch,
    /// O tipo de `e as T`.
    As,
    /// O tipo de `e is T` / `e is! T`.
    Is,
    /// Um tipo de `extends`, `implements` ou `with` (de classe, enum, mixin,
    /// alias de classe ou tipo de extensão): o `reportNullOrNonTypeElement`
    /// não relata nome indefinido nem nome que não é tipo ali ("The error
    /// will be reported elsewhere", `named_type_resolver.dart:601-608`), só o
    /// `boolean`, que vem antes.
    Hierarquia,
}

/// O código e os argumentos do analyzer para um nome de tipo que não
/// resolve para tipo (`_ErrorHelper.reportNullOrNonTypeElement`,
/// `an611:src/dart/resolver/named_type_resolver.dart:515-643`): `achou` é a
/// busca ter encontrado um elemento (que não é tipo). As cláusulas de herança
/// e a criação de instância não passam por aqui (outros códigos).
pub fn codigo_de_nome_de_tipo(contexto: ContextoDeTipo, achou: bool, nome: &str) -> dartforge_diagnostics::Codigo {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    if nome == "boolean" {
        return c::UNDEFINED_CLASS_BOOLEAN;
    }
    match contexto {
        ContextoDeTipo::Catch => c::NON_TYPE_IN_CATCH_CLAUSE,
        ContextoDeTipo::As => c::CAST_TO_NON_TYPE,
        ContextoDeTipo::Is if achou => c::TYPE_TEST_WITH_NON_TYPE,
        ContextoDeTipo::Is => c::TYPE_TEST_WITH_UNDEFINED_NAME,
        ContextoDeTipo::ArgumentoDeTipo => c::NON_TYPE_AS_TYPE_ARGUMENT,
        ContextoDeTipo::Normal if achou => c::NOT_A_TYPE,
        ContextoDeTipo::Normal if nome == "await" => c::UNDEFINED_IDENTIFIER_AWAIT,
        ContextoDeTipo::Normal | ContextoDeTipo::Hierarquia => c::UNDEFINED_CLASS,
    }
}

/// O diagnóstico de [`codigo_de_nome_de_tipo`] no nome (`faixa`: do prefixo,
/// se houver, ao fim do nome — o `_getErrorRange`).
pub fn diagnostico_de_nome_de_tipo(contexto: ContextoDeTipo, achou: bool, nome: &str, faixa: dartforge_diagnostics::Span) -> Diagnostic {
    let codigo = codigo_de_nome_de_tipo(contexto, achou, nome);
    if codigo == dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_IDENTIFIER_AWAIT {
        return Diagnostic::com_codigo(codigo, faixa, std::iter::empty::<&str>());
    }
    Diagnostic::com_codigo(codigo, faixa, [nome])
}

/// `WRONG_NUMBER_OF_TYPE_ARGUMENTS` (`_buildTypeArguments`,
/// `an611:src/dart/resolver/named_type_resolver.dart:136-150`): no tipo
/// inteiro, com o nome, o número de parâmetros e o de argumentos.
pub fn diagnostico_de_argumentos_de_tipo(nome: &str, parametros: usize, argumentos: usize, span: dartforge_diagnostics::Span) -> Diagnostic {
    Diagnostic::com_codigo(
        dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS,
        span,
        [nome.to_string(), parametros.to_string(), argumentos.to_string()],
    )
}

/// O parâmetro de tipo é de uma classe, mixin, enum ou extensão.
pub(crate) fn param_da_classe(table: &TypeTable, p: TypeParamId) -> bool {
    matches!(table.param(p).owner, TypeParamOwner::Class(_) | TypeParamOwner::Extension(_))
}

/// `TYPE_PARAMETER_REFERENCED_BY_STATIC` (`_checkForTypeParameterReferencedByStatic`,
/// `an611:src/generated/error_verifier.dart:5419-5434`): um parâmetro de tipo
/// da classe citado num método ou campo estático, no nome.
pub fn diagnostico_de_parametro_em_estatico(faixa: dartforge_diagnostics::Span) -> Diagnostic {
    Diagnostic::com_codigo(
        dartforge_diagnostics::codigos::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC,
        faixa,
        std::iter::empty::<&str>(),
    )
}

/// O nome `sym` no escopo de instância de um contêiner (classe, mixin,
/// enum, extension type ou extensão): os membros **declarados** (getters,
/// inclusive os implícitos de campos e de constantes de enum, métodos,
/// setters), estáticos ou não (`an611:src/dart/element/scope.dart:273-279`,
/// `ExtensionScope` em `:92-100`). O `values` sintético de um enum conta (é
/// um campo estático declarado pelo analyzer); o `index` sintético não (é
/// herdado de `Enum`).
pub(crate) fn nome_no_conteiner(
    program: &Program,
    interner: &Interner,
    classe: Option<ClassId>,
    extensao: Option<ExtensionId>,
    sym: SymbolId,
) -> Option<NoConteiner> {
    let (instancia, estaticos, campos) = match (classe, extensao) {
        (Some(c), _) => {
            let k = program.class(c);
            (&k.instance_members, &k.static_members, k.fields.iter().chain(&k.enum_constants).copied().collect::<Vec<_>>())
        }
        (None, Some(e)) => {
            let x = &program.extensions[e.0 as usize];
            (&x.instance_members, &x.static_members, x.fields.clone())
        }
        _ => return None,
    };
    let declarado = |f: FunctionElementId| {
        let fe = &program.functions[f.0 as usize];
        !matches!(fe.node, FunctionRef::None) || fe.variable.is_some() || (fe.static_ && interner.resolve(fe.name) == "values")
    };
    let getter = [instancia, estaticos]
        .iter()
        .any(|m| m.get(&sym).is_some_and(|&f| declarado(f) && program.functions[f.0 as usize].kind != FunctionKind::Constructor))
        || campos.iter().any(|v| program.variables[v.0 as usize].name == sym);
    if getter {
        return Some(NoConteiner::Getter);
    }
    let nome_setter = format!("{}_=", interner.resolve(sym));
    let setter = interner
        .lookup(&nome_setter)
        .is_some_and(|k| [instancia, estaticos].iter().any(|m| m.get(&k).is_some_and(|&f| declarado(f))));
    setter.then_some(NoConteiner::SoSetter)
}

/// `NamedTypeResolver.resolve` (`named_type_resolver.dart:89-121`): o
/// primeiro nome de `p.N` busca no escopo do `ResolutionVisitor`; se o
/// elemento achado não é prefixo, classe nem alias, é
/// `PREFIX_SHADOWED_BY_LOCAL_DECLARATION` no prefixo, e o tipo é inválido.
/// O escopo, de dentro para fora: os parâmetros de tipo e os locais
/// (`local`, decidido por quem chama), os membros declarados do contêiner
/// (o `LocalScope` em que o `ResolutionVisitor` define `accessors` e
/// `methods` pelo nome deles: um setter fica sob `x=` e não esconde `x`) e
/// o escopo de topo ([`prefixo_sombreado_no_topo`]).
pub(crate) fn prefixo_sombreado(
    program: &Program,
    interner: &Interner,
    unit: UnitId,
    p: SymbolId,
    local: bool,
    classe: Option<ClassId>,
    extensao: Option<ExtensionId>,
) -> bool {
    local
        || matches!(nome_no_conteiner(program, interner, classe, extensao, p), Some(NoConteiner::Getter))
        || prefixo_sombreado_no_topo(program, unit, p)
}

/// `LibraryFragmentScope.lookup` (`scope.dart:424-440`): as declarações da
/// biblioteca vêm antes dos prefixos e dos imports. Uma função, variável ou
/// extensão (ou um nome ambíguo, o `MultiplyDefinedElement`) com o nome do
/// prefixo o esconde; só setter é `getter` nulo (tipo indefinido, não
/// sombra).
pub(crate) fn prefixo_sombreado_no_topo(program: &Program, unit: UnitId, p: SymbolId) -> bool {
    let lib = program.library(program.unit(unit).library);
    let b = match lib.declared.get(&p) {
        Some(b) => *b,
        None if program.prefixos_na_unidade(unit).contains_key(&p) => return false,
        None => match program.lookup_na_unidade(unit, p) {
            Some(b) => b,
            None => return false,
        },
    };
    b.ambiguous || matches!(b.getter, Some(Element::Function(_) | Element::Variable(_) | Element::Extension(_)))
}

/// `PREFIX_SHADOWED_BY_LOCAL_DECLARATION` no prefixo, com o nome dele.
pub fn diagnostico_de_prefixo_sombreado(nome: &str, faixa: dartforge_diagnostics::Span) -> Diagnostic {
    Diagnostic::com_codigo(dartforge_diagnostics::codigos::compile_time_error::PREFIX_SHADOWED_BY_LOCAL_DECLARATION, faixa, [nome])
}

/// Declaração cujos membros formam um escopo (o `InstanceScope` do analyzer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Conteiner {
    Classe(ClassId),
    Extensao(ExtensionId),
}

/// O que um nome é no escopo de instância de um contêiner.
pub(crate) enum NoConteiner {
    /// Getter (inclusive o implícito de campo e de constante de enum) ou
    /// método: o resultado da busca tem `getter`, e não é tipo.
    Getter,
    /// Só setter: a busca do `InstanceScope` (a do vinculador, que dá o
    /// tipo do elemento) para aqui, sem `getter`: tipo inválido. A do
    /// `ResolutionVisitor` (a que relata) não o vê e segue para fora.
    SoSetter,
}

/// Contexto de resolução com acesso ao programa e acumuladores de estado.
pub struct OutlineResolver<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub diagnostics: Vec<Diagnostic>,
    /// Unidade de cada diagnóstico (paralelo a `diagnostics`): o arquivo em
    /// que ele é relatado, sem depender de adivinhar pelo intervalo.
    pub unidades_dos_avisos: Vec<UnitId>,
    /// O contêiner (classe, mixin, enum, extension type ou extensão) cujos
    /// membros estão em escopo na anotação sendo resolvida: o
    /// `InstanceScope` do analyzer (docs/ANALISADOR-PARIDADE-PLANO.md §3.3).
    conteiner: Option<Conteiner>,
    /// O contexto da anotação que a próxima chamada de `resolve_annotation`
    /// resolve (lido e zerado na entrada; os argumentos de tipo o põem em
    /// [`ContextoDeTipo::ArgumentoDeTipo`]).
    contexto_de_tipo: ContextoDeTipo,
    /// Resolvendo a assinatura de um método ou campo estático (os parâmetros
    /// de tipo da classe não valem ali).
    membro_estatico: bool,
    /// Parâmetros de tipo alocados por classe: `[ClassId] -> Box<[TypeParamId]>`
    pub class_type_params: Vec<Box<[TypeParamId]>>,
    /// Parâmetros de tipo alocados por typedef: `[TypedefId] -> Box<[TypeParamId]>`
    pub typedef_type_params: Vec<Box<[TypeParamId]>>,
    /// Parâmetros de tipo alocados por extension: `[ExtensionId] -> Box<[TypeParamId]>`
    pub extension_type_params: Vec<Box<[TypeParamId]>>,
    /// Tipos alvo expandidos de typedefs: `[TypedefId] -> Option<TypeId>`
    pub typedef_targets: Vec<Option<TypeId>>,
    /// Cache do `hasSelfReference` de cada `typedef` (`auto_referencia`).
    typedefs_auto_referentes: Vec<Option<bool>>,
    /// Ver [`OutlineTypes::sobrescritas_de_campo`].
    pub sobrescritas_de_campo: Vec<SobrescritaDeCampo>,
    /// Ver [`OutlineTypes::tipos_escritos`].
    pub tipos_escritos: HashMap<(UnitId, ast::TypeId), TypeId>,
}

impl<'a> OutlineResolver<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
    ) -> Self {
        let num_classes = program.classes.len();
        let num_typedefs = program.typedefs.len();
        let num_extensions = program.extensions.len();

        Self {
            program,
            interner,
            table,
            core,
            diagnostics: Vec::new(),
            unidades_dos_avisos: Vec::new(),
            conteiner: None,
            contexto_de_tipo: ContextoDeTipo::Normal,
            membro_estatico: false,
            class_type_params: vec![Box::new([]); num_classes],
            typedef_type_params: vec![Box::new([]); num_typedefs],
            extension_type_params: vec![Box::new([]); num_extensions],
            typedef_targets: vec![None; num_typedefs],
            typedefs_auto_referentes: vec![None; num_typedefs],
            sobrescritas_de_campo: Vec::new(),
            tipos_escritos: HashMap::new(),
        }
    }

    /// Registra um diagnóstico relatado em `unidade`.

    /// `reportNullOrNonTypeElement` (named_type_resolver.dart:518-521): o
    /// nome sintético (vazio, da recuperação do parser) não é relatado.
    fn avisar_nome_de_tipo(&mut self, unit_id: UnitId, contexto: ContextoDeTipo, achou: bool, texto: &str, faixa: dartforge_diagnostics::Span) {
        if texto.is_empty() || contexto == ContextoDeTipo::Hierarquia && texto != "boolean" {
            return;
        }
        self.avisar(unit_id, diagnostico_de_nome_de_tipo(contexto, achou, texto, faixa));
    }
    /// O nome `sym` do namespace do `dart:core` está no escopo sem prefixo:
    /// a biblioteca não importa o core explicitamente (o import implícito),
    /// ou o importa sem prefixo com combinadores que o deixam passar.
    fn nucleo_visivel(&self, unidade: UnitId, sym: SymbolId) -> bool {
        let lib = self.program.unit(unidade).library;
        if Some(lib) == self.program.core {
            return true;
        }
        // Como o core implícito do outline (`elements::outline`): só o import
        // da unidade definidora conta (`hasDartCoreImport` do
        // `LibraryFileKind`); o `import 'dart:core' as prefix0;` de uma
        // augmentation não esconde o `dynamic` da biblioteca.
        let principal = self.program.library(lib).units.first().copied();
        let imports = &self.program.library(lib).imports;
        let do_core: Vec<_> = imports.iter().filter(|i| Some(i.library) == self.program.core && Some(i.unit) == principal).collect();
        if do_core.is_empty() {
            return true;
        }
        do_core.iter().any(|i| {
            i.prefix.is_none()
                && i.combinators.iter().all(|c| match c {
                    ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == sym),
                    ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == sym),
                })
        })
    }

    fn avisar(&mut self, unidade: UnitId, d: Diagnostic) {
        self.diagnostics.push(d);
        self.unidades_dos_avisos.push(unidade);
    }

    /// O nome `sym` no escopo de instância do contêiner em resolução
    /// ([`nome_no_conteiner`]).
    fn no_conteiner(&self, sym: SymbolId) -> Option<NoConteiner> {
        let (classe, extensao) = match self.conteiner? {
            Conteiner::Classe(c) => (Some(c), None),
            Conteiner::Extensao(e) => (None, Some(e)),
        };
        nome_no_conteiner(self.program, self.interner, classe, extensao, sym)
    }

    /// Resolve todo o outline do programa, retornando as tabelas laterais e os diagnósticos acumulados.
    pub fn resolve_all(self) -> (OutlineTypes, Vec<Diagnostic>) {
        let (o, d, _) = self.resolve_all_com_unidades();
        (o, d)
    }

    /// Como [`OutlineResolver::resolve_all`], com a unidade de cada diagnóstico.
    pub fn resolve_all_com_unidades(mut self) -> (OutlineTypes, Vec<Diagnostic>, Vec<UnitId>) {
        // 1. Alocar TypeParamIds para classes, typedefs e extensions
        self.allocate_outline_type_params();

        // 2. Resolver typedefs (com suporte a referências mútuas)
        self.resolve_typedefs();

        // 3. Resolver anotações de supertipos das classes e construir a hierarquia instanciada
        let (class_type_data, hierarchy) = self.resolve_classes_and_hierarchy();

        // 4. Resolver extensions
        let extension_type_data = self.resolve_extensions();

        // 5. Resolver variáveis de topo e campos
        let mut variable_type_data = self.resolve_variables();

        // 6. Resolver funções, métodos e construtores (com override inference)
        let function_type_data = self.resolve_functions(&hierarchy, &mut variable_type_data);

        let typedef_type_data: Vec<TypedefTypeData> = self
            .typedef_type_params
            .iter()
            .zip(self.typedef_targets.iter())
            .map(|(params, target)| TypedefTypeData {
                type_params: params.clone(),
                target_type: target.unwrap_or(self.core.dynamic_),
            })
            .collect();

        let outline = OutlineTypes {
            functions: function_type_data,
            variables: variable_type_data,
            classes: class_type_data,
            extensions: extension_type_data,
            typedefs: typedef_type_data,
            hierarchy,
            sobrescritas_de_campo: std::mem::take(&mut self.sobrescritas_de_campo),
            tipos_escritos: std::mem::take(&mut self.tipos_escritos),
            sobrescritas_inferidas: false,
        };

        (outline, self.diagnostics, self.unidades_dos_avisos)
    }

    fn allocate_outline_type_params(&mut self) {
        for (i, class) in self.program.classes.iter().enumerate() {
            let class_id = ClassId(i as u32);
            let mut params = Vec::with_capacity(class.type_params.len());
            // A variância escrita (`class C<out T>`), guardada mesmo com o
            // experimento desligado, como o `ElementBuilder` do analyzer.
            let escritos: &[dartforge_frontend::ast::TypeParameter] = match class.decl {
                Some(d) => match &self.program.unit(d.unit).ast.decl(d.decl).kind {
                    dartforge_frontend::ast::DeclKind::Class(k) => &k.type_params,
                    dartforge_frontend::ast::DeclKind::Mixin(k) => &k.type_params,
                    dartforge_frontend::ast::DeclKind::Enum(k) => &k.type_params,
                    _ => &[],
                },
                None => &[],
            };
            for (j, p) in class.type_params.iter().enumerate() {
                let variance = match escritos.get(j).and_then(|t| t.variance).map(|v| v.0) {
                    Some(dartforge_frontend::ast::Variance::In) => Variance::Contravariant,
                    Some(dartforge_frontend::ast::Variance::Out) => Variance::Covariant,
                    Some(dartforge_frontend::ast::Variance::Inout) => Variance::Invariant,
                    None => Variance::Unspecified,
                };
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Class(class_id),
                    self.core.object_nullable,
                    variance,
                );
                params.push(pid);
            }
            self.class_type_params[i] = params.into_boxed_slice();
        }

        for (i, typedef) in self.program.typedefs.iter().enumerate() {
            let typedef_id = TypedefId(i as u32);
            let mut params = Vec::with_capacity(typedef.type_params.len());
            for p in typedef.type_params.iter() {
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Typedef(typedef_id),
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                params.push(pid);
            }
            self.typedef_type_params[i] = params.into_boxed_slice();
        }

        for (i, ext) in self.program.extensions.iter().enumerate() {
            let ext_id = ExtensionId(i as u32);
            let mut params = Vec::with_capacity(ext.type_params.len());
            for p in ext.type_params.iter() {
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Extension(ext_id),
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                params.push(pid);
            }
            self.extension_type_params[i] = params.into_boxed_slice();
        }

        // Atualizar bounds escritos para classes. Três passadas: um tipo cru
        // no limite (`I extends Restriction`) é instanciado para os limites
        // da outra classe, que podem ainda não ter sido lidos na primeira
        // (sairia `Restriction<Object?>` em vez de `Restriction<Object>`);
        // cadeias de limites crus até três níveis; só a última relata.
        let (n_diag, n_unid) = (self.diagnostics.len(), self.unidades_dos_avisos.len());
        for passada in 0..3 {
            if passada > 0 {
                self.diagnostics.truncate(n_diag);
                self.unidades_dos_avisos.truncate(n_unid);
            }
            self.resolver_limites_de_classes();
        }

        // E os dos typedefs (`typedef F<X extends num> = X Function();`):
        // sem eles, o limite era `Object?` e `F<Object>` passava.
        for (i, typedef) in self.program.typedefs.iter().enumerate() {
            let params = self.typedef_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in typedef.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
            for (p_elem, &pid) in typedef.type_params.iter().zip(params.iter()) {
                if let Some((unit_id, ast_ty_id)) = p_elem.bound {
                    let bound_ty = self.resolve_annotation(unit_id, ast_ty_id, typedef.library, &scope);
                    self.table.set_type_param_bound(pid, bound_ty);
                }
            }
        }
    }

    /// Os limites escritos dos parâmetros de tipo de todas as classes.
    fn resolver_limites_de_classes(&mut self) {
        for (i, class) in self.program.classes.iter().enumerate() {
            let params = self.class_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in class.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
            for (p_elem, &pid) in class.type_params.iter().zip(params.iter()) {
                if let Some((unit_id, ast_ty_id)) = p_elem.bound {
                    let bound_ty = self.resolve_annotation(unit_id, ast_ty_id, class.library, &scope);
                    self.table.set_type_param_bound(pid, bound_ty);
                }
            }
        }
    }

    fn resolve_typedefs(&mut self) {
        for i in 0..self.program.typedefs.len() {
            let typedef_id = TypedefId(i as u32);
            self.ensure_typedef_resolved(typedef_id);
        }
    }

    /// O `typedef` chega a si mesmo (`hasSelfReference`): instancia como
    /// `dynamic` (`TypeAliasElementImpl.instantiate`, `element.dart:9690-9696`).
    fn typedef_auto_referente(&mut self, id: TypedefId) -> bool {
        let i = id.0 as usize;
        if let Some(r) = self.typedefs_auto_referentes[i] {
            return r;
        }
        let r = crate::auto_referencia::typedef_auto_referente(self.program, self.program.typedefs[i].decl);
        self.typedefs_auto_referentes[i] = Some(r);
        r
    }

    fn ensure_typedef_resolved(&mut self, id: TypedefId) -> TypeId {
        let idx = id.0 as usize;
        if let Some(target) = self.typedef_targets[idx] {
            return target;
        }

        let typedef = &self.program.typedefs[idx];
        let params = self.typedef_type_params[idx].clone();
        let mut scope = HashMap::with_capacity(params.len());
        for (p_elem, &pid) in typedef.type_params.iter().zip(params.iter()) {
            scope.insert(p_elem.name, pid);
        }

        // Define fallback temporário para evitar ciclos
        self.typedef_targets[idx] = Some(self.core.dynamic_);

        let unit_id = typedef.decl.unit;
        let decl = self.program.unit(unit_id).ast.decl(typedef.decl.decl);
        let resolved_target = match &decl.kind {
            DeclKind::Typedef(d) => match &d.kind {
                ast::TypedefKind::Alias(ast_ty) => {
                    self.resolve_annotation(unit_id, *ast_ty, typedef.library, &scope)
                }
                ast::TypedefKind::Legacy {
                    return_type,
                    parameters,
                } => {
                    let ret = if let Some(r) = return_type {
                        self.resolve_annotation(unit_id, *r, typedef.library, &scope)
                    } else {
                        self.core.dynamic_
                    };
                    let (pos, opt, named) = self.resolve_ast_parameter_types(
                        unit_id,
                        parameters,
                        typedef.library,
                        &scope,
                    );
                    self.table.intern(Type::Function {
                        type_params: Box::new([]),
                        ret,
                        positional: pos.into_boxed_slice(),
                        optional: opt.into_boxed_slice(),
                        named: named.into_boxed_slice(),
                        nullable: false,
                    })
                }
            },
            _ => self.core.dynamic_,
        };

        self.typedef_targets[idx] = Some(resolved_target);
        resolved_target
    }

    fn resolve_classes_and_hierarchy(&mut self) -> (Vec<ClassTypeData>, ClassHierarchy) {
        let mut class_type_data = Vec::with_capacity(self.program.classes.len());
        let mut hierarchy_inputs: Vec<Option<crate::hierarchy::ImmediateSupertypeInput>> =
            Vec::with_capacity(self.program.classes.len());

        let enum_type = self.program.core.and_then(|core| {
            let sym = self.interner.lookup("Enum")?;
            match self.program.lookup(core, sym)?.getter {
                Some(dartforge_elements::model::Element::Class(c)) => {
                    Some(self.table.intern(Type::Interface { class: c, args: Box::new([]), nullable: false }))
                }
                _ => None,
            }
        });
        for (i, class) in self.program.classes.iter().enumerate() {
            let params = self.class_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in class.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }

            let supertype = class.supertype.map(|(unit, ast_id)| {
                self.contexto_de_tipo = ContextoDeTipo::Hierarquia;
                self.resolve_annotation(unit, ast_id, class.library, &scope)
            });

            let mixins: Vec<TypeId> = class
                .mixins
                .iter()
                .map(|&(unit, ast_id)| {
                    self.contexto_de_tipo = ContextoDeTipo::Hierarquia;
                    self.resolve_annotation(unit, ast_id, class.library, &scope)
                })
                .collect();

            let interfaces: Vec<TypeId> = class
                .interfaces
                .iter()
                .map(|&(unit, ast_id)| {
                    self.contexto_de_tipo = ContextoDeTipo::Hierarquia;
                    self.resolve_annotation(unit, ast_id, class.library, &scope)
                })
                .collect();

            let on: Vec<TypeId> = class
                .on
                .iter()
                .map(|&(unit, ast_id)| self.resolve_annotation(unit, ast_id, class.library, &scope))
                .collect();

            let mut all_direct = Vec::new();
            if let Some(s) = supertype {
                all_direct.push(s);
            }
            all_direct.extend_from_slice(&mixins);
            all_direct.extend_from_slice(&interfaces);
            all_direct.extend_from_slice(&on);
            // Toda declaração `enum` é subtipo de `Enum` (§14.1: a classe
            // estende implicitamente `_Enum`, que implementa `Enum`), mesmo
            // sem superclasse escrita: `Cor <: Enum` decide os limites de
            // `extension EnumByName<T extends Enum> on Iterable<T>`.
            if class.kind == dartforge_elements::model::ClassKind::Enum {
                if let Some(e) = enum_type {
                    all_direct.push(e);
                }
            }

            hierarchy_inputs.push(Some((params.clone(), all_direct)));

            class_type_data.push(ClassTypeData {
                type_params: params,
                supertype,
                mixins: mixins.into_boxed_slice(),
                interfaces: interfaces.into_boxed_slice(),
                on: on.into_boxed_slice(),
            });
        }

        // T1.1 d: a hierarquia de um homônimo é a do dono do grupo (o
        // `ClassHierarchy._map` do analyzer é por elemento, e os homônimos
        // são o mesmo elemento como chave). A identidade do tipo continua a
        // de cada classe.
        for i in 0..hierarchy_inputs.len() {
            let dono = self.program.dono_da_classe(ClassId(i as u32));
            if dono.0 as usize != i && (dono.0 as usize) < hierarchy_inputs.len() {
                hierarchy_inputs[i] = hierarchy_inputs[dono.0 as usize].clone();
            }
        }
        let mut hierarchy = build_class_hierarchy(
            self.program.classes.len(),
            &hierarchy_inputs,
            self.table,
            self.core,
        );
        // `_MixinInference` (3.6.2 `types_builder.dart:461-605`): o mixin
        // genérico escrito sem argumentos (`with M`) tem os argumentos
        // inferidos das restrições dele (`on` do mixin; a superclasse e os
        // mixins da classe usada como mixin, sem o último de um alias de
        // classe) casadas com os supertipos já aplicados (a superclasse e os
        // mixins anteriores, com os supertipos deles: `InterfacesMerger`),
        // por `matchSupertypeConstraints` (`type_system.dart:1448-1487`).
        // Sem casamento, fica o tipo cru. Com mudança, a hierarquia é refeita
        // e a passada se repete (a superclasse pode ter mixins inferidos).
        for _ in 0..4 {
            let mut mudou = false;
            for i in 0..self.program.classes.len() {
                for j in 0..self.program.classes[i].mixins.len() {
                    if let Some(novo) = self.inferir_mixin(&class_type_data, &hierarchy, i, j) {
                        class_type_data[i].mixins[j] = novo;
                        mudou = true;
                    }
                }
            }
            if !mudou {
                break;
            }
            for i in 0..self.program.classes.len() {
                let d = &class_type_data[i];
                let mut todos = Vec::new();
                todos.extend(d.supertype);
                todos.extend_from_slice(&d.mixins);
                todos.extend_from_slice(&d.interfaces);
                todos.extend_from_slice(&d.on);
                if self.program.classes[i].kind == dartforge_elements::model::ClassKind::Enum {
                    if let Some(e) = enum_type {
                        todos.push(e);
                    }
                }
                hierarchy_inputs[i] = Some((d.type_params.clone(), todos));
            }
            for i in 0..hierarchy_inputs.len() {
                let dono = self.program.dono_da_classe(ClassId(i as u32));
                if dono.0 as usize != i && (dono.0 as usize) < hierarchy_inputs.len() {
                    hierarchy_inputs[i] = hierarchy_inputs[dono.0 as usize].clone();
                }
            }
            hierarchy = build_class_hierarchy(self.program.classes.len(), &hierarchy_inputs, self.table, self.core);
        }
        // O tipo de extensão é subtipo de `Object` só se alguma
        // superinterface (o `implements`) o for: uma classe, ou outro tipo
        // de extensão que o seja.
        let mut memo: HashMap<usize, bool> = HashMap::new();
        fn com_object(
            i: usize,
            dados: &[ClassTypeData],
            program: &Program,
            table: &TypeTable,
            memo: &mut HashMap<usize, bool>,
            prof: u32,
        ) -> bool {
            if program.classes[i].kind != ClassKind::ExtensionType {
                return true;
            }
            if let Some(&r) = memo.get(&i) {
                return r;
            }
            if prof > 64 {
                return true;
            }
            memo.insert(i, true);
            let r = dados[i].interfaces.iter().any(|&t| match table.get(t) {
                Type::Interface { .. } => true,
                Type::ExtensionType { decl, .. } => com_object(decl.0 as usize, dados, program, table, memo, prof + 1),
                _ => false,
            });
            memo.insert(i, r);
            r
        }
        for i in 0..self.program.classes.len() {
            if self.program.classes[i].kind == ClassKind::ExtensionType
                && !com_object(i, &class_type_data, self.program, self.table, &mut memo, 0)
            {
                hierarchy.extensoes_sem_object.insert(ClassId(i as u32));
            }
        }

        (class_type_data, hierarchy)
    }

    /// `_MixinInference._inferSingle` para o mixin `j` da classe `i`: o novo
    /// tipo, quando a inferência o muda.
    fn inferir_mixin(&mut self, dados: &[ClassTypeData], hierarquia: &ClassHierarchy, i: usize, j: usize) -> Option<TypeId> {
        let (unit, ast_id) = self.program.classes[i].mixins[j];
        let sem_args = matches!(&self.program.unit(unit).ast.ty(ast_id).kind, ast::TypeKind::Named { args, .. } if args.is_empty());
        if !sem_args {
            return None;
        }
        let mt = *dados[i].mixins.get(j)?;
        let Type::Interface { class: m, nullable, .. } = self.table.get(mt).clone() else { return None };
        let params = dados[m.0 as usize].type_params.clone();
        if params.is_empty() {
            return None;
        }
        // `gatherMixinSupertypeConstraintsForInference` (`type_system.dart:500-518`).
        let mk = &self.program.classes[m.0 as usize];
        let candidatos: Vec<TypeId> = if mk.kind == dartforge_elements::model::ClassKind::Mixin {
            dados[m.0 as usize].on.to_vec()
        } else {
            let s = dados[m.0 as usize].supertype?;
            let mut v = vec![s];
            v.extend(dados[m.0 as usize].mixins.iter().copied());
            let alias = mk.decl.is_some_and(|d| {
                matches!(&self.program.unit(d.unit).ast.decl(d.decl).kind, DeclKind::Class(x) if x.mixin_application)
            });
            if alias {
                v.pop();
            }
            v
        };
        let restricoes: Vec<(TypeId, ClassId)> = candidatos
            .into_iter()
            .filter_map(|r| match self.table.get(r) {
                Type::Interface { class, .. } if !dados[class.0 as usize].type_params.is_empty() => Some((r, *class)),
                _ => None,
            })
            .collect();
        // `InterfacesMerger.typeList` da superclasse e dos mixins anteriores,
        // por elemento como o `_ClassInterfaceType.update`
        // (`class_hierarchy.dart:170-196`): o primeiro tipo; outro diferente
        // passa a juntar os normalizados pelo `topMerge`, e o que falha deixa
        // o resultado anterior.
        let fontes: Vec<TypeId> = dados[i].supertype.into_iter().chain(dados[i].mixins[..j].iter().copied()).collect();
        let mut alvos: Vec<TypeId> = Vec::new();
        for &(_, rc) in restricoes.iter() {
            let (mut unico, mut atual, mut erro): (Option<TypeId>, Option<TypeId>, bool) = (None, None, false);
            for &f in fontes.iter() {
                let Some(t) = hierarquia.supertype_of(f, rc, self.table, self.core) else { continue };
                if erro {
                    continue;
                }
                if atual.is_none() {
                    match unico {
                        None => {
                            unico = Some(t);
                            continue;
                        }
                        Some(u) if u == t => continue,
                        Some(u) => atual = Some(crate::ops::normalize(u, self.table, self.core)),
                    }
                }
                let nt = crate::ops::normalize(t, self.table, self.core);
                match crate::ops::top_merge(self.table, self.core, atual.expect("posto acima"), nt) {
                    Some(r) => atual = Some(r),
                    None => erro = true,
                }
            }
            alvos.push(atual.or(unico)?);
        }
        let mut gi = crate::constraints::GenericInferrer::new(&params);
        let tipos = {
            let mut env = crate::subtyping::SubtypeEnv::new(self.table, hierarquia, self.core);
            for (k, &(src, _)) in restricoes.iter().enumerate() {
                gi.constrain_return(src, alvos[k], &mut env);
                gi.constrain_return(alvos[k], src, &mut env);
            }
            gi.choose_final(&mut env)
        };
        let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(tipos.iter().copied()).collect();
        for (k, &(src, _)) in restricoes.iter().enumerate() {
            if substitute(src, &mapa, self.table) != alvos[k] {
                return None;
            }
        }
        let novo = self.table.intern(Type::Interface { class: m, args: tipos.into_boxed_slice(), nullable });
        (novo != mt).then_some(novo)
    }

    fn resolve_extensions(&mut self) -> Vec<ExtensionTypeData> {
        let mut data = Vec::with_capacity(self.program.extensions.len());
        for (i, ext) in self.program.extensions.iter().enumerate() {
            let params = self.extension_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in ext.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
            // Limites escritos (`extension X<T extends Enum> on ...`).
            for (p_elem, &pid) in ext.type_params.iter().zip(params.iter()) {
                if let Some((unit_id, ast_ty_id)) = p_elem.bound {
                    let b = self.resolve_annotation(unit_id, ast_ty_id, ext.library, &scope);
                    self.table.set_type_param_bound(pid, b);
                }
            }
            let on_ty = self.resolve_annotation(ext.on.0, ext.on.1, ext.library, &scope);
            data.push(ExtensionTypeData {
                type_params: params,
                on: on_ty,
            });
        }
        data
    }

    fn resolve_variables(&mut self) -> Vec<VariableTypeData> {
        let mut data = Vec::with_capacity(self.program.variables.len());
        for var in self.program.variables.iter() {
            let (declared, inferred) = match var.node {
                VariableRef::TopLevel { unit, decl, .. } => {
                    let decl_node = self.program.unit(unit).ast.decl(decl);
                    if let DeclKind::Variables(var_list) = &decl_node.kind {
                        if let Some(ast_ty) = var_list.ty {
                            let ty =
                                self.resolve_annotation(unit, ast_ty, var.library, &HashMap::new());
                            (Some(ty), Some(ty))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                }
                VariableRef::Field { unit, member, .. } => {
                    let mem_node = &self.program.unit(unit).ast.members[member.0 as usize];
                    if let MemberKind::Field(var_list) = &mem_node.kind {
                        let scope = self.get_enclosing_type_param_scope(var.class, var.extension);
                        if let Some(ast_ty) = var_list.ty {
                            self.conteiner = conteiner_de(var.class, var.extension);
                            self.membro_estatico = var.static_;
                            let ty = self.resolve_annotation(unit, ast_ty, var.library, &scope);
                            self.conteiner = None;
                            self.membro_estatico = false;
                            (Some(ty), Some(ty))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                }
                VariableRef::EnumConstant { unit, decl, index } => {
                    if let Some(cls) = var.class {
                        // Enum genérico: `a<int>()` dá `E<int>`; sem argumentos
                        // escritos, os limites (a inferência pelos argumentos do
                        // construtor fica de fora). Sem isto o tipo saía `E`
                        // sem argumentos e o UP de duas constantes vazava `E<T>`.
                        let n = self.class_type_params[cls.0 as usize].len();
                        let escritos: Vec<ast::TypeId> = match &self.program.unit(unit).ast.decl(decl).kind {
                            DeclKind::Enum(en) => en.constants.get(index).map(|k| k.type_args.to_vec()).unwrap_or_default(),
                            _ => Vec::new(),
                        };
                        let com_argumentos = match &self.program.unit(unit).ast.decl(decl).kind {
                            DeclKind::Enum(en) => en.constants.get(index).is_some_and(|k| k.arguments.is_some()),
                            _ => false,
                        };
                        // A constante de enum é implicitamente tipada: o tipo
                        // é o da criação `E(args)` inferida (os argumentos de
                        // tipo pelos argumentos, `enum E<T> { v('') }` dá
                        // `E<String>`). Genérica, sem argumentos de tipo
                        // escritos e com argumentos, fica para a inferência
                        // (`funcoes.rs::inferir_tipo_de_variavel_sem_tipo`).
                        if n > 0 && escritos.is_empty() && com_argumentos {
                            (None, None)
                        } else {
                        let args: Vec<TypeId> = if n == 0 {
                            Vec::new()
                        } else if escritos.len() == n {
                            escritos.iter().map(|&t| self.resolve_annotation(unit, t, var.library, &HashMap::new())).collect()
                        } else {
                            let formals = self.class_type_params[cls.0 as usize].clone();
                            self.instanciar_para_limites(&formals)
                        };
                        let ty = self.table.intern(Type::Interface {
                            class: cls,
                            args: args.into_boxed_slice(),
                            nullable: false,
                        });
                        (Some(ty), Some(ty))
                        }
                    } else {
                        (None, None)
                    }
                }
                VariableRef::Representation { unit, decl } => {
                    let decl_node = self.program.unit(unit).ast.decl(decl);
                    if let DeclKind::ExtensionType(ext) = &decl_node.kind {
                        // A representação é resolvida antes do escopo de
                        // instância (`an611:src/summary2/reference_resolver.dart:204-207`).
                        let scope = self.get_enclosing_type_param_scope(var.class, None);
                        let ty = self.resolve_annotation(
                            unit,
                            ext.representation_type,
                            var.library,
                            &scope,
                        );
                        (Some(ty), Some(ty))
                    } else {
                        (None, None)
                    }
                }
                VariableRef::None => (None, None),
            };

            data.push(VariableTypeData {
                declared_type: declared,
                inferred,
            });
        }
        data
    }

    fn resolve_functions(
        &mut self,
        hierarchy: &ClassHierarchy,
        variables: &mut [VariableTypeData],
    ) -> Vec<FunctionTypeData> {
        let mut data: Vec<FunctionTypeData> = Vec::with_capacity(self.program.functions.len());
        let augmentadas = self.declaracoes_augmentadas();

        for (i, func) in self.program.functions.iter().enumerate() {
            let func_id = FunctionElementId(i as u32);
            let (mut sig, mut ret, mut params, tparams) =
                self.resolve_function_signature(func_id, func, hierarchy, variables);
            if let Some(&anterior) = augmentadas.get(&func_id)
                && let Some(herdado) = data.get(anterior.0 as usize)
                && let Some((s2, r2, p2)) = self.herdar_assinatura(func, herdado, &params, ret, &tparams)
            {
                (sig, ret, params) = (s2, r2, p2);
            }

            // Se for acessor implícito de variável, sincronizar se necessário
            if let Some(var_id) = func.variable {
                let var_data = &mut variables[var_id.0 as usize];
                if func.kind == FunctionKind::Getter && var_data.declared_type.is_none() {
                    var_data.inferred = Some(ret);
                }
            }

            data.push(FunctionTypeData {
                signature: sig,
                return_type: ret,
                type_params: tparams,
                parameters: params,
            });
        }

        data
    }

    /// Para cada função de uma augmentation de código do usuário (não os
    /// patches do SDK), a declaração que ela aumenta: a primeira da cadeia
    /// (docs/AUGMENTATIONS.md §2 — os elementos anteriores apontam, por
    /// `patched_by`, para o efetivo).
    fn declaracoes_augmentadas(&self) -> HashMap<FunctionElementId, FunctionElementId> {
        let mut m: HashMap<FunctionElementId, FunctionElementId> = HashMap::new();
        for (i, f) in self.program.functions.iter().enumerate() {
            let Some(efetivo) = f.patched_by else { continue };
            if self.program.library(f.library).is_sdk {
                continue;
            }
            let atual = FunctionElementId(i as u32);
            m.entry(efetivo)
                .and_modify(|x| {
                    if atual.0 < x.0 {
                        *x = atual;
                    }
                })
                .or_insert(atual);
        }
        m
    }

    /// Os tipos omitidos numa augmentation vêm da declaração aumentada
    /// (spec de augmentations, "Augmenting functions": a assinatura pode
    /// omitir tipos, que valem os da declaração anterior). Parâmetros
    /// posicionais pela posição, nomeados pelo nome; o retorno, se omitido.
    /// Funções genéricas (dos dois lados) ficam como estão: herdar exigiria
    /// substituir os parâmetros de tipo. Conferido com o CFE e o analyzer
    /// 3.13.4 (`augment f(x) => …` com `String f(int x);` antes).
    fn herdar_assinatura(
        &mut self,
        func: &FunctionElement,
        anterior: &FunctionTypeData,
        params: &[ParameterTypeData],
        ret: TypeId,
        tparams: &[TypeParamId],
    ) -> Option<(TypeId, TypeId, Box<[ParameterTypeData]>)> {
        let FunctionRef::Function { unit, function } = func.node else { return None };
        if !tparams.is_empty() || !anterior.type_params.is_empty() {
            return None;
        }
        let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
        let escritos: Vec<bool> = match &ast_func.parameters {
            Some(ps) => ps.iter().map(|p| p.ty.is_some() || p.function_parameters.is_some()).collect(),
            None => Vec::new(),
        };
        if escritos.len() != params.len() {
            return None;
        }
        let mut novos: Vec<ParameterTypeData> = params.to_vec();
        let posicionais_antes: Vec<&ParameterTypeData> =
            anterior.parameters.iter().filter(|p| p.kind != ParameterKind::Named).collect();
        let mut posicao = 0usize;
        for (i, p) in novos.iter_mut().enumerate() {
            let correspondente = if p.kind == ParameterKind::Named {
                anterior.parameters.iter().find(|q| q.kind == ParameterKind::Named && q.externo == p.externo)
            } else {
                let q = posicionais_antes.get(posicao).copied();
                posicao += 1;
                q
            };
            if !escritos[i]
                && let Some(q) = correspondente
            {
                p.ty = q.ty;
            }
        }
        let novo_ret = if ast_func.return_type.is_none() && func.kind != FunctionKind::Setter {
            anterior.return_type
        } else {
            ret
        };
        let mut positional = Vec::new();
        let mut optional = Vec::new();
        let mut named = Vec::new();
        for p in &novos {
            match p.kind {
                ParameterKind::Required => positional.push(p.ty),
                ParameterKind::Optional => optional.push(p.ty),
                ParameterKind::Named => {
                    if let Some(n) = p.externo {
                        named.push((n, p.ty, p.required));
                    }
                }
            }
        }
        let sig = self.table.intern(Type::Function {
            type_params: Box::new([]),
            ret: novo_ret,
            positional: positional.into_boxed_slice(),
            optional: optional.into_boxed_slice(),
            named: named.into_boxed_slice(),
            nullable: false,
        });
        Some((sig, novo_ret, novos.into_boxed_slice()))
    }

    /// A assinatura de um membro é resolvida com o escopo de instância do
    /// contêiner dele (`InstanceScope`).
    fn resolve_function_signature(
        &mut self,
        func_id: FunctionElementId,
        func: &FunctionElement,
        hierarchy: &ClassHierarchy,
        variables: &[VariableTypeData],
    ) -> (TypeId, TypeId, Box<[ParameterTypeData]>, Box<[TypeParamId]>) {
        self.conteiner = conteiner_de(func.class, func.extension);
        self.membro_estatico = func.static_
            && !func.factory
            && func.kind != FunctionKind::Constructor
            && (func.class.is_some() || func.extension.is_some());
        let r = self.resolve_function_signature_no_escopo(func_id, func, hierarchy, variables);
        self.conteiner = None;
        self.membro_estatico = false;
        r
    }

    fn resolve_function_signature_no_escopo(
        &mut self,
        func_id: FunctionElementId,
        func: &FunctionElement,
        hierarchy: &ClassHierarchy,
        variables: &[VariableTypeData],
    ) -> (TypeId, TypeId, Box<[ParameterTypeData]>, Box<[TypeParamId]>) {
        let mut scope = self.get_enclosing_type_param_scope(func.class, func.extension);

        match func.node {
            FunctionRef::Function { unit, function } => {
                let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];

                // Parâmetros de tipo da função genérica
                let mut func_type_params = Vec::with_capacity(ast_func.type_params.len());
                for tp in ast_func.type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::Function(func_id),
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    func_type_params.push(pid);
                    scope.insert(tp.name.sym, pid);
                }

                // Bounds dos parâmetros de tipo
                for (tp, &pid) in ast_func.type_params.iter().zip(func_type_params.iter()) {
                    if let Some(ast_bound) = tp.bound {
                        let b = self.resolve_annotation(unit, ast_bound, func.library, &scope);
                        self.table.set_type_param_bound(pid, b);
                    }
                }

                // Resolução de parâmetros formais
                let mut param_types = Vec::new();
                let mut positional = Vec::new();
                let mut optional = Vec::new();
                let mut named = Vec::new();

                if let Some(parameters) = &ast_func.parameters {
                    let mut idx_posicional = 0usize;
                    for p in parameters.iter() {
                        let pos_atual = idx_posicional;
                        if p.kind != ParameterKind::Named {
                            idx_posicional += 1;
                        }
                        let p_name = p.name.as_ref().map(|n| n.sym);
                        let p_ty = if p.function_parameters.is_some() {
                            self.resolve_function_typed_parameter(unit, p, func.library, &scope)
                        } else if let Some(ast_ty) = p.ty {
                            self.resolve_annotation(unit, ast_ty, func.library, &scope)
                        } else {
                            // Tenta override inference se for método de instância
                            self.infer_override_parameter_type(func, p_name, p.kind, pos_atual, hierarchy, 0)
                                .unwrap_or_else(|| {
                                    if func.kind == FunctionKind::Setter {
                                        self.adiar_sobrescrita_de_campo(func_id, func, Some(param_types.len()), hierarchy);
                                    }
                                    self.core.dynamic_
                                })
                        };

                        param_types.push(ParameterTypeData {
                            name: p_name,
                            externo: p.nome_externo().map(|n| n.sym),
                            ty: p_ty,
                            required: p.required,
                            kind: p.kind,
                        });

                        match p.kind {
                            ParameterKind::Required => positional.push(p_ty),
                            ParameterKind::Optional => optional.push(p_ty),
                            ParameterKind::Named => {
                                if let Some(n) = p.nome_externo() {
                                    named.push((n.sym, p_ty, p.required));
                                }
                            }
                        }
                    }
                }

                // Retorno da função
                let ret_ty = if let Some(ast_ret) = ast_func.return_type {
                    self.resolve_annotation(unit, ast_ret, func.library, &scope)
                } else if func.kind == FunctionKind::Setter {
                    self.core.void_
                } else {
                    // Tenta override inference para retorno
                    self.infer_override_return_type(func, hierarchy, 0).unwrap_or_else(|| {
                        if func.kind == FunctionKind::Getter {
                            self.adiar_sobrescrita_de_campo(func_id, func, None, hierarchy);
                        }
                        self.core.dynamic_
                    })
                };

                let sig = self.table.intern(Type::Function {
                    type_params: func_type_params.clone().into_boxed_slice(),
                    ret: ret_ty,
                    positional: positional.into_boxed_slice(),
                    optional: optional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                });

                (
                    sig,
                    ret_ty,
                    param_types.into_boxed_slice(),
                    func_type_params.into_boxed_slice(),
                )
            }
            FunctionRef::Constructor { unit, member } => {
                let mem_node = &self.program.unit(unit).ast.members[member.0 as usize];
                let ctor = match &mem_node.kind {
                    MemberKind::Constructor(c) => c,
                    _ => unreachable!(),
                };

                let mut param_types = Vec::new();
                let mut positional = Vec::new();
                let mut optional = Vec::new();
                let mut named = Vec::new();

                for p in ctor.parameters.iter() {
                    let p_name = p.name.as_ref().map(|n| n.sym);
                    let p_ty = if p.function_parameters.is_some() {
                        self.resolve_function_typed_parameter(unit, p, func.library, &scope)
                    } else if let Some(ast_ty) = p.ty {
                        self.resolve_annotation(unit, ast_ty, func.library, &scope)
                    } else if p.this_ {
                        self.field_type_for_this_param(func, p_name).unwrap_or(self.core.dynamic_)
                    } else if p.super_ {
                        self.super_param_type(func, ctor, p, 0, hierarchy).unwrap_or(self.core.dynamic_)
                    } else {
                        self.core.dynamic_
                    };

                    param_types.push(ParameterTypeData {
                        name: p_name,
                        externo: p.nome_externo().map(|n| n.sym),
                        ty: p_ty,
                        required: p.required,
                        kind: p.kind,
                    });

                    match p.kind {
                        ParameterKind::Required => positional.push(p_ty),
                        ParameterKind::Optional => optional.push(p_ty),
                        ParameterKind::Named => {
                            if let Some(n) = p.nome_externo() {
                                named.push((n.sym, p_ty, p.required));
                            }
                        }
                    }
                }

                // Construtor instancia a classe dona com seus próprios parâmetros de tipo
                let ret_ty = self.instantiate_self_class(func.class);

                let sig = self.table.intern(Type::Function {
                    type_params: Box::new([]),
                    ret: ret_ty,
                    positional: positional.into_boxed_slice(),
                    optional: optional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                });

                (sig, ret_ty, param_types.into_boxed_slice(), Box::new([]))
            }
            FunctionRef::None => {
                // Sintéticos ou acessores implícitos
                if let Some(var_id) = func.variable {
                    let var_data = &variables[var_id.0 as usize];
                    let var_ty = var_data
                        .declared_type
                        .or(var_data.inferred)
                        .unwrap_or(self.core.dynamic_);

                    let is_getter = func.kind == FunctionKind::Getter
                        || self.program.variable(var_id).getter == Some(func_id);
                    if is_getter {
                        let sig = self.table.intern(Type::Function {
                            type_params: Box::new([]),
                            ret: var_ty,
                            positional: Box::new([]),
                            optional: Box::new([]),
                            named: Box::new([]),
                            nullable: false,
                        });
                        (sig, var_ty, Box::new([]), Box::new([]))
                    } else {
                        // Setter
                        let param = ParameterTypeData {
                            name: None,
                            externo: None,
                            ty: var_ty,
                            required: true,
                            kind: ParameterKind::Required,
                        };
                        let sig = self.table.intern(Type::Function {
                            type_params: Box::new([]),
                            ret: self.core.void_,
                            positional: Box::new([var_ty]),
                            optional: Box::new([]),
                            named: Box::new([]),
                            nullable: false,
                        });
                        (sig, self.core.void_, Box::new([param]), Box::new([]))
                    }
                } else if func.kind == FunctionKind::Getter {
                    // Getters sintéticos de enum: `values`, `index`, `name`.
                    let name = self.interner.resolve(func.name);
                    let ret = if name == "values" {
                        let elem = self.instantiate_self_class(func.class);
                        match self.core.list_class {
                            Some(l) => self.table.intern(Type::Interface { class: l, args: Box::new([elem]), nullable: false }),
                            None => self.core.dynamic_,
                        }
                    } else if name == "index" {
                        self.core.int
                    } else {
                        self.core.string
                    };
                    let sig = self.table.intern(Type::Function {
                        type_params: Box::new([]),
                        ret,
                        positional: Box::new([]),
                        optional: Box::new([]),
                        named: Box::new([]),
                        nullable: false,
                    });
                    (sig, ret, Box::new([]), Box::new([]))
                } else {
                    // Construtor sintético padrão
                    let ret_ty = self.instantiate_self_class(func.class);
                    let sig = self.table.intern(Type::Function {
                        type_params: Box::new([]),
                        ret: ret_ty,
                        positional: Box::new([]),
                        optional: Box::new([]),
                        named: Box::new([]),
                        nullable: false,
                    });
                    (sig, ret_ty, Box::new([]), Box::new([]))
                }
            }
        }
    }

    /// Argumentos de uma classe usada numa anotação: os escritos, ou a
    /// instanciação para os limites quando a classe é usada crua (`Map` é
    /// `Map<dynamic, dynamic>`, `C` com `T extends num` é `C<num>`).
    fn args_ou_limites(&mut self, cid: ClassId, args: Vec<TypeId>) -> Vec<TypeId> {
        let formals = self.class_type_params[cid.0 as usize].clone();
        if args.len() == formals.len() {
            return args;
        }
        self.instanciar_para_limites(&formals)
    }

    /// Instanciação para os limites (`instantiate to bounds`): o limite
    /// escrito, ou `dynamic`; limites que mencionam os próprios parâmetros
    /// (F-limites) têm esses parâmetros trocados por `dynamic`.
    fn instanciar_para_limites(&mut self, formals: &[TypeParamId]) -> Vec<TypeId> {
        crate::ops::instanciar_para_limites(formals, &[], self.table, self.core)
    }

    fn instantiate_self_class(&mut self, class_opt: Option<ClassId>) -> TypeId {
        if let Some(cls) = class_opt {
            let params = self.class_type_params[cls.0 as usize].clone();
            let args: Vec<TypeId> = params
                .iter()
                .map(|&p| {
                    self.table.intern(Type::TypeParameter {
                        param: p,
                        nullable: false,
                    })
                })
                .collect();
            let is_ext = self.program.class(cls).kind == ClassKind::ExtensionType;
            if is_ext {
                self.table.intern(Type::ExtensionType {
                    decl: cls,
                    args: args.into_boxed_slice(),
                    nullable: false,
                })
            } else {
                self.table.intern(Type::Interface {
                    class: cls,
                    args: args.into_boxed_slice(),
                    nullable: false,
                })
            }
        } else {
            self.core.dynamic_
        }
    }

    /// Membro homônimo nos supertipos da classe de `func`, na ordem de busca
    /// (superclasses e mixins, depois interfaces): `(classe, função)`.
    fn membro_sobreposto(&self, func: &FunctionElement, hierarchy: &ClassHierarchy) -> Option<(ClassId, FunctionElementId)> {
        let class_id = func.class?;
        if func.static_ {
            return None;
        }
        let chave = if func.kind == FunctionKind::Setter {
            self.interner.lookup(&format!("{}_=", self.interner.resolve(func.name)))?
        } else {
            func.name
        };
        for (sup, _) in crate::scope::supertipos_ordenados(self.program, hierarchy, class_id) {
            if let Some(&f) = self.program.class(sup).instance_members.get(&chave) {
                return Some((sup, f));
            }
        }
        None
    }

    /// Substitui os parâmetros da superclasse `sup` pelo que a classe de
    /// `func` lhe passa.
    fn instanciar_do_super(&mut self, func: &FunctionElement, sup: ClassId, t: TypeId, hierarchy: &ClassHierarchy) -> Option<TypeId> {
        let class_id = func.class?;
        let this = self.instantiate_self_class(Some(class_id));
        let super_ty = hierarchy.supertype_of(this, sup, self.table, self.core)?;
        let args = match self.table.get(super_ty).clone() {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args,
            _ => return None,
        };
        let params = self.class_type_params[sup.0 as usize].clone();
        let subst: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
        Some(substitute(t, &subst, self.table))
    }

    /// Tipo do campo `v` escrito na declaração, no escopo da classe `sup`.
    fn tipo_escrito_de_campo(&mut self, v: dartforge_elements::model::VariableId, sup: ClassId) -> Option<TypeId> {
        let (unit, ast_ty) = match self.program.variable(v).node {
            VariableRef::Field { unit, member, .. } => match &self.program.unit(unit).ast.member(member).kind {
                MemberKind::Field(vl) => (unit, vl.ty?),
                _ => return None,
            },
            _ => return None,
        };
        let lib = self.program.variable(v).library;
        let scope = self.get_enclosing_type_param_scope(Some(sup), None);
        Some(self.resolve_annotation(unit, ast_ty, lib, &scope))
    }

    /// Registra a sobrescrita cujo membro sobreposto é o acessor implícito de
    /// um campo sem tipo escrito: o tipo sai da inferência do inicializador
    /// ([`OutlineTypes::sobrescritas_de_campo`]).
    fn adiar_sobrescrita_de_campo(
        &mut self,
        func_id: FunctionElementId,
        func: &FunctionElement,
        parametro: Option<usize>,
        hierarchy: &ClassHierarchy,
    ) {
        let Some((sup, sf)) = self.membro_sobreposto(func, hierarchy) else { return };
        let super_func = self.program.function(sf);
        if !matches!(super_func.node, FunctionRef::None) {
            return;
        }
        let Some(v) = super_func.variable else { return };
        if !matches!(self.program.variable(v).node, VariableRef::Field { .. }) {
            return;
        }
        let Some(class_id) = func.class else { return };
        let this = self.instantiate_self_class(Some(class_id));
        let Some(super_ty) = hierarchy.supertype_of(this, sup, self.table, self.core) else { return };
        let args = match self.table.get(super_ty) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.clone(),
            _ => return,
        };
        let params = self.class_type_params[sup.0 as usize].clone();
        let subst = params.iter().copied().zip(args.iter().copied()).collect();
        self.sobrescritas_de_campo.push(SobrescritaDeCampo { funcao: func_id, campo: v, subst, parametro });
    }

    /// Tipo de retorno herdado (override inference): o do membro sobreposto,
    /// escrito ou, se omitido lá também, herdado recursivamente.
    fn infer_override_return_type(&mut self, func: &FunctionElement, hierarchy: &ClassHierarchy, prof: u32) -> Option<TypeId> {
        if prof > 16 {
            return None;
        }
        let (sup, sf) = self.membro_sobreposto(func, hierarchy)?;
        let super_func = self.program.function(sf);
        let t = match (super_func.kind, super_func.node) {
            (FunctionKind::ImplicitAccessor, _) => {
                let v = super_func.variable?;
                self.tipo_escrito_de_campo(v, sup)?
            }
            (_, FunctionRef::Function { unit, function }) => {
                let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
                if !ast_func.type_params.is_empty() {
                    return None;
                }
                match ast_func.return_type {
                    Some(r) => {
                        let scope = self.get_enclosing_type_param_scope(Some(sup), None);
                        self.resolve_annotation(unit, r, super_func.library, &scope)
                    }
                    None => {
                        let t = self.infer_override_return_type(super_func, hierarchy, prof + 1)?;
                        return self.instanciar_do_super(func, sup, t, hierarchy).or(Some(t));
                    }
                }
            }
            _ => return None,
        };
        self.instanciar_do_super(func, sup, t, hierarchy)
    }

    /// Tipo herdado de um parâmetro: o nomeado pelo nome, o posicional pela
    /// posição, no membro sobreposto (recursivo se lá também foi omitido).
    fn infer_override_parameter_type(
        &mut self,
        func: &FunctionElement,
        param_name: Option<SymbolId>,
        kind: ParameterKind,
        posicao: usize,
        hierarchy: &ClassHierarchy,
        prof: u32,
    ) -> Option<TypeId> {
        if prof > 16 {
            return None;
        }
        let (sup, sf) = self.membro_sobreposto(func, hierarchy)?;
        let super_func = self.program.function(sf);
        let t = match super_func.node {
            FunctionRef::Function { unit, function } => {
                let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
                if !ast_func.type_params.is_empty() {
                    return None;
                }
                let params = ast_func.parameters.as_ref()?;
                let alvo = if kind == ParameterKind::Named {
                    params.iter().find(|p| p.kind == ParameterKind::Named && p.name.map(|n| n.sym) == param_name)?
                } else {
                    params.iter().filter(|p| p.kind != ParameterKind::Named).nth(posicao)?
                };
                if alvo.function_parameters.is_some() {
                    let scope = self.get_enclosing_type_param_scope(Some(sup), None);
                    self.resolve_function_typed_parameter(unit, alvo, super_func.library, &scope)
                } else if let Some(ast_ty) = alvo.ty {
                    let scope = self.get_enclosing_type_param_scope(Some(sup), None);
                    self.resolve_annotation(unit, ast_ty, super_func.library, &scope)
                } else {
                    let t = self.infer_override_parameter_type(super_func, param_name, kind, posicao, hierarchy, prof + 1)?;
                    return self.instanciar_do_super(func, sup, t, hierarchy).or(Some(t));
                }
            }
            FunctionRef::None if func.kind == FunctionKind::Setter => {
                let v = super_func.variable?;
                self.tipo_escrito_de_campo(v, sup)?
            }
            _ => return None,
        };
        self.instanciar_do_super(func, sup, t, hierarchy)
    }

    fn get_enclosing_type_param_scope(
        &self,
        class: Option<ClassId>,
        ext: Option<ExtensionId>,
    ) -> HashMap<SymbolId, TypeParamId> {
        let mut scope = HashMap::new();
        if let Some(cls) = class {
            let elem = self.program.class(cls);
            let params = &self.class_type_params[cls.0 as usize];
            for (p_elem, &pid) in elem.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
        } else if let Some(e) = ext {
            let elem = self.program.extension(e);
            let params = &self.extension_type_params[e.0 as usize];
            for (p_elem, &pid) in elem.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
        }
        scope
    }

    fn resolve_annotation(
        &mut self,
        unit_id: UnitId,
        ast_ty_id: ast::TypeId,
        library: LibraryId,
        type_param_scope: &HashMap<SymbolId, TypeParamId>,
    ) -> TypeId {
        let t = self.resolver_anotacao_escrita(unit_id, ast_ty_id, library, type_param_scope);
        // `TypeAnnotation.type`, para as regras que o leem.
        self.tipos_escritos.insert((unit_id, ast_ty_id), t);
        // O `ResolutionVisitor.visitNamedType` resolve os argumentos de tipo
        // antes do nome, seja ele o que for (parâmetro de tipo, `dynamic`,
        // nome indefinido ou que não é tipo): os que o caminho do nome não
        // resolveu são resolvidos aqui, com os erros deles.
        let args: Vec<ast::TypeId> = match &self.program.unit(unit_id).ast.ty(ast_ty_id).kind {
            ast::TypeKind::Named { args, .. } => args.to_vec(),
            _ => Vec::new(),
        };
        for a in args {
            if !self.tipos_escritos.contains_key(&(unit_id, a)) {
                self.contexto_de_tipo = ContextoDeTipo::ArgumentoDeTipo;
                self.resolve_annotation(unit_id, a, library, type_param_scope);
            }
        }
        t
    }

    fn resolver_anotacao_escrita(
        &mut self,
        unit_id: UnitId,
        ast_ty_id: ast::TypeId,
        library: LibraryId,
        type_param_scope: &HashMap<SymbolId, TypeParamId>,
    ) -> TypeId {
        let annot = self.program.unit(unit_id).ast.ty(ast_ty_id);
        let is_nullable = annot.nullable;
        let span = annot.span;
        let contexto = std::mem::take(&mut self.contexto_de_tipo);

        match &annot.kind {
            ast::TypeKind::Void => self.core.void_,
            ast::TypeKind::Named { name, args } => {
                // O nome (com o prefixo) é a faixa dos erros de nome; o tipo
                // inteiro, a do número de argumentos.
                let faixa = dartforge_diagnostics::Span { start: name[0].span.start, end: name[name.len() - 1].span.end };
                let texto = self.interner.resolve(name[name.len() - 1].sym).to_string();
                if name.len() == 1 {
                    let sym = name[0].sym;

                    // 1. Verificar escopo de parâmetros de tipo vigentes
                    if let Some(&param_id) = type_param_scope.get(&sym) {
                        if !args.is_empty() {
                            self.avisar(unit_id, diagnostico_de_argumentos_de_tipo(&texto, 0, args.len(), span));
                        }
                        if self.membro_estatico && param_da_classe(self.table, param_id) {
                            self.avisar(unit_id, diagnostico_de_parametro_em_estatico(faixa));
                        }
                        return self.table.intern(Type::TypeParameter {
                            param: param_id,
                            nullable: is_nullable,
                        });
                    }

                    // 2. Verificar tipos especiais do sistema (`dynamic<int>`,
                    // `Never<int>`: a contagem errada do `_buildTypeArguments`,
                    // no tipo inteiro). `dynamic` e `Never` vêm do namespace do
                    // `dart:core`: com o core só por prefixo, são indefinidos.
                    let especial = self.interner.lookup("dynamic") == Some(sym) || self.interner.lookup("Never") == Some(sym);
                    if especial && !self.nucleo_visivel(unit_id, sym) {
                        if !crate::scope::deve_ignorar_indefinido(self.program, self.interner, unit_id, None, sym) {
                            self.avisar_nome_de_tipo(unit_id, contexto, false, &texto, faixa);
                        }
                        return self.table.invalido(self.core.dynamic_);
                    }
                    let embutido = self.interner.lookup("dynamic") == Some(sym) || self.interner.lookup("Never") == Some(sym);
                    if embutido && !args.is_empty() {
                        self.avisar(unit_id, diagnostico_de_argumentos_de_tipo(&texto, 0, args.len(), span));
                    }
                    if self.interner.lookup("dynamic") == Some(sym) {
                        return self.core.dynamic_;
                    }
                    if self.interner.lookup("void") == Some(sym) {
                        return self.core.void_;
                    }
                    if self.interner.lookup("Never") == Some(sym) {
                        return if is_nullable {
                            self.table.decorar(self.core.null, crate::table::Exibicao::NeverAnulavel)
                        } else {
                            self.core.never
                        };
                    }
                    if self.interner.lookup("Null") == Some(sym) {
                        return self.core.null;
                    }
                    // `FutureOr` só é o de `dart:async` quando está no escopo
                    // (o `dart:core` não o exporta): sem import, o nome é
                    // indefinido como qualquer outro.
                    let futureor_no_escopo = matches!(
                        self.program.lookup_na_unidade(unit_id, sym).and_then(|b| b.getter),
                        Some(Element::Class(c)) if self.program.library(self.program.class(c).library).uri == "dart:async"
                    );
                    if self.interner.lookup("FutureOr") == Some(sym) && futureor_no_escopo {
                        if args.len() == 1 {
                            let arg_ty = self.resolve_annotation(
                                unit_id,
                                args[0],
                                library,
                                type_param_scope,
                            );
                            return self.table.intern(Type::FutureOr {
                                arg: arg_ty,
                                nullable: is_nullable,
                            });
                        } else if args.is_empty() {
                            // `FutureOr` cru: instanciado para o limite.
                            return self.table.intern(Type::FutureOr {
                                arg: self.core.dynamic_,
                                nullable: is_nullable,
                            });
                        } else {
                            self.avisar(unit_id, diagnostico_de_argumentos_de_tipo(&texto, 1, args.len(), span));
                            return self.core.dynamic_;
                        }
                    }

                    // 3. O escopo de instância do contêiner esconde o de topo.
                    // (Nome sintético da recuperação, sem largura: nada.)
                    match if span.start == span.end { None } else { self.no_conteiner(sym) } {
                        Some(NoConteiner::Getter) => {
                            self.avisar_nome_de_tipo(unit_id, contexto, true, &texto, faixa);
                            return self.table.invalido(self.core.dynamic_);
                        }
                        Some(NoConteiner::SoSetter) => {
                            // O vinculador não acha o tipo (inválido, sem
                            // aviso); o `ResolutionVisitor`, que relata, acha
                            // o de fora.
                            return self.table.invalido(self.core.dynamic_);
                        }
                        None => {}
                    }

                    // 4. Resolução no escopo da biblioteca
                    let binding = self.program.lookup_na_unidade(unit_id, sym);
                    match binding {
                        Some(b) => {
                            if b.ambiguous {
                                let lista = crate::scope::bibliotecas_ambiguas(self.program, unit_id, None, sym);
                                self.avisar(
                                    unit_id,
                                    Diagnostic::com_codigo(dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_IMPORT, name[0].span, [texto.as_str(), lista.as_str()]),
                                );
                                return self.table.invalido(self.core.dynamic_);
                            }
                            match b.getter {
                                Some(Element::Class(cid)) => {
                                    let resolved_args: Vec<TypeId> = args
                                        .iter()
                                        .map(|&a| {
                                            self.contexto_de_tipo = ContextoDeTipo::ArgumentoDeTipo;
                                            self.resolve_annotation(
                                                unit_id,
                                                a,
                                                library,
                                                type_param_scope,
                                            )
                                        })
                                        .collect();

                                    let n_params = self.class_type_params[cid.0 as usize].len();
                                    if !args.is_empty() && args.len() != n_params {
                                        self.avisar(unit_id, diagnostico_de_argumentos_de_tipo(&texto, n_params, args.len(), span));
                                    }
                                    // Número errado de argumentos de tipo: todos ficam inválidos
                                    // (`_buildTypeArguments`, `named_type_resolver.dart:148`).
                                    let resolved_args = if !args.is_empty() && args.len() != n_params {
                                        vec![self.table.invalido(self.core.dynamic_); n_params]
                                    } else {
                                        self.args_ou_limites(cid, resolved_args)
                                    };
                                    let is_ext =
                                        self.program.class(cid).kind == ClassKind::ExtensionType;
                                    if is_ext {
                                        self.table.intern(Type::ExtensionType {
                                            decl: cid,
                                            args: resolved_args.into_boxed_slice(),
                                            nullable: is_nullable,
                                        })
                                    } else {
                                        self.table.intern(Type::Interface {
                                            class: cid,
                                            args: resolved_args.into_boxed_slice(),
                                            nullable: is_nullable,
                                        })
                                    }
                                }
                                Some(Element::Typedef(tid)) if self.typedef_auto_referente(tid) => self.core.dynamic_,
                                Some(Element::Typedef(tid)) => {
                                    let resolved_args: Vec<TypeId> = args
                                        .iter()
                                        .map(|&a| {
                                            self.contexto_de_tipo = ContextoDeTipo::ArgumentoDeTipo;
                                            self.resolve_annotation(
                                                unit_id,
                                                a,
                                                library,
                                                type_param_scope,
                                            )
                                        })
                                        .collect();

                                    let target_ty = self.ensure_typedef_resolved(tid);
                                    let formals = self.typedef_type_params[tid.0 as usize].clone();
                                    if !args.is_empty() && args.len() != formals.len() {
                                        self.avisar(unit_id, diagnostico_de_argumentos_de_tipo(&texto, formals.len(), args.len(), span));
                                    }
                                    let resolved_args = if resolved_args.len() == formals.len() {
                                        resolved_args
                                    } else if !args.is_empty() {
                                        // Número errado de argumentos de tipo: todos inválidos.
                                        vec![self.table.invalido(self.core.dynamic_); formals.len()]
                                    } else {
                                        self.instanciar_para_limites(&formals)
                                    };
                                    let mut subst = HashMap::with_capacity(formals.len());
                                    for (&f, &a) in formals.iter().zip(resolved_args.iter()) {
                                        subst.insert(f, a);
                                    }
                                    let expanded = substitute(target_ty, &subst, self.table);
                                    let expanded = self.table.decorar(
                                        expanded,
                                        crate::table::Exibicao::Alias { typedef: tid, args: resolved_args.clone().into_boxed_slice() },
                                    );
                                    if is_nullable {
                                        nullable(expanded, self.table)
                                    } else {
                                        expanded
                                    }
                                }
                                _ => {
                                    self.avisar_nome_de_tipo(unit_id, contexto, true, &texto, faixa);
                                    self.table.invalido(self.core.dynamic_)
                                }
                            }
                        }
                        None => {
                            // Nome sintético da recuperação do parser (vazio,
                            // sem largura): o analyzer não o relata; nem o que
                            // pode vir de um import ou parte que não existe
                            // (`shouldIgnoreUndefinedNamedType`).
                            if span.start != span.end
                                && !crate::scope::deve_ignorar_indefinido(self.program, self.interner, unit_id, None, name[0].sym)
                            {
                                self.avisar_nome_de_tipo(unit_id, contexto, false, &texto, faixa);
                            }
                            self.table.invalido(self.core.dynamic_)
                        }
                    }
                } else if name.len() == 2 {
                    let prefix = name[0].sym;
                    let member = name[1].sym;
                    if name[0].span.start != name[0].span.end {
                        let (classe, extensao) = match self.conteiner {
                            Some(Conteiner::Classe(c)) => (Some(c), None),
                            Some(Conteiner::Extensao(e)) => (None, Some(e)),
                            None => (None, None),
                        };
                        let local = type_param_scope.contains_key(&prefix);
                        if prefixo_sombreado(self.program, self.interner, unit_id, prefix, local, classe, extensao) {
                            let nome = self.interner.resolve(prefix).to_string();
                            self.avisar(unit_id, diagnostico_de_prefixo_sombreado(&nome, name[0].span));
                            return self.table.invalido(self.core.dynamic_);
                        }
                        if matches!(self.no_conteiner(prefix), Some(NoConteiner::SoSetter)) {
                            return self.table.invalido(self.core.dynamic_);
                        }
                    }
                    let binding = self.program.lookup_prefixed_na_unidade(unit_id, prefix, member);
                    match binding {
                        Some(b) => match b.getter {
                            Some(Element::Class(cid)) => {
                                let resolved_args: Vec<TypeId> = args
                                    .iter()
                                    .map(|&a| {
                                        self.contexto_de_tipo = ContextoDeTipo::ArgumentoDeTipo;
                                        self.resolve_annotation(
                                            unit_id,
                                            a,
                                            library,
                                            type_param_scope,
                                        )
                                    })
                                    .collect();

                                let n_params = self.class_type_params[cid.0 as usize].len();
                                if !args.is_empty() && args.len() != n_params {
                                    self.avisar(unit_id, diagnostico_de_argumentos_de_tipo(&texto, n_params, args.len(), span));
                                }
                                // Número errado de argumentos de tipo: todos ficam inválidos
                                // (`_buildTypeArguments`, `named_type_resolver.dart:148`).
                                let resolved_args = if !args.is_empty() && args.len() != n_params {
                                    vec![self.table.invalido(self.core.dynamic_); n_params]
                                } else {
                                    self.args_ou_limites(cid, resolved_args)
                                };
                                let is_ext =
                                    self.program.class(cid).kind == ClassKind::ExtensionType;
                                if is_ext {
                                    self.table.intern(Type::ExtensionType {
                                        decl: cid,
                                        args: resolved_args.into_boxed_slice(),
                                        nullable: is_nullable,
                                    })
                                } else {
                                    self.table.intern(Type::Interface {
                                        class: cid,
                                        args: resolved_args.into_boxed_slice(),
                                        nullable: is_nullable,
                                    })
                                }
                            }
                            Some(Element::Typedef(tid)) if self.typedef_auto_referente(tid) => self.core.dynamic_,
                            Some(Element::Typedef(tid)) => {
                                let resolved_args: Vec<TypeId> = args
                                    .iter()
                                    .map(|&a| {
                                        self.contexto_de_tipo = ContextoDeTipo::ArgumentoDeTipo;
                                        self.resolve_annotation(
                                            unit_id,
                                            a,
                                            library,
                                            type_param_scope,
                                        )
                                    })
                                    .collect();

                                let target_ty = self.ensure_typedef_resolved(tid);
                                let formals = self.typedef_type_params[tid.0 as usize].clone();
                                if !args.is_empty() && args.len() != formals.len() {
                                    self.avisar(unit_id, diagnostico_de_argumentos_de_tipo(&texto, formals.len(), args.len(), span));
                                }
                                let resolved_args = if resolved_args.len() == formals.len() {
                                    resolved_args
                                } else if !args.is_empty() {
                                    // Número errado de argumentos de tipo: todos inválidos.
                                    vec![self.table.invalido(self.core.dynamic_); formals.len()]
                                } else {
                                    self.instanciar_para_limites(&formals)
                                };
                                let mut subst = HashMap::with_capacity(formals.len());
                                for (&f, &a) in formals.iter().zip(resolved_args.iter()) {
                                    subst.insert(f, a);
                                }
                                let expanded = substitute(target_ty, &subst, self.table);
                                let expanded = self.table.decorar(
                                    expanded,
                                    crate::table::Exibicao::Alias { typedef: tid, args: resolved_args.clone().into_boxed_slice() },
                                );
                                if is_nullable {
                                    nullable(expanded, self.table)
                                } else {
                                    expanded
                                }
                            }
                            _ => {
                                self.avisar_nome_de_tipo(unit_id, contexto, true, &texto, faixa);
                                self.table.invalido(self.core.dynamic_)
                            }
                        },
                        None => {
                            // `_rewriteToConstructorName` (`named_type_resolver.dart:101-109`,
                            // `:329-382`): o "prefixo" é uma classe ou um alias, não um
                            // prefixo de import (`A.foo` lido como `prefixo.Nome`). Fora
                            // da criação, `NOT_A_TYPE` com `A.foo`, do prefixo ao fim do
                            // nome, em qualquer contexto; nunca o nome indefinido.
                            let prefixo_e_tipo = matches!(
                                self.program.lookup_na_unidade(unit_id, prefix).and_then(|b| b.getter),
                                Some(Element::Class(_) | Element::Typedef(_))
                            );
                            if prefixo_e_tipo {
                                let completo = format!("{}.{}", self.interner.resolve(prefix), texto);
                                self.avisar(
                                    unit_id,
                                    Diagnostic::com_codigo(dartforge_diagnostics::codigos::compile_time_error::NOT_A_TYPE, faixa, [completo.as_str()]),
                                );
                            } else if !crate::scope::deve_ignorar_indefinido(self.program, self.interner, unit_id, Some(name[0].sym), name[1].sym) {
                                self.avisar_nome_de_tipo(unit_id, contexto, false, &texto, faixa);
                            }
                            self.table.invalido(self.core.dynamic_)
                        }
                    }
                } else {
                    self.table.invalido(self.core.dynamic_)
                }
            }
            ast::TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                let mut local_scope = type_param_scope.clone();
                let mut local_params = Vec::with_capacity(type_params.len());
                for tp in type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::GenericFunctionType,
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    local_params.push(pid);
                    local_scope.insert(tp.name.sym, pid);
                }

                for (tp, &pid) in type_params.iter().zip(local_params.iter()) {
                    if let Some(ast_bound) = tp.bound {
                        let b = self.resolve_annotation(unit_id, ast_bound, library, &local_scope);
                        self.table.set_type_param_bound(pid, b);
                    }
                }

                let ret = if let Some(r) = return_type {
                    self.resolve_annotation(unit_id, *r, library, &local_scope)
                } else {
                    self.core.dynamic_
                };

                let (pos, opt, named) =
                    self.resolve_ast_parameter_types(unit_id, parameters, library, &local_scope);

                self.table.intern(Type::Function {
                    type_params: local_params.into_boxed_slice(),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: is_nullable,
                })
            }
            ast::TypeKind::Record { positional, named } => {
                let mut pos = Vec::with_capacity(positional.len());
                for &p in positional.iter() {
                    pos.push(self.resolve_annotation(unit_id, p, library, type_param_scope));
                }

                let mut n = Vec::with_capacity(named.len());
                for (name, ty) in named.iter() {
                    let resolved = self.resolve_annotation(unit_id, *ty, library, type_param_scope);
                    n.push((name.sym, resolved));
                }

                self.table.intern(Type::Record {
                    positional: pos.into_boxed_slice(),
                    named: n.into_boxed_slice(),
                    nullable: is_nullable,
                })
            }
        }
    }

    /// Parâmetro na forma antiga `R nome(P p)`: um tipo de função com retorno `p.ty`.
    fn resolve_function_typed_parameter(
        &mut self,
        unit_id: UnitId,
        p: &ast::Parameter,
        library: LibraryId,
        scope: &HashMap<SymbolId, TypeParamId>,
    ) -> TypeId {
        let mut local_scope = scope.clone();
        let mut local_params = Vec::new();
        for tp in p.function_type_params.iter() {
            let pid = self.table.alloc_type_param(
                tp.name.sym,
                TypeParamOwner::GenericFunctionType,
                self.core.object_nullable,
                Variance::Unspecified,
            );
            local_params.push(pid);
            local_scope.insert(tp.name.sym, pid);
        }
        let ret = match p.ty {
            Some(t) => self.resolve_annotation(unit_id, t, library, &local_scope),
            None => self.core.dynamic_,
        };
        let params: &[ast::Parameter] = p.function_parameters.as_deref().unwrap_or(&[]);
        let (pos, opt, named) = self.resolve_ast_parameter_types(unit_id, params, library, &local_scope);
        self.table.intern(Type::Function {
            type_params: local_params.into_boxed_slice(),
            ret,
            positional: pos.into_boxed_slice(),
            optional: opt.into_boxed_slice(),
            named: named.into_boxed_slice(),
            nullable: p.function_nullable,
        })
    }

    /// Tipo de um parâmetro `super.x` sem anotação: o do parâmetro homónimo do
    /// construtor da superclasse chamado (`super(...)`/`super.nome(...)`; sem
    /// inicializador, o sem nome), com os parâmetros de tipo da superclasse
    /// substituídos pelo que a classe lhe passa.
    fn super_param_type(&mut self, func: &FunctionElement, ctor: &ast::Constructor, p: &ast::Parameter, depth: u32, hierarchy: &ClassHierarchy) -> Option<TypeId> {
        let t = self.super_param_type_cru(func, ctor, p, depth, hierarchy)?;
        let sup = self.program.class(func.class?).supertype_class?;
        Some(self.instanciar_do_super(func, sup, t, hierarchy).unwrap_or(t))
    }

    fn super_param_type_cru(&mut self, func: &FunctionElement, ctor: &ast::Constructor, p: &ast::Parameter, depth: u32, hierarchy: &ClassHierarchy) -> Option<TypeId> {
        if depth > 8 {
            return None;
        }
        let class = func.class?;
        let sup = self.program.class(class).supertype_class?;
        let super_name = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        });
        let key = match super_name {
            Some(Some(n)) => n,
            _ => self.interner.lookup("")?,
        };
        let sfid = *self.program.class(sup).constructors.get(&key)?;
        let sfunc = self.program.function(sfid);
        let FunctionRef::Constructor { unit, member } = sfunc.node else { return None };
        let mem = self.program.unit(unit).ast.member(member);
        let MemberKind::Constructor(sctor) = &mem.kind else { return None };
        let pname = p.name?.sym;
        let sp = if p.kind == ParameterKind::Named {
            sctor.parameters.iter().find(|q| q.kind == ParameterKind::Named && q.nome_externo().map(|n| n.sym) == Some(pname))?
        } else {
            // Posicional: o índice entre os posicionais de `super.` casa com os
            // posicionais restantes do super construtor após os passados em `super(...)`.
            let explicit = ctor
                .initializers
                .iter()
                .find_map(|i| match i {
                    ast::Initializer::Super { arguments, .. } => Some(arguments.args.iter().filter(|a| a.name.is_none()).count()),
                    _ => None,
                })
                .unwrap_or(0);
            let my_index = ctor.parameters.iter().filter(|q| q.super_ && q.kind != ParameterKind::Named).position(|q| std::ptr::eq(q, p))?;
            sctor.parameters.iter().filter(|q| q.kind != ParameterKind::Named).nth(explicit + my_index)?
        };
        let scope = self.get_enclosing_type_param_scope(Some(sup), None);
        if let Some(t) = sp.ty {
            return Some(self.resolve_annotation(unit, t, sfunc.library, &scope));
        }
        if sp.this_ {
            return self.field_type_for_this_param(sfunc, sp.name.map(|n| n.sym));
        }
        if sp.super_ {
            let sfunc2 = self.program.function(sfid);
            return self.super_param_type(sfunc2, sctor, sp, depth + 1, hierarchy);
        }
        None
    }

    /// Tipo do campo para um parâmetro `this.x` sem anotação.
    fn field_type_for_this_param(&mut self, func: &FunctionElement, name: Option<SymbolId>) -> Option<TypeId> {
        let class = func.class?;
        let name = name?;
        let cls = self.program.class(class);
        let vid = cls.fields.iter().copied().find(|v| self.program.variable(*v).name == name)?;
        let v = self.program.variable(vid);
        let VariableRef::Field { unit, member, index } = v.node else { return None };
        let mem = self.program.unit(unit).ast.member(member);
        let MemberKind::Field(list) = &mem.kind else { return None };
        let _ = index;
        let t = list.ty?;
        let scope = self.get_enclosing_type_param_scope(Some(class), None);
        Some(self.resolve_annotation(unit, t, v.library, &scope))
    }

    fn resolve_ast_parameter_types(
        &mut self,
        unit_id: UnitId,
        parameters: &[ast::Parameter],
        library: LibraryId,
        scope: &HashMap<SymbolId, TypeParamId>,
    ) -> ParameterComponents {
        let mut positional = Vec::new();
        let mut optional = Vec::new();
        let mut named = Vec::new();

        for p in parameters.iter() {
            let p_ty = if p.function_parameters.is_some() {
                self.resolve_function_typed_parameter(unit_id, p, library, scope)
            } else if let Some(ast_ty) = p.ty {
                self.resolve_annotation(unit_id, ast_ty, library, scope)
            } else {
                self.core.dynamic_
            };

            match p.kind {
                ParameterKind::Required => positional.push(p_ty),
                ParameterKind::Optional => optional.push(p_ty),
                ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, p_ty, p.required));
                    }
                }
            }
        }

        (positional, optional, named)
    }
}

/// O contêiner de um membro (classe ou extensão), se houver.
fn conteiner_de(classe: Option<ClassId>, extensao: Option<ExtensionId>) -> Option<Conteiner> {
    match (classe, extensao) {
        (Some(c), _) => Some(Conteiner::Classe(c)),
        (None, Some(e)) => Some(Conteiner::Extensao(e)),
        _ => None,
    }
}
