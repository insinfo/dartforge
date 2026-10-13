//! `dart:ffi` (NATIVO-PLANO, FFI): o que o transformador de FFI do
//! front-end da VM e o compilador dela fazem, aqui no lowering do programa.
//!
//! * **Tipos nativos.** Cada classe de tipo nativo que uma assinatura pode
//!   usar tem um [`CType`]: os primitivos (`Int8`…`Double`, `Bool`), o
//!   `Pointer` e os inteiros específicos da ABI (`Long`, `Size`, `WChar`…, do
//!   SDK e do programa), resolvidos para o alvo pela anotação
//!   `@AbiSpecificIntegerMapping`. A tabela vai ao runtime
//!   (`dartforge_ffi_registrar_tipo`), que a usa em `sizeOf` e para achar o
//!   trampolim de uma assinatura pela RTI.
//! * **Trampolins.** Toda assinatura nativa `NativeFunction<F>` que aparece
//!   no programa (a tabela de tipos inteira, depois da inferência) ganha um
//!   trampolim: uma entrada da convenção uniforme das closures que confere a
//!   aridade, converte os argumentos Dart para os tipos C, chama a função
//!   nativa pelo endereço guardado na closure ([`Instruction::NativeCall`],
//!   com a extensão dos inteiros estreitos que a ABI exige) e converte o
//!   retorno. `asFunction`/`lookupFunction` (a sobreposição de `dart:ffi`)
//!   pedem ao runtime a closure com o trampolim da assinatura (a chave é a
//!   mesma dos dois lados: [`signature_key`]).
//!
//! * **Compostos, variádicas e `Handle`.** Structs e unions por valor
//!   ([`NativeType::Composite`], com o layout C do alvo), a parte variádica de
//!   `VarArgs` (inclusive com structs, pela regra variádica do alvo) e
//!   `Handle` (o objeto Dart na fronteira) também baixam
//!   (`corpus/native/16_ffi_structs_por_valor.dart`, `17_ffi_varargs.dart`,
//!   `18_ffi_handle.dart`).
//!
//! O que continua recusado — inteiro específico da ABI sem mapeamento para o
//! alvo, struct vazia por valor,
//! assinatura com parâmetros opcionais, nomeados ou genéricos — vira erro do
//! runtime com o motivo, nunca uma chamada com a ABI errada.

use super::closures::{DefaultValue, EntryParam};
use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::ClassId;
use dartforge_frontend::ast::{self, ParameterKind};
use dartforge_types::table::Type as T;
use dartforge_types::TypeId;

/// O nome da ABI do alvo no enum `Abi` do `dart:ffi` (a chave das
/// anotações `@AbiSpecificIntegerMapping`).
fn target_abi() -> &'static str {
    let arm = cfg!(target_arch = "aarch64");
    match crate::alvo::sistema() {
        crate::alvo::Sistema::Windows => if arm { "windowsArm64" } else { "windowsX64" },
        crate::alvo::Sistema::Linux => if arm { "linuxArm64" } else { "linuxX64" },
        crate::alvo::Sistema::MacOs => if arm { "macosArm64" } else { "macosX64" },
    }
}

/// O tipo C de um primitivo de `dart:ffi` pelo nome da classe.
fn primitive(name_str: &str) -> Option<CType> {
    Some(match name_str {
        "Int8" => CType::I8,
        "Uint8" => CType::U8,
        "Int16" => CType::I16,
        "Uint16" => CType::U16,
        "Int32" => CType::I32,
        "Uint32" => CType::U32,
        "Int64" => CType::I64,
        "Uint64" => CType::U64,
        "Float" => CType::F32,
        "Double" => CType::F64,
        "Bool" => CType::Bool,
        "Void" => CType::Void,
        "Pointer" => CType::Ptr,
        _ => return None,
    })
}

/// As classes de tipo nativo do programa com o tipo C de cada uma.
pub struct NativeTypes {
    classes: std::collections::HashMap<ClassId, CType>,
    /// Classes que existem em `dart:ffi` mas ainda não baixam por valor
    /// (`Handle`, `VarArgs`), com o motivo.
    rejected: std::collections::HashMap<ClassId, &'static str>,
    /// As subclasses de `Struct` e `Union`: por valor, com o layout C.
    composites: std::collections::HashSet<ClassId>,
    /// `VarArgs` (o último parâmetro de uma função variádica).
    varargs: Option<ClassId>,
    /// `Array` (vetor C de tamanho fixo: campo ou variável `@Array`).
    array: Option<ClassId>,
}

impl NativeTypes {
    pub fn of_program(ctx: &Context) -> Option<NativeTypes> {
        let ffi = ctx.program.libraries.iter().position(|l| l.uri == "dart:ffi")?;
        let ffi = dartforge_elements::model::LibraryId(ffi as u32);
        let mut classes = std::collections::HashMap::new();
        let mut rejected = std::collections::HashMap::new();
        let mut composites = std::collections::HashSet::new();
        let mut varargs = None;
        let mut array = None;
        let specific_abi = ctx.sdk_class("ffi", "AbiSpecificInteger");
        let struct_ = ctx.sdk_class("ffi", "Struct");
        let union_ = ctx.sdk_class("ffi", "Union");
        for (i, c) in ctx.program.classes.iter().enumerate() {
            let id = ClassId(i as u32);
            let name_str = ctx.symbol_name(c.name);
            if c.library == ffi {
                if let Some(t) = primitive(name_str) {
                    classes.insert(id, t);
                    continue;
                }
                match name_str {
                    "Handle" => {
                        classes.insert(id, CType::Handle);
                    }
                    "VarArgs" => varargs = Some(id),
                    "Array" => array = Some(id),
                    _ => {}
                }
            }
            if c.supertype_class.is_some() && c.supertype_class == specific_abi {
                match abi_mapping(ctx, id) {
                    Some(t) => {
                        classes.insert(id, t);
                    }
                    None => {
                        rejected.insert(id, "inteiro específico da ABI sem mapeamento para este alvo");
                    }
                }
            } else if c.supertype_class.is_some() && (c.supertype_class == struct_ || c.supertype_class == union_) {
                composites.insert(id);
            }
        }
        Some(NativeTypes { classes, rejected, composites, varargs, array })
    }

    /// O tipo C de um tipo nativo (argumento ou retorno de uma assinatura).
    fn c_type(&self, ctx: &Context, t: TypeId) -> Result<CType, String> {
        match ctx.table.get(t) {
            T::Interface { class, .. } => {
                if let Some(tc) = self.classes.get(class) {
                    return Ok(*tc);
                }
                if let Some(m) = self.rejected.get(class) {
                    return Err(format!("{m} ainda não é suportado no backend nativo"));
                }
                Err(format!("`{}` não é um tipo nativo", ctx.symbol_name(ctx.program.classes[class.0 as usize].name)))
            }
            _ => Err("tipo nativo não é uma classe de `dart:ffi`".to_string()),
        }
    }

    /// O tipo nativo de um argumento ou retorno: primitivo, ou struct/union
    /// por valor (com a classe, para criar o valor devolvido).
    fn native_type(&self, ctx: &Context, t: TypeId) -> Result<(NativeType, Option<ClassId>), String> {
        if let T::Interface { class, .. } = ctx.table.get(t)
            && self.composites.contains(class)
        {
            let layout = layout_c(ctx, *class).ok_or_else(|| {
                format!("struct `{}` sem layout", ctx.symbol_name(ctx.program.classes[class.0 as usize].name))
            })?;
            if layout.size == 0 {
                return Err("struct vazia por valor".to_string());
            }
            return Ok((NativeType::Composite(layout), Some(*class)));
        }
        Ok((NativeType::Prim(self.c_type(ctx, t)?), None))
    }

