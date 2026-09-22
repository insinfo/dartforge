//! Representação de tipos e tabela hash-consed (`TypeTable`).
//!
//! Disciplina de memória (docs/FRONTEND-ARQUITETURA.md §2):
//! - Tipos semânticos são hash-consed: um único [`TypeId`] por tipo estruturalmente igual.
//! - Nulabilidade faz parte da identidade estrutural (`int` ≠ `int?`).
//! - Sem ponteiros de heap em ciclo (`Rc`/`RefCell` proibidos); nós referenciam
//!   outros tipos exclusivamente por [`TypeId`] e [`TypeParamId`].
//! - Rastreio contínuo de `payload_bytes` e contagem de tipos para monitoramento
//!   de platô do compilador e LSP.

use dartforge_elements::model::{
    ClassId, ExtensionId, FunctionElementId, LibraryId, Program, TypedefId,
};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// Identificador de um tipo internado na [`TypeTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(pub u32);

/// Identificador de um parâmetro de tipo na [`TypeTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeParamId(pub u32);

/// Dono de um parâmetro de tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeParamOwner {
    Class(ClassId),
    Extension(ExtensionId),
    Typedef(TypedefId),
    Function(FunctionElementId),
    GenericFunctionType,
}

/// Variância de um parâmetro de tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Variance {
    #[default]
    Unspecified,
    Covariant,
    Contravariant,
    Invariant,
}

/// Metadados de um parâmetro de tipo registrado na tabela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameterData {
    pub name: SymbolId,
    pub owner: TypeParamOwner,
    pub bound: TypeId,
    pub variance: Variance,
}

/// Representação semântica de um tipo em Dart 3.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    /// O tipo `dynamic`.
    Dynamic,
    /// O tipo `void`.
    Void,
    /// O tipo `Never` (fundo do sistema de tipos).
    Never,
    /// O tipo `Null`.
    Null,
    /// Tipo de interface de classe, mixin ou enum: `C<T1, ..., Tn>` ou `C<T1, ..., Tn>?`.
    Interface {
        class: ClassId,
        args: Box<[TypeId]>,
        nullable: bool,
    },
    /// Tipo de função estrutural: `U Function<X extends B>(P, [O], {N})`.
    Function {
        type_params: Box<[TypeParamId]>,
        ret: TypeId,
        positional: Box<[TypeId]>,
        optional: Box<[TypeId]>,
        /// Parâmetros nomeados ordenados canonicamente por `SymbolId`: `(nome, tipo, required)`.
        named: Box<[(SymbolId, TypeId, bool)]>,
        nullable: bool,
    },
    /// Tipo de tupla/record: `(T1, ..., Tn, {N1: U1, ...})`.
    Record {
        positional: Box<[TypeId]>,
        /// Campos nomeados ordenados canonicamente por `SymbolId`: `(nome, tipo)`.
        named: Box<[(SymbolId, TypeId)]>,
        nullable: bool,
    },
    /// Parâmetro de tipo em escopo: `X` ou `X?`.
    TypeParameter { param: TypeParamId, nullable: bool },
    /// Tipo união `FutureOr<T>` ou `FutureOr<T>?`.
    FutureOr { arg: TypeId, nullable: bool },
    /// Tipo de extensão (Dart 3.3+): preserva identidade estática e apaga sob demanda.
    ExtensionType {
        decl: ClassId,
        args: Box<[TypeId]>,
        nullable: bool,
    },
}

impl Type {
    /// Informa se o tipo foi explicitamente anotado como anulável com `?`.
    pub fn is_declared_nullable(&self) -> bool {
        match self {
            Type::Dynamic | Type::Void | Type::Null | Type::Never => false,
            Type::Interface { nullable, .. }
            | Type::Function { nullable, .. }
            | Type::Record { nullable, .. }
            | Type::TypeParameter { nullable, .. }
            | Type::FutureOr { nullable, .. }
            | Type::ExtensionType { nullable, .. } => *nullable,
        }
    }
}

