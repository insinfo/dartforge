//! Tipos de extensão (Dart 3.3) apagados: o que o lowering precisa saber
//! deles, calculado antes, enquanto a tabela de tipos ainda aceita tipos
//! novos (o `Context` a vê só para leitura).
//!
//! Em tempo de execução um valor de tipo de extensão **é** o valor da
//! representação: `Id(3)` é o `int` 3, `is Id` é `is int`, e o `runtimeType`
//! de `<Id>[]` é `List<int>`. O tipo estático só decide a rota dos membros,
//! que é estática (`lower/extension_types.rs`). Daqui saem:
//!
//! * o apagamento de cada tipo da tabela (`E<A>` → a representação de `E`
//!   com `A`, recursivamente, dentro de listas, funções e records);
//! * o tipo de `this` de cada tipo de extensão (`E<T>`, com os próprios
//!   parâmetros): é o padrão com que os argumentos de tipo de um membro são
//!   tirados do tipo estático do receptor;
//! * a instância de cada supertipo de extensão de cada tipo de extensão da
//!   tabela (`Filho<int>` implementa `Base<List<int>>`), para achar esses
//!   argumentos quando o membro é herdado.

use dartforge_elements::model::{ClassId, ClassKind, Program};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::table::{Type, TypeId, TypeParamId, TypeTable};
use std::collections::HashMap;

/// O apagamento e os tipos auxiliares dos tipos de extensão do programa.
#[derive(Debug, Default)]
pub struct ExtensionTypes {
    /// O tipo apagado de cada tipo da tabela (pelo índice); vazio quando o
    /// programa não tem tipo de extensão.
    erased_ones: Vec<TypeId>,
    /// `E<T…>` de cada tipo de extensão, com os próprios parâmetros.
    pub this: HashMap<ClassId, TypeId>,
    /// `(E<A…>, S) → S<B…>`: a instância do supertipo de extensão `S` de
    /// `E<A…>`.
    pub supertypes: HashMap<(TypeId, ClassId), TypeId>,
}

impl ExtensionTypes {
    /// O tipo apagado de `t` (o próprio, sem tipo de extensão dentro).
    pub fn erase(&self, t: TypeId) -> TypeId {
        self.erased_ones.get(t.0 as usize).copied().unwrap_or(t)
    }
}

/// Os parâmetros de tipo de `c` e o tipo da representação, escrito com eles.
fn representation(program: &Program, outline: &OutlineTypes, c: ClassId) -> Option<(Vec<TypeParamId>, TypeId)> {
    let v = program.class(c).representation?;
    let t = outline.variables.get(v.0 as usize)?.declared_type?;
    let params = outline.classes.get(c.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
    Some((params, t))
}

/// Calcula o apagamento de toda a tabela (ver o topo do módulo).
pub fn compute(program: &Program, outline: &OutlineTypes, table: &mut TypeTable) -> ExtensionTypes {
    let types: Vec<ClassId> = (0..program.classes.len() as u32)
        .map(ClassId)
        .filter(|&c| program.class(c).kind == ClassKind::ExtensionType)
        .collect();
    let mut r = ExtensionTypes::default();
    if types.is_empty() {
        return r;
    }
    for &c in &types {
        let params = outline.classes.get(c.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let args: Vec<TypeId> =
            params.iter().map(|&p| table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let t = table.intern(Type::ExtensionType { decl: c, args: args.into_boxed_slice(), nullable: false });
        r.this.insert(c, t);
    }
    // Os supertipos de extensão de cada tipo de extensão da tabela, pelo
    // `implements` (com a substituição dos argumentos), até o fecho.
    let mut i = 0;
    while i < table.len() {
        let t = TypeId(i as u32);
        i += 1;
        let Type::ExtensionType { decl, args, .. } = table.get(t).clone() else { continue };
        let mut pending_set = vec![(decl, args.to_vec())];
        while let Some((d, a)) = pending_set.pop() {
            let Some(data) = outline.classes.get(d.0 as usize) else { continue };
            let map: HashMap<TypeParamId, TypeId> = data.type_params.iter().copied().zip(a.iter().copied()).collect();
            for &s in data.interfaces.iter() {
                let Type::ExtensionType { decl: sd, .. } = table.get(s).clone() else { continue };
                let inst = dartforge_types::substitute(s, &map, table);
                if r.supertypes.insert((t, sd), inst).is_none()
                    && let Type::ExtensionType { args: sa, .. } = table.get(inst).clone()
                {
                    pending_set.push((sd, sa.to_vec()));
                }
            }
        }
    }
    let rep = |c: ClassId, args: &[TypeId], table: &mut TypeTable| -> Option<TypeId> {
        let (params, t) = representation(program, outline, c)?;
        let map: HashMap<TypeParamId, TypeId> = params.into_iter().zip(args.iter().copied()).collect();
        Some(dartforge_types::substitute(t, &map, table))
    };
    // Os tipos que o apagamento cria já saem apagados: basta a tabela de
    // antes dele.
    let n = table.len();
    r.erased_ones = (0..n as u32).map(TypeId).collect();
    for i in 0..n {
        r.erased_ones[i] = dartforge_types::erase_extension_type(TypeId(i as u32), table, &rep);
    }
    r
}
