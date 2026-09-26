//! `dart:ffi` (NATIVO-PLANO, FFI): o que o transformador de FFI do
//! front-end da VM e o compilador dela fazem, aqui no lowering do programa.
//!
//! * **Tipos nativos.** Cada classe de tipo nativo que uma assinatura pode
//!   usar tem um [`TipoC`]: os primitivos (`Int8`…`Double`, `Bool`), o
//!   `Pointer` e os inteiros específicos da ABI (`Long`, `Size`, `WChar`…, do
//!   SDK e do programa), resolvidos para o alvo pela anotação
//!   `@AbiSpecificIntegerMapping`. A tabela vai ao runtime
//!   (`dartforge_ffi_registrar_tipo`), que a usa em `sizeOf` e para achar o
//!   trampolim de uma assinatura pela RTI.
//! * **Trampolins.** Toda assinatura nativa `NativeFunction<F>` que aparece
//!   no programa (a tabela de tipos inteira, depois da inferência) ganha um
//!   trampolim: uma entrada da convenção uniforme das closures que confere a
//!   aridade, converte os argumentos Dart para os tipos C, chama a função
//!   nativa pelo endereço guardado na closure ([`Instruction::ChamadaNativa`],
//!   com a extensão dos inteiros estreitos que a ABI exige) e converte o
//!   retorno. `asFunction`/`lookupFunction` (a sobreposição de `dart:ffi`)
//!   pedem ao runtime a closure com o trampolim da assinatura (a chave é a
//!   mesma dos dois lados: [`chave_da_assinatura`]).
//!
//! Structs e unions por valor, funções variádicas e `Handle` ainda não
//! baixam: a assinatura é recusada (a chamada vira erro do runtime com o
//! motivo), nunca uma chamada com a ABI errada.

use super::closures::{Padrao, ParamEntrada};
use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::ClassId;
use dartforge_frontend::ast::{self, ParameterKind};
use dartforge_types::table::Type as T;
use dartforge_types::TypeId;

/// O nome da ABI do alvo no enum `Abi` do `dart:ffi` (a chave das
/// anotações `@AbiSpecificIntegerMapping`).
fn abi_do_alvo() -> &'static str {
    let arm = cfg!(target_arch = "aarch64");
    match crate::alvo::sistema() {
        crate::alvo::Sistema::Windows => if arm { "windowsArm64" } else { "windowsX64" },
        crate::alvo::Sistema::Linux => if arm { "linuxArm64" } else { "linuxX64" },
        crate::alvo::Sistema::MacOs => if arm { "macosArm64" } else { "macosX64" },
    }
}

/// O tipo C de um primitivo de `dart:ffi` pelo nome da classe.
fn primitivo(nome: &str) -> Option<TipoC> {
    Some(match nome {
        "Int8" => TipoC::I8,
        "Uint8" => TipoC::U8,
        "Int16" => TipoC::I16,
        "Uint16" => TipoC::U16,
        "Int32" => TipoC::I32,
        "Uint32" => TipoC::U32,
        "Int64" => TipoC::I64,
        "Uint64" => TipoC::U64,
        "Float" => TipoC::F32,
        "Double" => TipoC::F64,
        "Bool" => TipoC::Bool,
        "Void" => TipoC::Void,
        "Pointer" => TipoC::Ptr,
        _ => return None,
    })
}

/// As classes de tipo nativo do programa com o tipo C de cada uma.
pub struct TiposNativos {
    classes: std::collections::HashMap<ClassId, TipoC>,
    /// Classes que existem em `dart:ffi` mas ainda não baixam por valor
    /// (`Struct`, `Union`, `Handle`, `VarArgs`), com o motivo.
    recusadas: std::collections::HashMap<ClassId, &'static str>,
}