/// Tabela hash-consed de tipos.
///
/// Sobrevive à sessão inteira do compilador e cresce com o carregamento do
/// SDK e dos pacotes. Garante que cada tipo estrutural idêntico receba
/// exatamente o mesmo [`TypeId`].
#[derive(Debug)]
pub struct TypeTable {
    types: Vec<Type>,
    lookup: HashMap<Type, TypeId>,
    type_params: Vec<TypeParameterData>,
    payload_bytes: usize,
}

impl Default for TypeTable {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeTable {
    /// Cria uma nova tabela vazia.
    pub fn new() -> Self {
        Self {
            types: Vec::with_capacity(1024),
            lookup: HashMap::with_capacity(1024),
            type_params: Vec::with_capacity(256),
            payload_bytes: 0,
        }
    }

    /// Retorna o número de tipos únicos internados na tabela.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Informa se a tabela está vazia.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Retorna o volume em bytes ocupado pelas estruturas internadas na tabela.
    pub fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Obtém a referência para a definição do tipo pelo seu [`TypeId`].
    pub fn get(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }

    /// Obtém os dados de um parâmetro de tipo pelo seu [`TypeParamId`].
    pub fn param(&self, id: TypeParamId) -> &TypeParameterData {
        &self.type_params[id.0 as usize]
    }

    /// Obtém referência mutável aos dados de um parâmetro de tipo.
    pub fn param_mut(&mut self, id: TypeParamId) -> &mut TypeParameterData {
        &mut self.type_params[id.0 as usize]
    }

    /// Registra um novo parâmetro de tipo na tabela.
    pub fn alloc_type_param(
        &mut self,
        name: SymbolId,
        owner: TypeParamOwner,
        bound: TypeId,
        variance: Variance,
    ) -> TypeParamId {
        let id = TypeParamId(self.type_params.len() as u32);
        self.type_params.push(TypeParameterData {
            name,
            owner,
            bound,
            variance,
        });
        self.payload_bytes += std::mem::size_of::<TypeParameterData>();
        id
    }

    /// Atualiza o limite (*bound*) de um parâmetro de tipo (útil em recursão mútua / F-bounds).
    pub fn set_type_param_bound(&mut self, id: TypeParamId, bound: TypeId) {
        self.type_params[id.0 as usize].bound = bound;
    }