    /// A assinatura de uma função nativa `F` (de `NativeFunction<F>`), com
    /// structs e unions por valor.
    pub fn native_signature(&self, ctx: &Context, f: TypeId) -> Result<NativeSignature, String> {
        match ctx.table.get(f) {
            T::Function { type_params, ret, positional, optional, named, .. } => {
                if !type_params.is_empty() || !optional.is_empty() || !named.is_empty() {
                    return Err("assinatura nativa com parâmetros opcionais, nomeados ou genéricos".to_string());
                }
                let (r, ret_class) = self.native_type(ctx, *ret)?;
                let mut ps = Vec::with_capacity(positional.len());
                let mut cs = Vec::with_capacity(positional.len());
                let mut variadic_flag = None;
                for (i, p) in positional.iter().enumerate() {
                    // `VarArgs<(T1, T2…)>` ou `VarArgs<T>`, só no fim: os
                    // tipos da parte variádica.
                    let variadic_set = match ctx.table.get(*p) {
                        T::Interface { class, args, .. } if Some(*class) == self.varargs => {
                            if i + 1 != positional.len() {
                                return Err("VarArgs fora do último parâmetro".to_string());
                            }
                            variadic_flag = Some(i);
                            match args.first().map(|a| ctx.table.get(*a)) {
                                Some(T::Record { positional, named, .. }) if named.is_empty() => positional.to_vec(),
                                Some(_) => args.to_vec(),
                                None => return Err("VarArgs sem argumento de tipo".to_string()),
                            }
                        }
                        _ => vec![*p],
                    };
                    for v in variadic_set {
                        match self.native_type(ctx, v)? {
                            (NativeType::Prim(CType::Void), _) => return Err("parâmetro nativo `Void`".to_string()),
                            (t, c) => {
                                ps.push(t);
                                cs.push(c);
                            }
                        }
                    }
                }
                Ok(NativeSignature { ret: r, ret_class, params: ps, class_params: cs, variadic_flag })
            }
            _ => Err("NativeFunction sem tipo de função".to_string()),
        }
    }

    /// As assinaturas `F` de todo `NativeFunction<F>` concreto do programa.
    fn program_signatures(&self, ctx: &Context) -> Vec<TypeId> {
        // Não só os `NativeFunction<F>` concretos: `lookupFunction<NS, DS>`
        // e `asFunction` instanciam `NativeFunction<NS>` dentro do corpo
        // genérico, e a assinatura concreta aparece só como argumento de
        // tipo da chamada. Todo tipo de função feito só de tipos nativos é
        // uma assinatura candidata (os tipos Dart — `int`, `void` — nunca
        // são classes nativas, então uma assinatura Dart não entra por
        // engano, e uma entrada a mais só custa um trampolim).
        let mut seen_set = std::collections::HashSet::new();
        let mut exit = Vec::new();
        for i in 0..ctx.table.len() {
            let f = TypeId(i as u32);
            if matches!(ctx.table.get(f), T::Function { .. })
                && !contains_type_parameter(ctx, f)
                && self.native_signature(ctx, f).is_ok()
                && seen_set.insert(f)
            {
                exit.push(f);
            }
        }
        exit
    }

    /// A tabela (id RTI da classe, letra do tipo C) que o runtime recebe.
    pub fn rti_table(&self, ctx: &Context) -> Vec<(i64, char)> {
        let mut v: Vec<(i64, char)> = self.classes.iter().map(|(c, t)| (ctx.id_rti(*c), t.letter())).collect();
        // `VarArgs`: a marca da parte variádica na chave.
        v.extend(self.varargs.map(|c| (ctx.id_rti(c), '*')));
        v.sort_unstable();
        v
    }
}

/// Se o tipo menciona um parâmetro de tipo (não é uma assinatura concreta).
fn contains_type_parameter(ctx: &Context, t: TypeId) -> bool {
    match ctx.table.get(t) {
        T::TypeParameter { .. } => true,
        T::Interface { args, .. } => args.iter().any(|a| contains_type_parameter(ctx, *a)),
        T::Function { ret, positional, optional, named, .. } => {
            contains_type_parameter(ctx, *ret)
                || positional.iter().chain(optional.iter()).any(|a| contains_type_parameter(ctx, *a))
                || named.iter().any(|(_, a, _)| contains_type_parameter(ctx, *a))
        }
        _ => false,
    }
}

/// O tipo C que `@AbiSpecificIntegerMapping({Abi.x: Int32(), …})` dá à
/// classe no alvo.
fn abi_mapping(ctx: &Context, c: ClassId) -> Option<CType> {
    let decl = ctx.program.classes[c.0 as usize].decl?;
    let unit = ctx.program.unit(decl.unit);
    let a = &unit.ast;
    let alvo = target_abi();
    for an in a.decls[decl.decl.0 as usize].metadata.iter() {
        if an.name.last().map(|n| ctx.interner.resolve(n.sym)) != Some("AbiSpecificIntegerMapping") {
            continue;
        }
        let arg = an.arguments.as_ref()?.args.first()?;
        let ast::ExprKind::SetOrMap { elements, .. } = &a.exprs[arg.value.0 as usize].kind else {
            return None;
        };
        for e in elements.iter() {
            let ast::CollectionElement::MapEntry { key, value, .. } = e else { continue };
            let map_key = match &a.exprs[key.0 as usize].kind {
                ast::ExprKind::Property { name, .. } => ctx.interner.resolve(name.sym),
                _ => continue,
            };
            if map_key != alvo {
                continue;
            }
            let cls = match &a.exprs[value.0 as usize].kind {
                ast::ExprKind::Call { target, .. } => match &a.exprs[target.0 as usize].kind {
                    ast::ExprKind::Identifier(n) => ctx.interner.resolve(n.sym),
                    _ => return None,
                },
                ast::ExprKind::InstanceCreation { ty, .. } => match &a.types[ty.0 as usize].kind {
                    ast::TypeKind::Named { name, .. } => ctx.interner.resolve(name.last()?.sym),
                    _ => return None,
                },
                _ => return None,
            };
            return primitive(cls).filter(|t| !matches!(t, CType::Void | CType::Ptr | CType::F32 | CType::F64 | CType::Bool));
        }
    }
    None
}

/// A chave num nome de símbolo, sem ambiguidade: `.` (fim de `S<rti>.`)
/// vira `$p` e `*` (início dos variádicos) vira `$v`.
fn escape_key(map_key: &str) -> String {
    map_key.replace('.', "$p").replace('*', "$v")
}

/// Uma assinatura nativa: retorno (com a classe de uma struct devolvida
/// por valor) e parâmetros.
pub struct NativeSignature {
    pub ret: NativeType,
    pub ret_class: Option<ClassId>,
    pub params: Vec<NativeType>,
    pub class_params: Vec<Option<ClassId>>,
    /// Função variádica: quantos parâmetros são fixos.
    pub variadic_flag: Option<usize>,
}

/// A chave de uma assinatura: a letra do retorno, `_` e as dos parâmetros
/// (`i_ip` = `Int32 Function(Int32, Pointer)`); uma struct por valor é
/// `S<id RTI da class>.`. O runtime calcula a mesma chave a partir da RTI.
pub fn signature_key(ctx: &Context, a: &NativeSignature) -> String {
    let letter = |t: &NativeType, class: Option<ClassId>, s: &mut String| match (t, class) {
        (NativeType::Prim(tc), _) => s.push(tc.letter()),
        (NativeType::Composite(_), Some(c)) => s.push_str(&format!("S{}.", ctx.id_rti(c))),
        (NativeType::Composite(_), None) => s.push('?'),
    };
    let mut s = String::with_capacity(a.params.len() + 2);
    letter(&a.ret, a.ret_class, &mut s);
    s.push('_');
    for (i, (p, c)) in a.params.iter().zip(&a.class_params).enumerate() {
        if a.variadic_flag == Some(i) {
            s.push('*');
        }
        letter(p, *c, &mut s);
    }
    if a.variadic_flag == Some(a.params.len()) {
        s.push('*');
    }
    s
}