impl TiposNativos {
    pub fn do_programa(ctx: &Context) -> Option<TiposNativos> {
        let ffi = ctx.program.libraries.iter().position(|l| l.uri == "dart:ffi")?;
        let ffi = dartforge_elements::model::LibraryId(ffi as u32);
        let mut classes = std::collections::HashMap::new();
        let mut recusadas = std::collections::HashMap::new();
        let abi_especifico = ctx.classe_do_sdk("ffi", "AbiSpecificInteger");
        let struct_ = ctx.classe_do_sdk("ffi", "Struct");
        let union_ = ctx.classe_do_sdk("ffi", "Union");
        for (i, c) in ctx.program.classes.iter().enumerate() {
            let id = ClassId(i as u32);
            let nome = ctx.symbol_name(c.name);
            if c.library == ffi {
                if let Some(t) = primitivo(nome) {
                    classes.insert(id, t);
                    continue;
                }
                match nome {
                    "Handle" => {
                        recusadas.insert(id, "Handle (objeto Dart na fronteira nativa)");
                    }
                    "VarArgs" => {
                        recusadas.insert(id, "função variádica (VarArgs)");
                    }
                    _ => {}
                }
            }
            if c.supertype_class.is_some() && c.supertype_class == abi_especifico {
                match mapeamento_da_abi(ctx, id) {
                    Some(t) => {
                        classes.insert(id, t);
                    }
                    None => {
                        recusadas.insert(id, "inteiro específico da ABI sem mapeamento para este alvo");
                    }
                }
            } else if c.supertype_class.is_some() && (c.supertype_class == struct_ || c.supertype_class == union_) {
                recusadas.insert(id, "struct ou union por valor");
            }
        }
        Some(TiposNativos { classes, recusadas })
    }

    /// O tipo C de um tipo nativo (argumento ou retorno de uma assinatura).
    fn tipo_c(&self, ctx: &Context, t: TypeId) -> Result<TipoC, String> {
        match ctx.table.get(t) {
            T::Interface { class, .. } => {
                if let Some(tc) = self.classes.get(class) {
                    return Ok(*tc);
                }
                if let Some(m) = self.recusadas.get(class) {
                    return Err(format!("{m} ainda não é suportado no backend nativo"));
                }
                Err(format!("`{}` não é um tipo nativo", ctx.symbol_name(ctx.program.classes[class.0 as usize].name)))
            }
            _ => Err("tipo nativo não é uma classe de `dart:ffi`".to_string()),
        }
    }

    /// A assinatura C de uma função nativa `F` (de `NativeFunction<F>`).
    pub fn assinatura(&self, ctx: &Context, f: TypeId) -> Result<(TipoC, Vec<TipoC>), String> {
        match ctx.table.get(f) {
            T::Function { type_params, ret, positional, optional, named, .. } => {
                if !type_params.is_empty() || !optional.is_empty() || !named.is_empty() {
                    return Err("assinatura nativa com parâmetros opcionais, nomeados ou genéricos".to_string());
                }
                let r = self.tipo_c(ctx, *ret)?;
                let mut ps = Vec::with_capacity(positional.len());
                for p in positional.iter() {
                    match self.tipo_c(ctx, *p)? {
                        TipoC::Void => return Err("parâmetro nativo `Void`".to_string()),
                        tc => ps.push(tc),
                    }
                }
                Ok((r, ps))
            }
            _ => Err("NativeFunction sem tipo de função".to_string()),
        }
    }

    /// As assinaturas `F` de todo `NativeFunction<F>` concreto do programa.
    fn assinaturas_do_programa(&self, ctx: &Context) -> Vec<TypeId> {
        // Não só os `NativeFunction<F>` concretos: `lookupFunction<NS, DS>`
        // e `asFunction` instanciam `NativeFunction<NS>` dentro do corpo
        // genérico, e a assinatura concreta aparece só como argumento de
        // tipo da chamada. Todo tipo de função feito só de tipos nativos é
        // uma assinatura candidata (os tipos Dart — `int`, `void` — nunca
        // são classes nativas, então uma assinatura Dart não entra por
        // engano, e uma entrada a mais só custa um trampolim).
        let mut vistas = std::collections::HashSet::new();
        let mut saida = Vec::new();
        for i in 0..ctx.table.len() {
            let f = TypeId(i as u32);
            if matches!(ctx.table.get(f), T::Function { .. })
                && !contem_parametro_de_tipo(ctx, f)
                && self.assinatura(ctx, f).is_ok()
                && vistas.insert(f)
            {
                saida.push(f);
            }
        }
        saida
    }

    /// A tabela (id RTI da classe, letra do tipo C) que o runtime recebe.
    pub fn tabela_rti(&self, ctx: &Context) -> Vec<(i64, char)> {
        let mut v: Vec<(i64, char)> = self.classes.iter().map(|(c, t)| (ctx.id_rti(*c), t.letra())).collect();
        v.sort_unstable();
        v
    }
}

/// Se o tipo menciona um parâmetro de tipo (não é uma assinatura concreta).
fn contem_parametro_de_tipo(ctx: &Context, t: TypeId) -> bool {
    match ctx.table.get(t) {
        T::TypeParameter { .. } => true,
        T::Interface { args, .. } => args.iter().any(|a| contem_parametro_de_tipo(ctx, *a)),
        T::Function { ret, positional, optional, named, .. } => {
            contem_parametro_de_tipo(ctx, *ret)
                || positional.iter().chain(optional.iter()).any(|a| contem_parametro_de_tipo(ctx, *a))
                || named.iter().any(|(_, a, _)| contem_parametro_de_tipo(ctx, *a))
        }
        _ => false,
    }
}