    /// Interna um tipo estrutural na tabela, garantindo unicidade por hash-consing.
    ///
    /// Garante também canonicidade ordenando campos nomeados em registros e tipos de função.
    pub fn intern(&mut self, mut ty: Type) -> TypeId {
        // Ordenação canônica para unicidade de hash e igualdade
        match &mut ty {
            Type::Function { named, .. } => {
                named.sort_by_key(|(sym, _, _)| sym.as_u32());
            }
            Type::Record { named, .. } => {
                named.sort_by_key(|(sym, _)| sym.as_u32());
            }
            _ => {}
        }

        if let Some(&existing) = self.lookup.get(&ty) {
            return existing;
        }

        let extra_bytes = match &ty {
            Type::Interface { args, .. } => args.len() * std::mem::size_of::<TypeId>(),
            Type::Function {
                type_params,
                positional,
                optional,
                named,
                ..
            } => {
                type_params.len() * std::mem::size_of::<TypeParamId>()
                    + (positional.len() + optional.len()) * std::mem::size_of::<TypeId>()
                    + named.len() * std::mem::size_of::<(SymbolId, TypeId, bool)>()
            }
            Type::Record {
                positional, named, ..
            } => {
                positional.len() * std::mem::size_of::<TypeId>()
                    + named.len() * std::mem::size_of::<(SymbolId, TypeId)>()
            }
            Type::ExtensionType { args, .. } => args.len() * std::mem::size_of::<TypeId>(),
            _ => 0,
        };

        let id = TypeId(self.types.len() as u32);
        self.payload_bytes +=
            std::mem::size_of::<Type>() + extra_bytes + std::mem::size_of::<(Type, TypeId)>() + 8;
        self.lookup.insert(ty.clone(), id);
        self.types.push(ty);
        id
    }
}

/// Tipos fundamentais do Dart (`dart:core` e `dart:async`) cacheados para consultas rápidas.
#[derive(Debug, Clone)]
pub struct CoreTypes {
    pub dynamic_: TypeId,
    pub void_: TypeId,
    pub never: TypeId,
    pub null: TypeId,
    pub object: TypeId,
    pub object_nullable: TypeId,
    pub int: TypeId,
    pub num: TypeId,
    pub string: TypeId,
    pub bool_: TypeId,
    pub function: TypeId,
    pub record: TypeId,
    pub object_class: Option<ClassId>,
    pub int_class: Option<ClassId>,
    pub num_class: Option<ClassId>,
    pub string_class: Option<ClassId>,
    pub bool_class: Option<ClassId>,
    pub function_class: Option<ClassId>,
    pub record_class: Option<ClassId>,
    pub future_class: Option<ClassId>,
    pub iterable_class: Option<ClassId>,
    pub list_class: Option<ClassId>,
    pub map_class: Option<ClassId>,
    pub core_library: Option<LibraryId>,
    pub async_library: Option<LibraryId>,
}

impl CoreTypes {
    /// Inicializa os tipos fundamentais registrando-os na [`TypeTable`] e
    /// resolvendo os símbolos em `dart:core` e `dart:async` a partir do [`Program`].
    pub fn init(table: &mut TypeTable, program: &Program, interner: &Interner) -> Self {
        let dynamic_ = table.intern(Type::Dynamic);
        let void_ = table.intern(Type::Void);
        let never = table.intern(Type::Never);
        let null = table.intern(Type::Null);

        let core_lib = program.core;
        let mut async_lib = None;
        for (i, lib) in program.libraries.iter().enumerate() {
            if lib.uri == "dart:async" {
                async_lib = Some(LibraryId(i as u32));
                break;
            }
        }

        let find_class = |lib_id: Option<LibraryId>, name: &str| -> Option<ClassId> {
            let lib = lib_id?;
            let sym = interner.lookup(name)?;
            let binding = program.lookup(lib, sym)?;
            match binding.getter {
                Some(dartforge_elements::model::Element::Class(cid)) => Some(cid),
                _ => None,
            }
        };

        let object_class = find_class(core_lib, "Object");
        let int_class = find_class(core_lib, "int");
        let num_class = find_class(core_lib, "num");
        let string_class = find_class(core_lib, "String");
        let bool_class = find_class(core_lib, "bool");
        let function_class = find_class(core_lib, "Function");
        let record_class = find_class(core_lib, "Record");
        let iterable_class = find_class(core_lib, "Iterable");
        let list_class = find_class(core_lib, "List");
        let map_class = find_class(core_lib, "Map");
        let future_class = find_class(async_lib, "Future");

        let make_interface =
            |table: &mut TypeTable, class_opt: Option<ClassId>, nullable: bool| -> TypeId {
                if let Some(class) = class_opt {
                    table.intern(Type::Interface {
                        class,
                        args: Box::new([]),
                        nullable,
                    })
                } else {
                    // Fallback para quando o SDK não foi carregado ainda (testes unitários isolados)
                    if nullable { dynamic_ } else { never }
                }
            };

        let object = make_interface(table, object_class, false);
        let object_nullable = make_interface(table, object_class, true);
        let int = make_interface(table, int_class, false);
        let num = make_interface(table, num_class, false);
        let string = make_interface(table, string_class, false);
        let bool_ = make_interface(table, bool_class, false);
        let function = make_interface(table, function_class, false);
        let record = make_interface(table, record_class, false);

        Self {
            dynamic_,
            void_,
            never,
            null,
            object,
            object_nullable,
            int,
            num,
            string,
            bool_,
            function,
            record,
            object_class,
            int_class,
            num_class,
            string_class,
            bool_class,
            function_class,
            record_class,
            future_class,
            iterable_class,
            list_class,
            map_class,
            core_library: core_lib,
            async_library: async_lib,
        }
    }
}