/// Gera os trampolins das assinaturas do programa e as tabelas que o
/// runtime recebe na preparação do isolado.
pub fn lower_ffi(ctx: &Context, module: &mut Module) {
    let Some(types) = NativeTypes::of_program(ctx) else { return };
    module.ffi_types = types.rti_table(ctx);
    if let Some(comp) = composites(ctx) {
        let idx = |v: dartforge_elements::model::VariableId, c: ClassId| {
            super::members::layout(ctx, c).iter().position(|&x| x == v).map_or(-1, |i| (i + super::enums::layout_base(ctx, c)) as i64)
        };
        let mut v: Vec<FfiComposite> = comp
            .classes
            .iter()
            .filter_map(|(c, l)| {
                Some(FfiComposite {
                    rti: ctx.id_rti(*c),
                    class: i64::from(ctx.class_id_of(*c)?),
                    fields: (super::members::layout(ctx, *c).len() + super::enums::layout_base(ctx, *c)) as i64,
                    base_index: idx(comp.base, *c),
                    offset_index: idx(comp.offset, *c),
                    size: l.size as i64,
                    alignment: l.alignment as i64,
                })
            })
            .collect();
        v.sort_by_key(|x| x.rti);
        module.ffi_composites = v;
    }
    let mut done = std::collections::HashSet::new();
    for f in types.program_signatures(ctx) {
        // Uma assinatura que não baixa fica sem trampolim: o runtime recusa
        // a chamada com a assinatura.
        let Ok(signature) = types.native_signature(ctx, f) else { continue };
        let map_key = signature_key(ctx, &signature);
        if !done.insert(map_key.clone()) {
            continue;
        }
        let symbol = format!("df.ffi.{}$ent", escape_key(&map_key));
        let unit = dartforge_elements::model::UnitId(0);
        let mut b = FnBuilder::new(ctx, unit, symbol.clone(), format!("ffi {map_key}"), Type::Ref);
        b.trampoline_body(&signature);
        module.functions.push(b.func);
        module.functions.extend(b.extra_functions);
        module.dart_scalar_parameters.extend(b.dart_scalar_parameters);
        module.dart_rti_parameters.extend(b.dart_rti_parameters);
        module.dart_vector_parameters.extend(b.dart_vector_parameters);
        module.arc_uniform_returns.extend(b.arc_uniform_returns);
        module.dart_ref_returns.extend(b.dart_ref_returns);
        module.arc_rti_caches.extend(b.arc_rti_caches);
        module.arc_shared_functions.extend(b.arc_shared_functions);
        module.arc_direct_local_functions.extend(b.arc_direct_local_functions);
        module.globals.extend(b.extra_globals);
        module.ffi_trampolines.push((map_key.clone(), symbol));
        // O callback da mesma assinatura (`Pointer.fromFunction`,
        // `NativeCallable`): o corpo HIR; a entrada C sai no emissor. Um
        // callback nunca é variádico.
        if signature.variadic_flag.is_some() {
            continue;
        }
        let body = format!("df.ffi.{}$cb", escape_key(&map_key));
        let mut b = FnBuilder::new(ctx, unit, body.clone(), format!("ffi callback {map_key}"), signature.ret.hir_type());
        b.callback_body(ctx, &signature);
        module.functions.push(b.func);
        module.functions.extend(b.extra_functions);
        module.dart_scalar_parameters.extend(b.dart_scalar_parameters);
        module.dart_rti_parameters.extend(b.dart_rti_parameters);
        module.dart_vector_parameters.extend(b.dart_vector_parameters);
        module.arc_uniform_returns.extend(b.arc_uniform_returns);
        module.dart_ref_returns.extend(b.dart_ref_returns);
        module.arc_rti_caches.extend(b.arc_rti_caches);
        module.arc_shared_functions.extend(b.arc_shared_functions);
        module.arc_direct_local_functions.extend(b.arc_direct_local_functions);
        module.globals.extend(b.extra_globals);
        module.ffi_callbacks.push(FfiCallback { key: map_key, body, ret: signature.ret.clone(), params: signature.params.clone() });
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O que o front-end da VM reescreve nas chamadas estáticas de `dart:ffi`
    /// (`Pointer.fromFunction`, as factories `NativeCallable.isolateLocal` e
    /// `.listener`, `Struct.create`/`Union.create`): o ajudante da
    /// sobreposição que as implementa, com os mesmos parâmetros, e como os
    /// argumentos de tipo e o sítio chegam a ele.
    pub fn redirected_ffi_call(&self, fid: usize) -> Option<(usize, Redirection)> {
        let f = &self.ctx.program.functions[fid];
        let c = f.class?;
        if self.ctx.program.library(f.library).uri != "dart:ffi" {
            return None;
        }
        let cls = self.ctx.symbol_name(self.ctx.program.classes[c.0 as usize].name);
        let name_str = self.ctx.symbol_name(f.name);
        let (helper, shape) = match (cls, name_str, f.factory) {
            ("Pointer", "fromFunction", false) if f.static_ => ("_dartforgeFromFunction", Redirection::WithSite),
            ("NativeCallable", "isolateLocal", true) => ("_dartforgeCallableLocal", Redirection::ClassTuple),
            ("NativeCallable", "listener", true) => ("_dartforgeCallableListener", Redirection::ClassTuple),
            ("Struct" | "Union", "create", false) if f.static_ => ("_dartforgeCompostoCriado", Redirection::DirectOnly),
            _ => return None,
        };
        Some((self.top_level_function("dart:ffi", helper)?, shape))
    }

    /// O corpo de um callback nativo: `(context, arguments)` na
    /// representação Dart (um ponteiro chega como endereço) → a closure do
    /// callback pela convenção uniforme → o retorno na representação Dart
    /// (um `Pointer` volta como endereço). Uma exceção da closure fica
    /// pendente: a entrada C devolve o retorno excepcional
    /// (`ffi_callbacks.rs`).
    fn callback_body(&mut self, ctx: &Context, a: &NativeSignature) {
        let context = Operand::Val(self.add_param("ctx".to_string(), Type::I64));
        let values: Vec<Operand> =
            a.params.iter().enumerate().map(|(i, t)| Operand::Val(self.add_param(format!("a{i}"), t.hir_type()))).collect();
        // Sob ARC, o contexto e os argumentos chegam da entrada C já em
        // representação escalar (endereços, handles e números): Trivial.
        if self.ctx.arc_memory {
            let scalars: std::collections::HashSet<ValueId> = self
                .func
                .params
                .iter()
                .filter(|(_, _, t)| matches!(t, Type::I64 | Type::F64 | Type::I1 | Type::I8))
                .map(|(v, _, _)| *v)
                .collect();
            self.dart_scalar_parameters.entry(self.func.symbol.clone()).or_default().extend(scalars);
        }
        let clo = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_ffi_callback_closure".to_string(),
                args: vec![(context.clone(), Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        let mut evaluated = Vec::with_capacity(a.params.len());
        for (i, ((v, t), cls)) in values.into_iter().zip(a.params.iter()).zip(&a.class_params).enumerate() {
            let x = match (t, cls) {
                (NativeType::Prim(CType::Handle), _) => self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_objeto_do_handle".to_string(),
                        args: vec![(v, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                // O `Pointer` vem do ajudante Dart, que pode lançar.
                (NativeType::Prim(CType::Ptr), _) => self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_callback_ponteiro".to_string(),
                        args: vec![(context.clone(), Type::I64), (Operand::Constant(Constant::Int(i as i64)), Type::I64), (v, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                // Uma struct por valor: uma cópia sobre memória do heap Dart
                // (a VM também a copia para um `TypedData`).
                (NativeType::Composite(_), Some(c)) => self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_composto_copia".to_string(),
                        args: vec![(Operand::Constant(Constant::Int(ctx.id_rti(*c))), Type::I64), (v, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                _ => self.coerce(v, Type::Ref),
            };
            evaluated.push((None, x));
        }
        let r = self.call_function_value(clo, &evaluated);
        if self.is_terminated() {
            return;
        }
        let result = match &a.ret {
            NativeType::Prim(CType::Void) => None,
            NativeType::Prim(CType::Handle) => {
                let r = self.coerce(r, Type::Ref);
                Some(self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_handle_novo".to_string(),
                        args: vec![(r, Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ))
            }
            NativeType::Prim(CType::Ptr) => {
                let r = self.coerce(r, Type::Ref);
                Some(self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_endereco_do_ponteiro".to_string(),
                        args: vec![(r, Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ))
            }
            // O endereço dos bytes da struct devolvida: a entrada C os copia
            // antes de qualquer outra alocação.
            NativeType::Composite(_) => {
                let r = self.coerce(r, Type::Ref);
                Some(self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_endereco_do_composto".to_string(),
                        args: vec![(r, Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ))
            }
            NativeType::Prim(tc) => Some(self.coerce(r, tc.hir_type())),
        };
        if !self.is_terminated() {
            self.terminate(Terminator::Return(result));
        }
    }

    /// O corpo de um trampolim: `(closure, args, desc)` da convenção
    /// uniforme → a chamada nativa → o retorno como `Ref`. Uma struct por
    /// valor vai como o endereço dos bytes dela; uma devolvida por valor é
    /// criada antes (sobre um `Uint8List` novo, como a VM) e a chamada grava
    /// nela.
    fn trampoline_body(&mut self, a: &NativeSignature) {
        let clo = Operand::Val(self.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(self.add_param_vector(DartEntryVector::Argumentos));
        let desc = Operand::Val(self.add_param_vector(DartEntryVector::Descriptor));
        self.declare_arc_uniform_return();
        let infos: Vec<EntryParam> = a
            .params
            .iter()
            .map(|_| EntryParam { name: None, kind: ParameterKind::Required, required: true, pattern: DefaultValue::Absent })
            .collect();
        let Some(values) = self.unpack(&infos, args, desc) else {
            return;
        };
        let alvo = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_ffi_endereco_da_closure".to_string(),
                args: vec![(clo.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let result = self.signature_native_call(alvo, &values, a, Some(clo));
        self.terminate(Terminator::Return(Some(result)));
    }
}

/// O layout C achatado de uma struct ou union do programa.
pub fn layout_c(ctx: &Context, c: ClassId) -> Option<LayoutC> {
    let comp = composites(ctx)?;
    let l = comp.classes.get(&c)?;
    let mut leaves = Vec::new();
    for (offset, field) in l.fields.values() {
        flatten(ctx, field, *offset, &mut leaves)?;
    }
    leaves.sort_by_key(|(o, _)| *o);
    Some(LayoutC { size: l.size, alignment: l.alignment, leaves })
}

/// As folhas primitivas de um campo em `base`.
fn flatten(ctx: &Context, t: &FieldType, base: usize, leaves: &mut Vec<(usize, CType)>) -> Option<()> {
    match t {
        FieldType::Prim(tc) => leaves.push((base, *tc)),
        FieldType::Composite(c) => {
            let inside = layout_c(ctx, *c)?;
            leaves.extend(inside.leaves.iter().map(|(o, tc)| (base + o, *tc)));
        }
        FieldType::Array { elem, dims } => {
            let step = size_of(ctx, elem);
            for i in 0..dims.iter().product::<usize>() {
                flatten(ctx, elem, base + i * step, leaves)?;
            }
        }
    }
    Some(())
}

// ─── Structs e unions (`Struct`, `Union`, `@Array`, `@Packed`) ─────────────
//
// O que o transformador de FFI do front-end da VM faz com uma subclasse de
// `Struct`/`Union`: o layout dos campos `external` na ABI do alvo (C: cada
// campo no próximo múltiplo do alinhamento dele; a struct com o tamanho
// arredondado ao maior alinhamento; `@Packed(n)` limita os alinhamentos; na
// union tudo começa em 0), e cada acesso a um desses campos vira uma carga ou
// gravação em `_typedDataBase` + `_offsetInBytes` + deslocamento do campo
// (`FnBuilder::read_ffi_field`). O runtime recebe o tamanho e o alinhamento
// de cada composto (`sizeOf<S>()`, `Pointer<S>.ref`, `[i]`), por classe.

/// Como uma chamada redirecionada ([`FnBuilder::redirected_ffi_call`])
/// passa ao ajudante o que o front-end da VM resolveria em compilação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redirection {
    /// Os mesmos argumentos e a mesma tupla de tipos.
    DirectOnly,
    /// A tupla de uma factory de classe genérica (o último argumento) vira a
    /// do ajudante genérico.
    ClassTuple,
    /// Mais um argumento: o sítio da chamada (`fromFunction`, um trampolim
    /// por sítio).
    WithSite,
}

/// O tipo de um campo de composto.
#[derive(Debug, Clone)]
pub enum FieldType {
    Prim(CType),
    Composite(ClassId),
    /// `@Array(d0, d1…)` de elementos `elem` (o tipo mais interno).
    Array { elem: Box<FieldType>, dims: Vec<usize> },
}

/// O layout de uma struct ou union.
#[derive(Debug, Clone)]
pub struct Composite {
    pub size: usize,
    pub alignment: usize,
    pub fields: std::collections::HashMap<dartforge_elements::model::VariableId, (usize, FieldType)>,
}

/// Os compostos do programa e os campos de `_Compound` que os acessos usam.
pub struct Composites {
    pub classes: std::collections::HashMap<ClassId, Composite>,
    pub base: dartforge_elements::model::VariableId,
    pub offset: dartforge_elements::model::VariableId,
}

/// Os compostos do programa (calculados uma vez por contexto).
pub fn composites<'c>(ctx: &'c Context) -> Option<&'c Composites> {
    ctx.ffi_composites.get_or_init(|| compute_composites(ctx)).as_ref()
}

fn compute_composites(ctx: &Context) -> Option<Composites> {
    let types = NativeTypes::of_program(ctx)?;
    let compound = ctx.sdk_class("ffi", "_Compound")?;
    let field = |name_str: &str| {
        ctx.program.classes[compound.0 as usize]
            .fields
            .iter()
            .copied()
            .find(|v| ctx.symbol_name(ctx.program.variables[v.0 as usize].name) == name_str)
    };
    let (base, offset) = (field("_typedDataBase")?, field("_offsetInBytes")?);
    let struct_ = ctx.sdk_class("ffi", "Struct");
    let union_ = ctx.sdk_class("ffi", "Union");
    let mut calc = Computation { ctx, types: &types, struct_, union_, done_set: Default::default(), ongoing: Default::default() };
    for i in 0..ctx.program.classes.len() {
        let c = ClassId(i as u32);
        if calc.is_composite(c) {
            calc.layout(c);
        }
    }
    Some(Composites { classes: calc.done_set.into_iter().filter_map(|(c, l)| Some((c, l?))).collect(), base, offset })
}

struct Computation<'a, 'c> {
    ctx: &'a Context<'c>,
    types: &'a NativeTypes,
    struct_: Option<ClassId>,
    union_: Option<ClassId>,
    done_set: std::collections::HashMap<ClassId, Option<Composite>>,
    ongoing: std::collections::HashSet<ClassId>,
}

impl Computation<'_, '_> {
    fn is_composite(&self, c: ClassId) -> bool {
        let s = self.ctx.program.classes[c.0 as usize].supertype_class;
        s.is_some() && (s == self.struct_ || s == self.union_)
    }

    fn is_union(&self, c: ClassId) -> bool {
        self.union_.is_some() && self.ctx.program.classes[c.0 as usize].supertype_class == self.union_
    }

    /// `(size, alignment)` de um tipo de campo.
    fn measure(&mut self, t: &FieldType) -> Option<(usize, usize)> {
        match t {
            FieldType::Prim(tc) => Some((tc.c_size(), tc.c_size())),
            FieldType::Composite(c) => self.layout(*c).map(|l| (l.size, l.alignment)),
            FieldType::Array { elem, dims } => {
                let (t, a) = self.measure(elem)?;
                Some((t * dims.iter().product::<usize>(), a))
            }
        }
    }

    fn layout(&mut self, c: ClassId) -> Option<Composite> {
        if let Some(l) = self.done_set.get(&c) {
            return l.clone();
        }
        if !self.ongoing.insert(c) {
            // Composto que contém a si mesmo por valor: recusado.
            return None;
        }
        let r = self.compute(c);
        self.ongoing.remove(&c);
        self.done_set.insert(c, r.clone());
        r
    }

    fn compute(&mut self, c: ClassId) -> Option<Composite> {
        let ctx = self.ctx;
        let packed = full_annotation(ctx, c, "Packed");
        let union_ = self.is_union(c);
        let mut offset = 0usize;
        let mut largest_alignment = 1usize;
        let mut max_size = 0usize;
        let mut fields = std::collections::HashMap::new();
        for &vid in &ctx.program.classes[c.0 as usize].fields {
            let v = &ctx.program.variables[vid.0 as usize];
            if v.static_ || !v.external {
                continue;
            }
            let typ = self.field_type(vid)?;
            let (t, mut a) = self.measure(&typ)?;
            if let Some(p) = packed {
                a = a.min(p.max(1) as usize);
            }
            let a = a.max(1);
            largest_alignment = largest_alignment.max(a);
            let off = if union_ { 0 } else { offset.div_ceil(a) * a };
            fields.insert(vid, (off, typ));
            offset = off + t;
            max_size = max_size.max(t);
        }
        let raw = if union_ { max_size } else { offset };
        let size = raw.div_ceil(largest_alignment) * largest_alignment;
        Some(Composite { size, alignment: largest_alignment, fields })
    }

    /// O tipo nativo de um campo: pela anotação (`@Int32()`, `@Long()`,
    /// `@Array(…)`) ou pelo tipo declarado (`Pointer<…>`, outra struct).
    fn field_type(&mut self, vid: dartforge_elements::model::VariableId) -> Option<FieldType> {
        let ctx = self.ctx;
        let declared = super::members::variable_type(ctx, vid);
        let dartforge_elements::model::VariableRef::Field { unit, member, .. } = ctx.program.variables[vid.0 as usize].node else {
            return None;
        };
        let u = ctx.program.unit(unit);
        let ast = &u.ast;
        for an in ast.members[member.0 as usize].metadata.iter() {
            // A classe que a anotação nomeia, pelo escopo da biblioteca: um
            // alias (`typedef DWORD = Uint32;` e `@DWORD()`, as estruturas
            // do Win32 do prometheus_client no new_sali/backend) e um prefixo
            // (`@ffi.Int32()`) levam à classe do `dart:ffi`. Pelo texto, o
            // campo ficava sem tipo, a struct sem layout e o `.ref` lançava
            // "struct or union not registered" (docs/NATIVO-PROJETOS-REAIS.md, C13).
            let name_str = match self.annotation_class(u.library, an) {
                Some(c) => ctx.symbol_name(ctx.program.classes[c.0 as usize].name),
                None => ctx.interner.resolve(an.name.first()?.sym),
            };
            if name_str == "Array" {
                let dims = array_dimensions(&u.source, ast, an)?;
                let elem = self.array_element(declared)?;
                return Some(FieldType::Array { elem: Box::new(elem), dims });
            }
            if let Some(tc) = primitive(name_str) {
                return Some(FieldType::Prim(tc));
            }
            if let Some((_, tc)) = self.types.classes.iter().find(|(c, _)| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == name_str) {
                return Some(FieldType::Prim(*tc));
            }
        }
        match ctx.table.get(declared) {
            T::Interface { class, .. } if self.types.classes.get(class) == Some(&CType::Ptr) => Some(FieldType::Prim(CType::Ptr)),
            T::Interface { class, .. } if self.is_composite(*class) => Some(FieldType::Composite(*class)),
            _ => None,
        }
    }

    /// A classe nomeada por uma anotação `@Nome()`, `@p.Nome()` ou
    /// `@Alias()` (alias de uma classe), pelo escopo da biblioteca.
    fn annotation_class(&self, lib: dartforge_elements::model::LibraryId, an: &ast::Annotation) -> Option<ClassId> {
        use dartforge_elements::model::Element;
        let ctx = self.ctx;
        let cls = |el: Option<Element>| match el? {
            Element::Class(c) => Some(c),
            Element::Typedef(td) => match ctx.table.get(ctx.outline.typedefs.get(td.0 as usize)?.target_type) {
                T::Interface { class, .. } => Some(*class),
                _ => None,
            },
            _ => None,
        };
        // `@Nome()`/`@Nome.ctor()`, senão `@p.Nome()`/`@p.Nome.ctor()`.
        let first_one = an.name.first()?;
        cls(ctx.program.lookup(lib, first_one.sym).and_then(|b| b.getter)).or_else(|| {
            let n = an.name.get(1)?;
            cls(ctx.program.lookup_prefixed(lib, first_one.sym, n.sym).and_then(|b| b.getter))
        })
    }

    /// O tipo mais interno de `Array<Array<…<E>>>`.
    fn array_element(&mut self, t: TypeId) -> Option<FieldType> {
        let T::Interface { class, args, .. } = self.ctx.table.get(t) else { return None };
        let name_str = self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name);
        if name_str == "Array" {
            return self.array_element(*args.first()?);
        }
        if let Some(tc) = self.types.classes.get(class) {
            return Some(FieldType::Prim(*tc));
        }
        if self.is_composite(*class) {
            return Some(FieldType::Composite(*class));
        }
        None
    }
}

/// O elemento mais interno de `Array<Array<…<E>>>` com `n` níveis (as
/// dimensões da anotação): um tipo nativo primitivo, ponteiro ou composto.
fn native_array_element(ctx: &Context, types: &NativeTypes, t: TypeId, n: usize) -> Option<FieldType> {
    let T::Interface { class, args, .. } = ctx.table.get(t) else { return None };
    if n > 0 {
        if Some(*class) != types.array {
            return None;
        }
        return native_array_element(ctx, types, *args.first()?, n - 1);
    }
    if let Some(tc) = types.classes.get(class) {
        return (*tc != CType::Void).then_some(FieldType::Prim(*tc));
    }
    types.composites.contains(class).then_some(FieldType::Composite(*class))
}

/// As dimensões de `@Array(2, 3)` ou `@Array.multi([2, 3])`.
fn array_dimensions(source: &str, ast: &ast::Ast, an: &ast::Annotation) -> Option<Vec<usize>> {
    let args = &an.arguments.as_ref()?.args;
    let integer = |e: ast::ExprId| integer_literal(source, ast, e).and_then(|n| usize::try_from(n).ok());
    if an.name.len() > 1 {
        // `Array.multi([…])`.
        let ast::ExprKind::List { elements, .. } = &ast.expr(args.first()?.value).kind else { return None };
        return elements
            .iter()
            .map(|el| match el {
                ast::CollectionElement::Expression(e) => integer(*e),
                _ => None,
            })
            .collect();
    }
    args.iter().map(|a| integer(a.value)).collect()
}

/// O argumento inteiro de uma anotação de classe (`@Packed(1)`).
fn full_annotation(ctx: &Context, c: ClassId, name_str: &str) -> Option<i64> {
    let decl = ctx.program.classes[c.0 as usize].decl?;
    let u = ctx.program.unit(decl.unit);
    for an in u.ast.decls[decl.decl.0 as usize].metadata.iter() {
        if an.name.last().map(|n| ctx.interner.resolve(n.sym)) != Some(name_str) {
            continue;
        }
        let arg = an.arguments.as_ref()?.args.first()?;
        return integer_literal(&u.source, &u.ast, arg.value);
    }
    None
}

/// O valor de um literal inteiro (decimal ou hexadecimal).
fn integer_literal(source: &str, ast: &ast::Ast, e: ast::ExprId) -> Option<i64> {
    let ast::ExprKind::Int(span) = &ast.expr(e).kind else { return None };
    let text = source.get(span.start as usize..span.end as usize)?.replace('_', "");
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(h) => i64::from_str_radix(h, 16).ok(),
        None => text.parse().ok(),
    }
}

impl CType {
    /// O sufixo dos natives de carga/gravação (`DartForge_ffi_carregar_i32`).
    fn memory_suffix(self) -> &'static str {
        match self {
            CType::I8 => "i8",
            CType::U8 | CType::Bool => "u8",
            CType::I16 => "i16",
            CType::U16 => "u16",
            CType::I32 => "i32",
            CType::U32 => "u32",
            CType::I64 | CType::Ptr | CType::Handle => "i64",
            CType::U64 => "u64",
            CType::F32 => "f32",
            CType::F64 => "f64",
            CType::Void => "i64",
        }
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O campo `external` de uma struct/union: `(base, offset, ty)`
    /// do acesso — `_typedDataBase` do objeto e `_offsetInBytes` + o
    /// deslocamento do campo no layout.
    fn ffi_field_address(&mut self, obj: Operand, vid: dartforge_elements::model::VariableId) -> Option<(Operand, Operand, FieldType)> {
        let v = &self.ctx.program.variables[vid.0 as usize];
        if !v.external || v.static_ {
            return None;
        }
        let comp = composites(self.ctx)?;
        let (off, typ) = comp.classes.get(&v.class?)?.fields.get(&vid)?.clone();
        let (vbase, vdesl) = (comp.base, comp.offset);
        let span = dartforge_diagnostics::Span { start: 0, end: 0 };
        let base = self.read_field(obj.clone(), vbase, span);
        let d0 = self.read_field(obj, vdesl, span);
        let d0 = self.coerce(d0, Type::I64);
        let d = self.emit(Instruction::Add(d0, Operand::Constant(Constant::Int(off as i64))), Type::I64);
        Some((base, d, typ))
    }

    /// Leitura de um campo `external` de struct/union (`None`: não é um).
    pub fn read_ffi_field(&mut self, obj: Operand, vid: dartforge_elements::model::VariableId) -> Option<Operand> {
        let (base, d, typ) = self.ffi_field_address(obj, vid)?;
        Some(self.read_ffi_memory(base, d, typ))
    }

    /// O valor Dart do tipo `ty` em `base` + `d` (base `Pointer` ou
    /// `TypedData`): um campo de struct ou uma variável `@Native`.
    fn read_ffi_memory(&mut self, base: Operand, d: Operand, typ: FieldType) -> Operand {
        match typ {
            FieldType::Prim(tc) => {
                let ret = if matches!(tc, CType::F32 | CType::F64) { Type::F64 } else { Type::I64 };
                // A carga confere a base (`RangeError`/`ArgumentError` pendente).
                let v = self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: format!("dartforge_nativo_DartForge_ffi_carregar_{}", tc.memory_suffix()),
                        args: vec![(base, Type::Ref), (d, Type::I64)],
                        ret_ty: ret,
                    },
                    ret,
                );
                match tc {
                    CType::Bool => self.emit(Instruction::ICmp(ICmpOp::Ne, v, Operand::Constant(Constant::Int(0))), Type::I1),
                    CType::Ptr => self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_ffi_ponteiro_novo".to_string(),
                            args: vec![(v, Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    _ => v,
                }
            }
            FieldType::Composite(c) => self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_ffi_composto".to_string(),
                    args: vec![(Operand::Constant(Constant::Int(self.ctx.id_rti(c))), Type::I64), (base, Type::Ref), (d, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            FieldType::Array { dims, elem } => {
                // `Array._(base, offset, first dimensão, as demais)`.
                let mut table = Vec::new();
                for &n in &dims[1..] {
                    table.push(b'i');
                    table.extend_from_slice(&(n as i64).to_le_bytes());
                }
                let words: Vec<i64> = table
                    .chunks(8)
                    .map(|c| {
                        let mut w = [0u8; 8];
                        w[..c.len()].copy_from_slice(c);
                        i64::from_le_bytes(w)
                    })
                    .collect();
                let data = self.emit(Instruction::ConstArray(words), Type::Ptr);
                let rest = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_lista_de_tabela".to_string(),
                        args: vec![(data, Type::Ptr), (Operand::Constant(Constant::Int(table.len() as i64)), Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                );
                let Some(f) = self.top_level_function("dart:ffi", "_dartforgeArray") else {
                    return self.unsupported("dart:ffi sem `_dartforgeArray`", dartforge_diagnostics::Span { start: 0, end: 0 });
                };
                let bytes = size_of(self.ctx, &elem) as i64;
                let args = vec![
                    (None, base),
                    (None, d),
                    (None, Operand::Constant(Constant::Int(dims[0] as i64))),
                    (None, rest),
                    (None, Operand::Constant(Constant::Int(bytes))),
                ];
                let args = self.match_args(f, &args);
                self.call_direct(f, None, args)
            }
        }
    }

    /// Gravação de um campo `external` de struct/union (`false`: não é um).
    pub fn store_ffi_field(&mut self, obj: Operand, vid: dartforge_elements::model::VariableId, val: Operand) -> bool {
        let Some((base, d, typ)) = self.ffi_field_address(obj, vid) else { return false };
        self.store_ffi_memory(base, d, typ, val);
        true
    }

    /// Grava o valor Dart `val` do tipo `ty` em `base` + `d`.
    fn store_ffi_memory(&mut self, base: Operand, d: Operand, typ: FieldType, val: Operand) {
        match typ {
            FieldType::Prim(tc) => {
                let (v, ty) = match tc {
                    CType::F32 | CType::F64 => (self.coerce(val, Type::F64), Type::F64),
                    CType::Bool => {
                        let b = self.coerce(val, Type::I1);
                        (self.emit(Instruction::ZExt { op: b, from: Type::I1, to: Type::I64 }, Type::I64), Type::I64)
                    }
                    CType::Ptr => {
                        let p = self.coerce(val, Type::Ref);
                        (
                            self.emit_call_with_check(
                                Instruction::CallRuntime {
                                    name: "dartforge_ffi_endereco_do_ponteiro".to_string(),
                                    args: vec![(p, Type::Ref)],
                                    ret_ty: Type::I64,
                                },
                                Type::I64,
                            ),
                            Type::I64,
                        )
                    }
                    _ => (self.coerce(val, Type::I64), Type::I64),
                };
                // A gravação confere a base, como a carga.
                self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: format!("dartforge_nativo_DartForge_ffi_gravar_{}", tc.memory_suffix()),
                        args: vec![(base, Type::Ref), (d, Type::I64), (v, ty)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
            }
            FieldType::Composite(_) | FieldType::Array { .. } => {
                // Por valor: copia os bytes do composto atribuído.
                let size = size_of(self.ctx, &typ);
                let Some(comp) = composites(self.ctx) else { return };
                let (vbase, vdesl) = (comp.base, comp.offset);
                let span = dartforge_diagnostics::Span { start: 0, end: 0 };
                let val = self.coerce(val, Type::Ref);
                let ob = self.read_field(val.clone(), vbase, span);
                let od = self.read_field(val, vdesl, span);
                let od = self.coerce(od, Type::I64);
                self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_nativo_DartForge_ffi_copiar".to_string(),
                        args: vec![
                            (base, Type::Ref),
                            (d, Type::I64),
                            (ob, Type::Ref),
                            (od, Type::I64),
                            (Operand::Constant(Constant::Int(size as i64)), Type::I64),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
            }
        }
    }

    /// O endereço (i64) do símbolo nativo `symbol` no processo.
    fn symbol_address(&mut self, symbol: &[u8]) -> Operand {
        let words: Vec<i64> = symbol
            .chunks(8)
            .map(|c| {
                let mut w = [0u8; 8];
                w[..c.len()].copy_from_slice(c);
                i64::from_le_bytes(w)
            })
            .collect();
        let name_str = self.emit(Instruction::ConstArray(words), Type::Ptr);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_ffi_simbolo_nativo".to_string(),
                args: vec![(name_str, Type::Ptr), (Operand::Constant(Constant::Int(symbol.len() as i64)), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// Uma variável `external` com `@Native<T>()`: o símbolo e o tipo do
    /// dado (`None`: não é uma).
    fn native_variable(&self, vid: dartforge_elements::model::VariableId) -> Option<Result<(Vec<u8>, FieldType), String>> {
        use dartforge_elements::model::VariableRef;
        let v = &self.ctx.program.variables[vid.0 as usize];
        if !v.external {
            return None;
        }
        let VariableRef::TopLevel { unit, decl, .. } = v.node else { return None };
        let u = self.ctx.program.unit(unit);
        let an = u.ast.decls[decl.0 as usize]
            .metadata
            .iter()
            .find(|an| an.name.last().map(|n| self.ctx.interner.resolve(n.sym)) == Some("Native"))?;
        let Some(types) = NativeTypes::of_program(self.ctx) else { return Some(Err("@Native sem dart:ffi".to_string())) };
        let symbol = annotation_symbol(self.ctx, &u.ast, an).unwrap_or_else(|| self.ctx.symbol_name(v.name).as_bytes().to_vec());
        // `@Array(d0, d1…)` na variável: o dado é o vetor C, com os
        // elementos do tipo declarado (`Array<Array<…<E>>>`), como um campo
        // `@Array` de struct.
        if let Some(arr) = u.ast.decls[decl.0 as usize]
            .metadata
            .iter()
            .find(|an| an.name.first().map(|n| self.ctx.interner.resolve(n.sym)) == Some("Array"))
        {
            let Some(dims) = array_dimensions(&u.source, &u.ast, arr).filter(|d| !d.is_empty()) else {
                return Some(Err("@Array de variável @Native sem dimensões constantes".to_string()));
            };
            let declared = super::members::variable_type(self.ctx, vid);
            let Some(elem) = native_array_element(self.ctx, &types, declared, dims.len()) else {
                return Some(Err("@Native de `Array` com elemento que não é tipo nativo".to_string()));
            };
            return Some(Ok((symbol, FieldType::Array { elem: Box::new(elem), dims })));
        }
        let Some(&t) = an.type_args.first() else {
            return Some(Err("@Native de variável sem o argumento de tipo".to_string()));
        };
        let typ = match types.annotation_native_type(self.ctx, &u.ast, t, v.library) {
            Ok((NativeType::Prim(CType::Void), _)) => return Some(Err("@Native de variável `Void`".to_string())),
            Ok((NativeType::Prim(tc), _)) => FieldType::Prim(tc),
            Ok((NativeType::Composite(_), Some(c))) => FieldType::Composite(c),
            Ok(_) => return Some(Err("tipo de variável @Native".to_string())),
            Err(m) => return Some(Err(m)),
        };
        Some(Ok((symbol, typ)))
    }

    /// A base (`Pointer` para o símbolo) de uma variável `@Native`.
    fn native_variable_base(&mut self, symbol: &[u8]) -> Operand {
        let e = self.symbol_address(symbol);
        self.emit_call_with_check(
            Instruction::CallRuntime { name: "dartforge_ffi_ponteiro_novo".to_string(), args: vec![(e, Type::I64)], ret_ty: Type::Ref },
            Type::Ref,
        )
    }

    /// Leitura de uma variável `@Native` (`None`: não é uma).
    pub fn read_native_variable(&mut self, vid: dartforge_elements::model::VariableId, span: dartforge_diagnostics::Span) -> Option<Operand> {
        let (symbol, typ) = match self.native_variable(vid)? {
            Ok(x) => x,
            Err(m) => return Some(self.unsupported(&m, span)),
        };
        let base = self.native_variable_base(&symbol);
        Some(self.read_ffi_memory(base, Operand::Constant(Constant::Int(0)), typ))
    }

    /// Gravação de uma variável `@Native` (`false`: não é uma).
    pub fn store_native_variable(&mut self, vid: dartforge_elements::model::VariableId, val: Operand, span: dartforge_diagnostics::Span) -> bool {
        let Some(r) = self.native_variable(vid) else { return false };
        let (symbol, typ) = match r {
            Ok(x) => x,
            Err(m) => {
                self.unsupported(&m, span);
                return true;
            }
        };
        let base = self.native_variable_base(&symbol);
        self.store_ffi_memory(base, Operand::Constant(Constant::Int(0)), typ, val);
        true
    }

    /// `Native.addressOf<T>(x)` (o que o front-end da VM reescreve): o
    /// `Pointer` para o símbolo da função ou variável `@Native` citada.
    pub fn native_address(&mut self, ast: &ast::Ast, arg: ast::ExprId, span: dartforge_diagnostics::Span) -> Operand {
        use dartforge_elements::model::Element;
        let resolved = self.ctx.get_resolved(self.unit_id, arg).cloned();
        let el = match resolved {
            Some(dartforge_types::resolved::Resolved::Element(e)) => Some(e),
            _ => match &ast.expr(arg).kind {
                ast::ExprKind::Identifier(n) => match self.resolve_by_name(n.sym) {
                    Some(dartforge_types::resolved::Resolved::Element(e)) => Some(e),
                    _ => None,
                },
                _ => None,
            },
        };
        let symbol = match el {
            Some(Element::Function(f)) => {
                let fid = f.0 as usize;
                match self.ctx.program.functions[fid].variable {
                    Some(vid) => self.native_variable(vid).and_then(Result::ok).map(|(s, _)| s),
                    None => function_metadata(self.ctx, fid).and_then(|(_, a, meta)| {
                        let an = meta.iter().find(|an| an.name.last().map(|n| self.ctx.interner.resolve(n.sym)) == Some("Native"))?;
                        Some(annotation_symbol(self.ctx, a, an).unwrap_or_else(|| self.ctx.symbol_name(self.ctx.program.functions[fid].name).as_bytes().to_vec()))
                    }),
                }
            }
            Some(Element::Variable(vid)) => self.native_variable(vid).and_then(Result::ok).map(|(s, _)| s),
            _ => None,
        };
        let Some(symbol) = symbol else {
            return self.language_error("Argument to 'Native.addressOf' must be annotated with @Native.", span);
        };
        self.native_variable_base(&symbol)
    }
}

/// O `symbol:` de uma anotação `@Native`.
fn annotation_symbol(ctx: &Context, ast: &ast::Ast, an: &ast::Annotation) -> Option<Vec<u8>> {
    an.arguments
        .as_ref()?
        .args
        .iter()
        .find(|x| x.name.map(|n| ctx.interner.resolve(n.sym)) == Some("symbol"))
        .and_then(|x| match &ast.expr(x.value).kind {
            ast::ExprKind::String(s) => s.constant_value().map(|t| t.as_bytes().to_vec()),
            _ => None,
        })
}

/// O tamanho em bytes de um tipo de campo (compostos já calculados).
fn size_of(ctx: &Context, t: &FieldType) -> usize {
    match t {
        FieldType::Prim(tc) => tc.c_size(),
        FieldType::Composite(c) => composites(ctx).and_then(|x| x.classes.get(c)).map_or(0, |l| l.size),
        FieldType::Array { elem, dims } => size_of(ctx, elem) * dims.iter().product::<usize>(),
    }
}

// ─── `@Native` ─────────────────────────────────────────────────────────────
//
// `@Native<NF>(symbol: 's', isLeaf: …) external R f(…)`: o corpo é a chamada
// à função nativa `s` (o nome da função, sem `symbol:`), com a assinatura
// `NF`. A VM resolve o símbolo no *asset* da biblioteca e, sem ele, no
// processo e no executável; aqui a resolução é no processo
// (`dartforge_ffi_simbolo_nativo`, com cache), onde estão a libc e o que o
// programa carregou (`DynamicLibrary.open` com `RTLD_GLOBAL` não é
// necessário: o processo inclui as bibliotecas ligadas ao executável).

/// As anotações da declaração de uma função.
fn function_metadata<'p>(ctx: &'p Context, fid: usize) -> Option<(&'p str, &'p ast::Ast, &'p [ast::Annotation])> {
    use dartforge_elements::model::FunctionRef;
    let f = &ctx.program.functions[fid];
    let FunctionRef::Function { unit, function } = f.node else { return None };
    let u = ctx.program.unit(unit);
    let a = &u.ast;
    let meta = a
        .members
        .iter()
        .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if id == function))
        .map(|m| &*m.metadata)
        .or_else(|| a.decls.iter().find(|d| matches!(d.kind, ast::DeclKind::Function(id) if id == function)).map(|d| &*d.metadata))?;
    Some((&u.source, a, meta))
}

/// Marca de [`NativeTypes::annotation_c_type`]: o nome é de uma
/// struct/union (resolvida por [`NativeTypes::annotation_native_type`]).
const COMPOSITE_IN_ANNOTATION: &str = "struct por valor";

impl NativeTypes {
    /// O tipo nativo escrito na anotação, com struct/union por valor (a
    /// classe do mesmo nome, de preferência da biblioteca `lib`).
    fn annotation_native_type(
        &self,
        ctx: &Context,
        ast: &ast::Ast,
        t: ast::TypeId,
        lib: dartforge_elements::model::LibraryId,
    ) -> Result<(NativeType, Option<ClassId>), String> {
        match self.annotation_c_type(ctx, ast, t) {
            Ok(tc) => Ok((NativeType::Prim(tc), None)),
            Err(m) if m == COMPOSITE_IN_ANNOTATION => {
                let ast::TypeKind::Named { name, .. } = &ast.ty(t).kind else { return Err(m) };
                let name_str = ctx.interner.resolve(name.last().ok_or("tipo sem nome")?.sym);
                let mut candidates: Vec<ClassId> = self
                    .composites
                    .iter()
                    .copied()
                    .filter(|c| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == name_str)
                    .collect();
                candidates.sort_by_key(|c| (ctx.program.classes[c.0 as usize].library != lib, c.0));
                let c = *candidates.first().ok_or(m)?;
                let layout = layout_c(ctx, c).ok_or_else(|| format!("struct `{name_str}` sem layout"))?;
                Ok((NativeType::Composite(layout), Some(c)))
            }
            Err(m) => Err(m),
        }
    }

    /// O tipo C de um tipo nativo escrito na anotação (`Pointer<Utf8>`,
    /// `Size`, `Int32`), pelo nome da classe.
    fn annotation_c_type(&self, ctx: &Context, ast: &ast::Ast, t: ast::TypeId) -> Result<CType, String> {
        match &ast.ty(t).kind {
            ast::TypeKind::Void => Ok(CType::Void),
            ast::TypeKind::Named { name, .. } => {
                let name_str = ctx.interner.resolve(name.last().ok_or("tipo sem nome")?.sym);
                if let Some(tc) = primitive(name_str) {
                    return Ok(tc);
                }
                if let Some((_, tc)) = self.classes.iter().find(|(c, _)| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == name_str) {
                    return Ok(*tc);
                }
                if self.composites.iter().any(|c| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == name_str) {
                    return Err(COMPOSITE_IN_ANNOTATION.to_string());
                }
                if let Some((_, m)) = self.rejected.iter().find(|(c, _)| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == name_str) {
                    return Err(format!("{m} ainda não é suportado no backend nativo"));
                }
                Err(format!("`{name_str}` não é um tipo nativo suportado em @Native"))
            }
            _ => Err("tipo nativo inesperado em @Native".to_string()),
        }
    }
}

/// A assinatura nativa do tipo de função `t` escrito numa anotação
/// (seguindo `typedef`s, inclusive de outra unidade).
fn annotation_signature(
    ctx: &Context,
    types: &NativeTypes,
    ast: &ast::Ast,
    t: ast::TypeId,
    lib: dartforge_elements::model::LibraryId,
    depth: usize,
) -> Result<NativeSignature, String> {
    let from_params = |ast: &ast::Ast, lib, ret: Option<ast::TypeId>, params: &[ast::Parameter]| -> Result<NativeSignature, String> {
        let (r, ret_class) = match ret {
            Some(r) => types.annotation_native_type(ctx, ast, r, lib)?,
            None => (NativeType::Prim(CType::Void), None),
        };
        let mut ps = Vec::with_capacity(params.len());
        let mut cs = Vec::with_capacity(params.len());
        for p in params {
            let t = p.ty.ok_or_else(|| "parâmetro nativo sem tipo".to_string())?;
            let (tn, c) = types.annotation_native_type(ctx, ast, t, lib)?;
            ps.push(tn);
            cs.push(c);
        }
        Ok(NativeSignature { ret: r, ret_class, params: ps, class_params: cs, variadic_flag: None })
    };
    match &ast.ty(t).kind {
        ast::TypeKind::Function { return_type, parameters, .. } => from_params(ast, lib, *return_type, parameters),
        ast::TypeKind::Named { name, .. } if depth < 8 => {
            let name_str = ctx.interner.resolve(name.last().ok_or("tipo sem nome")?.sym);
            // O `typedef` com esse nome, de preferência da mesma biblioteca.
            let mut candidates: Vec<&dartforge_elements::model::TypedefElement> =
                ctx.program.typedefs.iter().filter(|td| ctx.symbol_name(td.name) == name_str).collect();
            candidates.sort_by_key(|td| td.library != lib);
            let Some(td) = candidates.first() else {
                return Err("@Native de variável (ponteiro para dado nativo) ainda não é suportado".to_string());
            };
            let u = ctx.program.unit(td.decl.unit);
            let ast::DeclKind::Typedef(decl) = &u.ast.decls[td.decl.decl.0 as usize].kind else {
                return Err(format!("`{name_str}` não é um typedef"));
            };
            match &decl.kind {
                ast::TypedefKind::Alias(alvo) => annotation_signature(ctx, types, &u.ast, *alvo, td.library, depth + 1),
                ast::TypedefKind::Legacy { return_type, parameters } => from_params(&u.ast, td.library, *return_type, parameters),
            }
        }
        _ => Err("assinatura nativa inesperada em @Native".to_string()),
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O corpo de um `external` com `@Native` (`None`: não tem a anotação).
    pub fn call_annotated_native(&mut self, fid: usize, args: &[Operand], span: dartforge_diagnostics::Span) -> Option<Operand> {
        let (source, ast, meta) = function_metadata(self.ctx, fid)?;
        let an = meta.iter().find(|an| an.name.last().map(|n| self.ctx.interner.resolve(n.sym)) == Some("Native"))?;
        let Some(types) = NativeTypes::of_program(self.ctx) else {
            return Some(self.unsupported("@Native sem dart:ffi", span));
        };
        // A assinatura: `NF` da anotação.
        let Some(&nf) = an.type_args.first() else {
            return Some(self.unsupported("@Native sem o argumento de tipo da assinatura nativa", span));
        };
        let lib = self.ctx.program.functions[fid].library;
        let signature = annotation_signature(self.ctx, &types, ast, nf, lib, 0);
        let signature = match signature {
            Ok(x) => x,
            Err(m) => return Some(self.unsupported(&m, span)),
        };
        // O símbolo: `symbol: 'x'`, ou o nome da função.
        let symbol = annotation_symbol(self.ctx, ast, an)
            .unwrap_or_else(|| self.ctx.symbol_name(self.ctx.program.functions[fid].name).as_bytes().to_vec());
        let _ = source;
        let alvo = self.symbol_address(&symbol);
        if args.len() != signature.params.len() {
            return Some(self.unsupported("@Native com aridade diferente da assinatura nativa", span));
        }
        let dart_ret = self.return_repr(fid);
        let r = self.signature_native_call(alvo, args, &signature, None);
        Some(if dart_ret == Type::Void { Operand::Constant(Constant::Null) } else { self.coerce(r, dart_ret) })
    }

    /// A chamada nativa de `alvo` com os valores Dart `args` pela
    /// assinatura: ponteiros e structs pelos endereços, o retorno de volta
    /// como valor Dart (`Pointer`, struct nova sobre `Uint8List`, primitivo)
    /// — o que o trampolim de `asFunction` e o corpo de um `@Native` fazem.
    /// `closure`: a do trampolim, para o tipo do `Pointer` devolvido.
    fn signature_native_call(
        &mut self,
        alvo: Operand,
        args: &[Operand],
        a: &NativeSignature,
        closure: Option<Operand>,
    ) -> Operand {
        let address = |b: &mut Self, name: &str, v: Operand| {
            let v = b.coerce(v, Type::Ref);
            b.emit_call_with_check(
                Instruction::CallRuntime { name: name.to_string(), args: vec![(v, Type::Ref)], ret_ty: Type::I64 },
                Type::I64,
            )
        };
        // `Handle`: as células dos objetos passados vivem num escopo aberto
        // só em volta da chamada (os handles locais da VM). As conversões
        // que podem lançar vêm antes de abri-lo, e o retorno é convertido
        // depois de fechá-lo: nenhuma saída excepcional deixa células para
        // trás. Criar uma célula não lança, logo a ordem observável das
        // conversões dos outros argumentos se mantém.
        let with_handles = std::iter::once(&a.ret).chain(a.params.iter()).any(|t| matches!(t, NativeType::Prim(CType::Handle)));
        let mut natives = Vec::with_capacity(a.params.len());
        for (v, t) in args.iter().cloned().zip(a.params.iter()) {
            let x = match t {
                NativeType::Prim(CType::Handle) => self.coerce(v, Type::Ref),
                NativeType::Prim(CType::Ptr) => address(self, "dartforge_ffi_endereco_do_ponteiro", v),
                NativeType::Composite(_) => address(self, "dartforge_ffi_endereco_do_composto", v),
                NativeType::Prim(tc) => self.coerce(v, tc.hir_type()),
            };
            natives.push((x, t.clone()));
        }
        let returned_struct = match (&a.ret, a.ret_class) {
            (NativeType::Composite(_), Some(c)) => {
                let new = self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_composto_novo".to_string(),
                        args: vec![(Operand::Constant(Constant::Int(self.ctx.id_rti(c))), Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                );
                let dest = address(self, "dartforge_ffi_endereco_do_composto", new.clone());
                Some((new, dest))
            }
            (NativeType::Composite(_), None) => unreachable!("struct devolvida sem classe"),
            _ => None,
        };
        let scope = with_handles.then(|| {
            self.emit(
                Instruction::CallRuntime { name: "dartforge_ffi_handles_abrir".to_string(), args: Vec::new(), ret_ty: Type::I64 },
                Type::I64,
            )
        });
        for (x, t) in &mut natives {
            if matches!(t, NativeType::Prim(CType::Handle)) {
                let obj = std::mem::replace(x, Operand::Constant(Constant::Null));
                *x = self.emit(
                    Instruction::CallRuntime { name: "dartforge_ffi_handle_novo".to_string(), args: vec![(obj, Type::Ref)], ret_ty: Type::I64 },
                    Type::I64,
                );
            }
        }
        let close = |b: &mut Self| {
            if let Some(e) = scope.clone() {
                b.emit(
                    Instruction::CallRuntime { name: "dartforge_ffi_handles_fechar".to_string(), args: vec![(e, Type::I64)], ret_ty: Type::Void },
                    Type::Void,
                );
            }
        };
        if let Some((new, dest)) = returned_struct {
            self.emit(
                Instruction::CompositeNativeCall { alvo, args: natives, ret: a.ret.clone(), dest: Some(dest), variadic_flag: a.variadic_flag },
                Type::Void,
            );
            close(self);
            return new;
        }
        let NativeType::Prim(ret) = a.ret else { unreachable!("retorno composto tratado acima") };
        let r = if a.variadic_flag.is_none() && natives.iter().all(|(_, t)| matches!(t, NativeType::Prim(_))) {
            let prims = natives
                .into_iter()
                .map(|(x, t)| match t {
                    NativeType::Prim(tc) => (x, tc),
                    NativeType::Composite(_) => unreachable!(),
                })
                .collect();
            self.emit(Instruction::NativeCall { alvo, args: prims, ret }, ret.hir_type())
        } else {
            self.emit(
                Instruction::CompositeNativeCall { alvo, args: natives, ret: a.ret.clone(), dest: None, variadic_flag: a.variadic_flag },
                ret.hir_type(),
            )
        };
        // O objeto de um `Handle` devolvido é lido antes de o escopo soltar
        // as células, na mesma chamada que o fecha.
        if ret == CType::Handle {
            let e = scope.expect("retorno Handle abre o escopo");
            return self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_ffi_handles_fechar_com_objeto".to_string(),
                    args: vec![(e, Type::I64), (r, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        close(self);
        let trampoline = closure.is_some();
        match (ret, closure) {
            (CType::Void, _) => Operand::Constant(Constant::Null),
            (CType::Ptr, Some(clo)) => self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_ffi_ponteiro_de_retorno".to_string(),
                    args: vec![(r, Type::I64), (clo, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            (CType::Ptr, None) => self.emit_call_with_check(
                Instruction::CallRuntime { name: "dartforge_ffi_ponteiro_novo".to_string(), args: vec![(r, Type::I64)], ret_ty: Type::Ref },
                Type::Ref,
            ),
            // O trampolim devolve `Ref`; um `@Native`, o valor cru
            // (quem chama o coage ao retorno Dart, sem caixa).
            _ if trampoline => self.coerce(r, Type::Ref),
            _ => r,
        }
    }
}