/// O tipo C que `@AbiSpecificIntegerMapping({Abi.x: Int32(), …})` dá à
/// classe no alvo.
fn mapeamento_da_abi(ctx: &Context, c: ClassId) -> Option<TipoC> {
    let decl = ctx.program.classes[c.0 as usize].decl?;
    let unit = ctx.program.unit(decl.unit);
    let a = &unit.ast;
    let alvo = abi_do_alvo();
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
            let chave = match &a.exprs[key.0 as usize].kind {
                ast::ExprKind::Property { name, .. } => ctx.interner.resolve(name.sym),
                _ => continue,
            };
            if chave != alvo {
                continue;
            }
            let classe = match &a.exprs[value.0 as usize].kind {
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
            return primitivo(classe).filter(|t| !matches!(t, TipoC::Void | TipoC::Ptr | TipoC::F32 | TipoC::F64 | TipoC::Bool));
        }
    }
    None
}

/// A chave de uma assinatura: a letra do retorno, `_` e as dos parâmetros
/// (`i_ip` = `Int32 Function(Int32, Pointer)`). O runtime calcula a mesma
/// chave a partir da RTI.
pub fn chave_da_assinatura(ret: TipoC, params: &[TipoC]) -> String {
    let mut s = String::with_capacity(params.len() + 2);
    s.push(ret.letra());
    s.push('_');
    s.extend(params.iter().map(|p| p.letra()));
    s
}

/// Gera os trampolins das assinaturas do programa e as tabelas que o
/// runtime recebe na preparação do isolado.
pub fn lower_ffi(ctx: &Context, module: &mut Module) {
    let Some(tipos) = TiposNativos::do_programa(ctx) else { return };
    module.ffi_tipos = tipos.tabela_rti(ctx);
    let mut feitas = std::collections::HashSet::new();
    for f in tipos.assinaturas_do_programa(ctx) {
        // Uma assinatura que não baixa fica sem trampolim: o runtime recusa
        // a chamada com a assinatura.
        let Ok((ret, params)) = tipos.assinatura(ctx, f) else { continue };
        let chave = chave_da_assinatura(ret, &params);
        if !feitas.insert(chave.clone()) {
            continue;
        }
        let simbolo = format!("df.ffi.{chave}$ent");
        let unit = dartforge_elements::model::UnitId(0);
        let mut b = FnBuilder::new(ctx, unit, simbolo.clone(), format!("ffi {chave}"), Type::Ref);
        b.corpo_do_trampolim(ret, &params);
        module.functions.push(b.func);
        module.functions.extend(b.extra_functions);
        module.ffi_trampolins.push((chave, simbolo));
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O corpo de um trampolim: `(closure, args, desc)` da convenção
    /// uniforme → a chamada nativa → o retorno como `Ref`.
    fn corpo_do_trampolim(&mut self, ret: TipoC, params: &[TipoC]) {
        let clo = Operand::Val(self.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(self.add_param("args".to_string(), Type::Ptr));
        let desc = Operand::Val(self.add_param("desc".to_string(), Type::Ptr));
        let infos: Vec<ParamEntrada> = params
            .iter()
            .map(|_| ParamEntrada { nome: None, kind: ParameterKind::Required, required: true, padrao: Padrao::Nenhum })
            .collect();
        let Some(valores) = self.desempacotar(&infos, args, desc) else {
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
        let mut nativos = Vec::with_capacity(params.len());
        for (v, tc) in valores.into_iter().zip(params.iter()) {
            let x = match tc {
                TipoC::Ptr => self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_endereco_do_ponteiro".to_string(),
                        args: vec![(v, Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ),
                tc => self.coagir(v, tc.tipo_hir()),
            };
            nativos.push((x, *tc));
        }
        let r = self.emit(Instruction::ChamadaNativa { alvo, args: nativos, ret }, ret.tipo_hir());
        let resultado = match ret {
            TipoC::Void => Operand::Constant(Constant::Null),
            TipoC::Ptr => self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_ffi_ponteiro_de_retorno".to_string(),
                    args: vec![(r, Type::I64), (clo, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            _ => self.coagir(r, Type::Ref),
        };
        self.terminate(Terminator::Return(Some(resultado)));
    }
}
