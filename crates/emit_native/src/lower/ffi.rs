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
    /// (`Handle`, `VarArgs`), com o motivo.
    recusadas: std::collections::HashMap<ClassId, &'static str>,
    /// As subclasses de `Struct` e `Union`: por valor, com o layout C.
    compostas: std::collections::HashSet<ClassId>,
    /// `VarArgs` (o último parâmetro de uma função variádica).
    varargs: Option<ClassId>,
}

impl TiposNativos {
    pub fn do_programa(ctx: &Context) -> Option<TiposNativos> {
        let ffi = ctx.program.libraries.iter().position(|l| l.uri == "dart:ffi")?;
        let ffi = dartforge_elements::model::LibraryId(ffi as u32);
        let mut classes = std::collections::HashMap::new();
        let mut recusadas = std::collections::HashMap::new();
        let mut compostas = std::collections::HashSet::new();
        let mut varargs = None;
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
                        classes.insert(id, TipoC::Handle);
                    }
                    "VarArgs" => varargs = Some(id),
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
                compostas.insert(id);
            }
        }
        Some(TiposNativos { classes, recusadas, compostas, varargs })
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

    /// O tipo nativo de um argumento ou retorno: primitivo, ou struct/union
    /// por valor (com a classe, para criar o valor devolvido).
    fn tipo_nativo(&self, ctx: &Context, t: TypeId) -> Result<(TipoNativo, Option<ClassId>), String> {
        if let T::Interface { class, .. } = ctx.table.get(t)
            && self.compostas.contains(class)
        {
            let layout = layout_c(ctx, *class).ok_or_else(|| {
                format!("struct `{}` sem layout", ctx.symbol_name(ctx.program.classes[class.0 as usize].name))
            })?;
            if layout.tamanho == 0 {
                return Err("struct vazia por valor".to_string());
            }
            return Ok((TipoNativo::Composto(layout), Some(*class)));
        }
        Ok((TipoNativo::Prim(self.tipo_c(ctx, t)?), None))
    }

    /// A assinatura de uma função nativa `F` (de `NativeFunction<F>`), com
    /// structs e unions por valor.
    pub fn assinatura_nativa(&self, ctx: &Context, f: TypeId) -> Result<AssinaturaNativa, String> {
        match ctx.table.get(f) {
            T::Function { type_params, ret, positional, optional, named, .. } => {
                if !type_params.is_empty() || !optional.is_empty() || !named.is_empty() {
                    return Err("assinatura nativa com parâmetros opcionais, nomeados ou genéricos".to_string());
                }
                let (r, classe_ret) = self.tipo_nativo(ctx, *ret)?;
                let mut ps = Vec::with_capacity(positional.len());
                let mut cs = Vec::with_capacity(positional.len());
                let mut variadica = None;
                for (i, p) in positional.iter().enumerate() {
                    // `VarArgs<(T1, T2…)>` ou `VarArgs<T>`, só no fim: os
                    // tipos da parte variádica.
                    let variadicos = match ctx.table.get(*p) {
                        T::Interface { class, args, .. } if Some(*class) == self.varargs => {
                            if i + 1 != positional.len() {
                                return Err("VarArgs fora do último parâmetro".to_string());
                            }
                            variadica = Some(i);
                            match args.first().map(|a| ctx.table.get(*a)) {
                                Some(T::Record { positional, named, .. }) if named.is_empty() => positional.to_vec(),
                                Some(_) => args.to_vec(),
                                None => return Err("VarArgs sem argumento de tipo".to_string()),
                            }
                        }
                        _ => vec![*p],
                    };
                    for v in variadicos {
                        match self.tipo_nativo(ctx, v)? {
                            (TipoNativo::Prim(TipoC::Void), _) => return Err("parâmetro nativo `Void`".to_string()),
                            (TipoNativo::Composto(_), _) if variadica.is_some() => {
                                return Err("struct por valor em VarArgs ainda não é suportado no backend nativo".to_string());
                            }
                            (t, c) => {
                                ps.push(t);
                                cs.push(c);
                            }
                        }
                    }
                }
                Ok(AssinaturaNativa { ret: r, classe_ret, params: ps, classes_params: cs, variadica })
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
                && self.assinatura_nativa(ctx, f).is_ok()
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
        // `VarArgs`: a marca da parte variádica na chave.
        v.extend(self.varargs.map(|c| (ctx.id_rti(c), '*')));
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

/// A chave num nome de símbolo, sem ambiguidade: `.` (fim de `S<rti>.`)
/// vira `$p` e `*` (início dos variádicos) vira `$v`.
fn escapar_chave(chave: &str) -> String {
    chave.replace('.', "$p").replace('*', "$v")
}

/// Uma assinatura nativa: retorno (com a classe de uma struct devolvida
/// por valor) e parâmetros.
pub struct AssinaturaNativa {
    pub ret: TipoNativo,
    pub classe_ret: Option<ClassId>,
    pub params: Vec<TipoNativo>,
    pub classes_params: Vec<Option<ClassId>>,
    /// Função variádica: quantos parâmetros são fixos.
    pub variadica: Option<usize>,
}

/// A chave de uma assinatura: a letra do retorno, `_` e as dos parâmetros
/// (`i_ip` = `Int32 Function(Int32, Pointer)`); uma struct por valor é
/// `S<id RTI da classe>.`. O runtime calcula a mesma chave a partir da RTI.
pub fn chave_da_assinatura(ctx: &Context, a: &AssinaturaNativa) -> String {
    let letra = |t: &TipoNativo, classe: Option<ClassId>, s: &mut String| match (t, classe) {
        (TipoNativo::Prim(tc), _) => s.push(tc.letra()),
        (TipoNativo::Composto(_), Some(c)) => s.push_str(&format!("S{}.", ctx.id_rti(c))),
        (TipoNativo::Composto(_), None) => s.push('?'),
    };
    let mut s = String::with_capacity(a.params.len() + 2);
    letra(&a.ret, a.classe_ret, &mut s);
    s.push('_');
    for (i, (p, c)) in a.params.iter().zip(&a.classes_params).enumerate() {
        if a.variadica == Some(i) {
            s.push('*');
        }
        letra(p, *c, &mut s);
    }
    if a.variadica == Some(a.params.len()) {
        s.push('*');
    }
    s
}

/// Gera os trampolins das assinaturas do programa e as tabelas que o
/// runtime recebe na preparação do isolado.
pub fn lower_ffi(ctx: &Context, module: &mut Module) {
    let Some(tipos) = TiposNativos::do_programa(ctx) else { return };
    module.ffi_tipos = tipos.tabela_rti(ctx);
    if let Some(comp) = compostos(ctx) {
        let idx = |v: dartforge_elements::model::VariableId, c: ClassId| {
            super::membros::layout(ctx, c).iter().position(|&x| x == v).map_or(-1, |i| (i + super::enums::base_do_layout(ctx, c)) as i64)
        };
        let mut v: Vec<FfiComposto> = comp
            .classes
            .iter()
            .filter_map(|(c, l)| {
                Some(FfiComposto {
                    rti: ctx.id_rti(*c),
                    classe: i64::from(ctx.id_de_classe(*c)?),
                    campos: (super::membros::layout(ctx, *c).len() + super::enums::base_do_layout(ctx, *c)) as i64,
                    indice_base: idx(comp.base, *c),
                    indice_deslocamento: idx(comp.deslocamento, *c),
                    tamanho: l.tamanho as i64,
                    alinhamento: l.alinhamento as i64,
                })
            })
            .collect();
        v.sort_by_key(|x| x.rti);
        module.ffi_compostos = v;
    }
    let mut feitas = std::collections::HashSet::new();
    for f in tipos.assinaturas_do_programa(ctx) {
        // Uma assinatura que não baixa fica sem trampolim: o runtime recusa
        // a chamada com a assinatura.
        let Ok(assinatura) = tipos.assinatura_nativa(ctx, f) else { continue };
        let chave = chave_da_assinatura(ctx, &assinatura);
        if !feitas.insert(chave.clone()) {
            continue;
        }
        let simbolo = format!("df.ffi.{}$ent", escapar_chave(&chave));
        let unit = dartforge_elements::model::UnitId(0);
        let mut b = FnBuilder::new(ctx, unit, simbolo.clone(), format!("ffi {chave}"), Type::Ref);
        b.corpo_do_trampolim(&assinatura);
        module.functions.push(b.func);
        module.functions.extend(b.extra_functions);
        module.ffi_trampolins.push((chave.clone(), simbolo));
        // O callback da mesma assinatura (`Pointer.fromFunction`,
        // `NativeCallable`): o corpo HIR; a entrada C sai no emissor. Um
        // callback nunca é variádico.
        if assinatura.variadica.is_some() {
            continue;
        }
        let corpo = format!("df.ffi.{}$cb", escapar_chave(&chave));
        let mut b = FnBuilder::new(ctx, unit, corpo.clone(), format!("ffi callback {chave}"), assinatura.ret.tipo_hir());
        b.corpo_do_callback(ctx, &assinatura);
        module.functions.push(b.func);
        module.functions.extend(b.extra_functions);
        module.ffi_callbacks.push(FfiCallback { chave, corpo, ret: assinatura.ret.clone(), params: assinatura.params.clone() });
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O que o front-end da VM reescreve nas chamadas estáticas de `dart:ffi`
    /// (`Pointer.fromFunction`, as factories `NativeCallable.isolateLocal` e
    /// `.listener`, `Struct.create`/`Union.create`): o ajudante da
    /// sobreposição que as implementa, com os mesmos parâmetros, e como os
    /// argumentos de tipo e o sítio chegam a ele.
    pub fn chamada_ffi_redirecionada(&self, fid: usize) -> Option<(usize, Redirecionamento)> {
        let f = &self.ctx.program.functions[fid];
        let c = f.class?;
        if self.ctx.program.library(f.library).uri != "dart:ffi" {
            return None;
        }
        let classe = self.ctx.symbol_name(self.ctx.program.classes[c.0 as usize].name);
        let nome = self.ctx.symbol_name(f.name);
        let (ajudante, forma) = match (classe, nome, f.factory) {
            ("Pointer", "fromFunction", false) if f.static_ => ("_dartforgeFromFunction", Redirecionamento::ComSitio),
            ("NativeCallable", "isolateLocal", true) => ("_dartforgeCallableLocal", Redirecionamento::TuplaDaClasse),
            ("NativeCallable", "listener", true) => ("_dartforgeCallableListener", Redirecionamento::TuplaDaClasse),
            ("Struct" | "Union", "create", false) if f.static_ => ("_dartforgeCompostoCriado", Redirecionamento::Direto),
            _ => return None,
        };
        Some((self.funcao_de_topo("dart:ffi", ajudante)?, forma))
    }

    /// O corpo de um callback nativo: `(contexto, argumentos)` na
    /// representação Dart (um ponteiro chega como endereço) → a closure do
    /// callback pela convenção uniforme → o retorno na representação Dart
    /// (um `Pointer` volta como endereço). Uma exceção da closure fica
    /// pendente: a entrada C devolve o retorno excepcional
    /// (`ffi_callbacks.rs`).
    fn corpo_do_callback(&mut self, ctx: &Context, a: &AssinaturaNativa) {
        let contexto = Operand::Val(self.add_param("ctx".to_string(), Type::I64));
        let valores: Vec<Operand> =
            a.params.iter().enumerate().map(|(i, t)| Operand::Val(self.add_param(format!("a{i}"), t.tipo_hir()))).collect();
        let clo = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_ffi_callback_closure".to_string(),
                args: vec![(contexto.clone(), Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        let mut avaliados = Vec::with_capacity(a.params.len());
        for (i, ((v, t), classe)) in valores.into_iter().zip(a.params.iter()).zip(&a.classes_params).enumerate() {
            let x = match (t, classe) {
                (TipoNativo::Prim(TipoC::Handle), _) => self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_objeto_do_handle".to_string(),
                        args: vec![(v, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (TipoNativo::Prim(TipoC::Ptr), _) => self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_callback_ponteiro".to_string(),
                        args: vec![(contexto.clone(), Type::I64), (Operand::Constant(Constant::Int(i as i64)), Type::I64), (v, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                // Uma struct por valor: uma cópia sobre memória do heap Dart
                // (a VM também a copia para um `TypedData`).
                (TipoNativo::Composto(_), Some(c)) => self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_composto_copia".to_string(),
                        args: vec![(Operand::Constant(Constant::Int(ctx.id_rti(*c))), Type::I64), (v, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                _ => self.coagir(v, Type::Ref),
            };
            avaliados.push((None, x));
        }
        let r = self.chamar_valor_funcao(clo, &avaliados);
        if self.is_terminated() {
            return;
        }
        let resultado = match &a.ret {
            TipoNativo::Prim(TipoC::Void) => None,
            TipoNativo::Prim(TipoC::Handle) => {
                let r = self.coagir(r, Type::Ref);
                Some(self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_handle_novo".to_string(),
                        args: vec![(r, Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ))
            }
            TipoNativo::Prim(TipoC::Ptr) => {
                let r = self.coagir(r, Type::Ref);
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
            TipoNativo::Composto(_) => {
                let r = self.coagir(r, Type::Ref);
                Some(self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_endereco_do_composto".to_string(),
                        args: vec![(r, Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ))
            }
            TipoNativo::Prim(tc) => Some(self.coagir(r, tc.tipo_hir())),
        };
        if !self.is_terminated() {
            self.terminate(Terminator::Return(resultado));
        }
    }

    /// O corpo de um trampolim: `(closure, args, desc)` da convenção
    /// uniforme → a chamada nativa → o retorno como `Ref`. Uma struct por
    /// valor vai como o endereço dos bytes dela; uma devolvida por valor é
    /// criada antes (sobre um `Uint8List` novo, como a VM) e a chamada grava
    /// nela.
    fn corpo_do_trampolim(&mut self, a: &AssinaturaNativa) {
        let clo = Operand::Val(self.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(self.add_param("args".to_string(), Type::Ptr));
        let desc = Operand::Val(self.add_param("desc".to_string(), Type::Ptr));
        let infos: Vec<ParamEntrada> = a
            .params
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
        let resultado = self.chamada_nativa_de_assinatura(alvo, &valores, a, Some(clo));
        self.terminate(Terminator::Return(Some(resultado)));
    }
}

/// O layout C achatado de uma struct ou union do programa.
pub fn layout_c(ctx: &Context, c: ClassId) -> Option<LayoutC> {
    let comp = compostos(ctx)?;
    let l = comp.classes.get(&c)?;
    let mut folhas = Vec::new();
    for (deslocamento, campo) in l.campos.values() {
        achatar(ctx, campo, *deslocamento, &mut folhas)?;
    }
    folhas.sort_by_key(|(o, _)| *o);
    Some(LayoutC { tamanho: l.tamanho, alinhamento: l.alinhamento, folhas })
}

/// As folhas primitivas de um campo em `base`.
fn achatar(ctx: &Context, t: &TipoCampo, base: usize, folhas: &mut Vec<(usize, TipoC)>) -> Option<()> {
    match t {
        TipoCampo::Prim(tc) => folhas.push((base, *tc)),
        TipoCampo::Composto(c) => {
            let dentro = layout_c(ctx, *c)?;
            folhas.extend(dentro.folhas.iter().map(|(o, tc)| (base + o, *tc)));
        }
        TipoCampo::Array { elem, dims } => {
            let passo = tamanho_de(ctx, elem);
            for i in 0..dims.iter().product::<usize>() {
                achatar(ctx, elem, base + i * passo, folhas)?;
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
// (`FnBuilder::ler_campo_ffi`). O runtime recebe o tamanho e o alinhamento
// de cada composto (`sizeOf<S>()`, `Pointer<S>.ref`, `[i]`), por classe.

/// Como uma chamada redirecionada ([`FnBuilder::chamada_ffi_redirecionada`])
/// passa ao ajudante o que o front-end da VM resolveria em compilação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redirecionamento {
    /// Os mesmos argumentos e a mesma tupla de tipos.
    Direto,
    /// A tupla de uma factory de classe genérica (o último argumento) vira a
    /// do ajudante genérico.
    TuplaDaClasse,
    /// Mais um argumento: o sítio da chamada (`fromFunction`, um trampolim
    /// por sítio).
    ComSitio,
}

/// O tipo de um campo de composto.
#[derive(Debug, Clone)]
pub enum TipoCampo {
    Prim(TipoC),
    Composto(ClassId),
    /// `@Array(d0, d1…)` de elementos `elem` (o tipo mais interno).
    Array { elem: Box<TipoCampo>, dims: Vec<usize> },
}

/// O layout de uma struct ou union.
#[derive(Debug, Clone)]
pub struct Composto {
    pub tamanho: usize,
    pub alinhamento: usize,
    pub campos: std::collections::HashMap<dartforge_elements::model::VariableId, (usize, TipoCampo)>,
}

/// Os compostos do programa e os campos de `_Compound` que os acessos usam.
pub struct Compostos {
    pub classes: std::collections::HashMap<ClassId, Composto>,
    pub base: dartforge_elements::model::VariableId,
    pub deslocamento: dartforge_elements::model::VariableId,
}

/// Os compostos do programa (calculados uma vez por contexto).
pub fn compostos<'c>(ctx: &'c Context) -> Option<&'c Compostos> {
    ctx.compostos_ffi.get_or_init(|| calcular_compostos(ctx)).as_ref()
}

fn calcular_compostos(ctx: &Context) -> Option<Compostos> {
    let tipos = TiposNativos::do_programa(ctx)?;
    let compound = ctx.classe_do_sdk("ffi", "_Compound")?;
    let campo = |nome: &str| {
        ctx.program.classes[compound.0 as usize]
            .fields
            .iter()
            .copied()
            .find(|v| ctx.symbol_name(ctx.program.variables[v.0 as usize].name) == nome)
    };
    let (base, deslocamento) = (campo("_typedDataBase")?, campo("_offsetInBytes")?);
    let struct_ = ctx.classe_do_sdk("ffi", "Struct");
    let union_ = ctx.classe_do_sdk("ffi", "Union");
    let mut calc = Calculo { ctx, tipos: &tipos, struct_, union_, feitos: Default::default(), em_curso: Default::default() };
    for i in 0..ctx.program.classes.len() {
        let c = ClassId(i as u32);
        if calc.e_composto(c) {
            calc.layout(c);
        }
    }
    Some(Compostos { classes: calc.feitos.into_iter().filter_map(|(c, l)| Some((c, l?))).collect(), base, deslocamento })
}

struct Calculo<'a, 'c> {
    ctx: &'a Context<'c>,
    tipos: &'a TiposNativos,
    struct_: Option<ClassId>,
    union_: Option<ClassId>,
    feitos: std::collections::HashMap<ClassId, Option<Composto>>,
    em_curso: std::collections::HashSet<ClassId>,
}

impl Calculo<'_, '_> {
    fn e_composto(&self, c: ClassId) -> bool {
        let s = self.ctx.program.classes[c.0 as usize].supertype_class;
        s.is_some() && (s == self.struct_ || s == self.union_)
    }

    fn e_union(&self, c: ClassId) -> bool {
        self.union_.is_some() && self.ctx.program.classes[c.0 as usize].supertype_class == self.union_
    }

    /// `(tamanho, alinhamento)` de um tipo de campo.
    fn medida(&mut self, t: &TipoCampo) -> Option<(usize, usize)> {
        match t {
            TipoCampo::Prim(tc) => Some((tc.tamanho_c(), tc.tamanho_c())),
            TipoCampo::Composto(c) => self.layout(*c).map(|l| (l.tamanho, l.alinhamento)),
            TipoCampo::Array { elem, dims } => {
                let (t, a) = self.medida(elem)?;
                Some((t * dims.iter().product::<usize>(), a))
            }
        }
    }

    fn layout(&mut self, c: ClassId) -> Option<Composto> {
        if let Some(l) = self.feitos.get(&c) {
            return l.clone();
        }
        if !self.em_curso.insert(c) {
            // Composto que contém a si mesmo por valor: recusado.
            return None;
        }
        let r = self.calcular(c);
        self.em_curso.remove(&c);
        self.feitos.insert(c, r.clone());
        r
    }

    fn calcular(&mut self, c: ClassId) -> Option<Composto> {
        let ctx = self.ctx;
        let empacotado = anotacao_inteira(ctx, c, "Packed");
        let union_ = self.e_union(c);
        let mut deslocamento = 0usize;
        let mut maior_alinhamento = 1usize;
        let mut tamanho_max = 0usize;
        let mut campos = std::collections::HashMap::new();
        for &vid in &ctx.program.classes[c.0 as usize].fields {
            let v = &ctx.program.variables[vid.0 as usize];
            if v.static_ || !v.external {
                continue;
            }
            let tipo = self.tipo_do_campo(vid)?;
            let (t, mut a) = self.medida(&tipo)?;
            if let Some(p) = empacotado {
                a = a.min(p.max(1) as usize);
            }
            let a = a.max(1);
            maior_alinhamento = maior_alinhamento.max(a);
            let off = if union_ { 0 } else { deslocamento.div_ceil(a) * a };
            campos.insert(vid, (off, tipo));
            deslocamento = off + t;
            tamanho_max = tamanho_max.max(t);
        }
        let bruto = if union_ { tamanho_max } else { deslocamento };
        let tamanho = bruto.div_ceil(maior_alinhamento) * maior_alinhamento;
        Some(Composto { tamanho, alinhamento: maior_alinhamento, campos })
    }

    /// O tipo nativo de um campo: pela anotação (`@Int32()`, `@Long()`,
    /// `@Array(…)`) ou pelo tipo declarado (`Pointer<…>`, outra struct).
    fn tipo_do_campo(&mut self, vid: dartforge_elements::model::VariableId) -> Option<TipoCampo> {
        let ctx = self.ctx;
        let declarado = super::membros::tipo_da_variavel(ctx, vid);
        let dartforge_elements::model::VariableRef::Field { unit, member, .. } = ctx.program.variables[vid.0 as usize].node else {
            return None;
        };
        let u = ctx.program.unit(unit);
        let ast = &u.ast;
        for an in ast.members[member.0 as usize].metadata.iter() {
            let nome = ctx.interner.resolve(an.name.first()?.sym);
            if nome == "Array" {
                let dims = dimensoes_do_array(&u.source, ast, an)?;
                let elem = self.elemento_do_array(declarado)?;
                return Some(TipoCampo::Array { elem: Box::new(elem), dims });
            }
            if let Some(tc) = primitivo(nome) {
                return Some(TipoCampo::Prim(tc));
            }
            if let Some((_, tc)) = self.tipos.classes.iter().find(|(c, _)| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == nome) {
                return Some(TipoCampo::Prim(*tc));
            }
        }
        match ctx.table.get(declarado) {
            T::Interface { class, .. } if self.tipos.classes.get(class) == Some(&TipoC::Ptr) => Some(TipoCampo::Prim(TipoC::Ptr)),
            T::Interface { class, .. } if self.e_composto(*class) => Some(TipoCampo::Composto(*class)),
            _ => None,
        }
    }

    /// O tipo mais interno de `Array<Array<…<E>>>`.
    fn elemento_do_array(&mut self, t: TypeId) -> Option<TipoCampo> {
        let T::Interface { class, args, .. } = self.ctx.table.get(t) else { return None };
        let nome = self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name);
        if nome == "Array" {
            return self.elemento_do_array(*args.first()?);
        }
        if let Some(tc) = self.tipos.classes.get(class) {
            return Some(TipoCampo::Prim(*tc));
        }
        if self.e_composto(*class) {
            return Some(TipoCampo::Composto(*class));
        }
        None
    }
}

/// As dimensões de `@Array(2, 3)` ou `@Array.multi([2, 3])`.
fn dimensoes_do_array(fonte: &str, ast: &ast::Ast, an: &ast::Annotation) -> Option<Vec<usize>> {
    let args = &an.arguments.as_ref()?.args;
    let inteiro = |e: ast::ExprId| inteiro_literal(fonte, ast, e).and_then(|n| usize::try_from(n).ok());
    if an.name.len() > 1 {
        // `Array.multi([…])`.
        let ast::ExprKind::List { elements, .. } = &ast.expr(args.first()?.value).kind else { return None };
        return elements
            .iter()
            .map(|el| match el {
                ast::CollectionElement::Expression(e) => inteiro(*e),
                _ => None,
            })
            .collect();
    }
    args.iter().map(|a| inteiro(a.value)).collect()
}

/// O argumento inteiro de uma anotação de classe (`@Packed(1)`).
fn anotacao_inteira(ctx: &Context, c: ClassId, nome: &str) -> Option<i64> {
    let decl = ctx.program.classes[c.0 as usize].decl?;
    let u = ctx.program.unit(decl.unit);
    for an in u.ast.decls[decl.decl.0 as usize].metadata.iter() {
        if an.name.last().map(|n| ctx.interner.resolve(n.sym)) != Some(nome) {
            continue;
        }
        let arg = an.arguments.as_ref()?.args.first()?;
        return inteiro_literal(&u.source, &u.ast, arg.value);
    }
    None
}

/// O valor de um literal inteiro (decimal ou hexadecimal).
fn inteiro_literal(fonte: &str, ast: &ast::Ast, e: ast::ExprId) -> Option<i64> {
    let ast::ExprKind::Int(span) = &ast.expr(e).kind else { return None };
    let texto = fonte.get(span.start as usize..span.end as usize)?.replace('_', "");
    match texto.strip_prefix("0x").or_else(|| texto.strip_prefix("0X")) {
        Some(h) => i64::from_str_radix(h, 16).ok(),
        None => texto.parse().ok(),
    }
}

impl TipoC {
    /// O sufixo dos natives de carga/gravação (`DartForge_ffi_carregar_i32`).
    fn sufixo_de_memoria(self) -> &'static str {
        match self {
            TipoC::I8 => "i8",
            TipoC::U8 | TipoC::Bool => "u8",
            TipoC::I16 => "i16",
            TipoC::U16 => "u16",
            TipoC::I32 => "i32",
            TipoC::U32 => "u32",
            TipoC::I64 | TipoC::Ptr | TipoC::Handle => "i64",
            TipoC::U64 => "u64",
            TipoC::F32 => "f32",
            TipoC::F64 => "f64",
            TipoC::Void => "i64",
        }
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O campo `external` de uma struct/union: `(base, deslocamento, tipo)`
    /// do acesso — `_typedDataBase` do objeto e `_offsetInBytes` + o
    /// deslocamento do campo no layout.
    fn endereco_do_campo_ffi(&mut self, obj: Operand, vid: dartforge_elements::model::VariableId) -> Option<(Operand, Operand, TipoCampo)> {
        let v = &self.ctx.program.variables[vid.0 as usize];
        if !v.external || v.static_ {
            return None;
        }
        let comp = compostos(self.ctx)?;
        let (off, tipo) = comp.classes.get(&v.class?)?.campos.get(&vid)?.clone();
        let (vbase, vdesl) = (comp.base, comp.deslocamento);
        let span = dartforge_diagnostics::Span { start: 0, end: 0 };
        let base = self.ler_campo(obj.clone(), vbase, span);
        let d0 = self.ler_campo(obj, vdesl, span);
        let d0 = self.coagir(d0, Type::I64);
        let d = self.emit(Instruction::Add(d0, Operand::Constant(Constant::Int(off as i64))), Type::I64);
        Some((base, d, tipo))
    }

    /// Leitura de um campo `external` de struct/union (`None`: não é um).
    pub fn ler_campo_ffi(&mut self, obj: Operand, vid: dartforge_elements::model::VariableId) -> Option<Operand> {
        let (base, d, tipo) = self.endereco_do_campo_ffi(obj, vid)?;
        Some(self.ler_memoria_ffi(base, d, tipo))
    }

    /// O valor Dart do tipo `tipo` em `base` + `d` (base `Pointer` ou
    /// `TypedData`): um campo de struct ou uma variável `@Native`.
    fn ler_memoria_ffi(&mut self, base: Operand, d: Operand, tipo: TipoCampo) -> Operand {
        match tipo {
            TipoCampo::Prim(tc) => {
                let ret = if matches!(tc, TipoC::F32 | TipoC::F64) { Type::F64 } else { Type::I64 };
                let v = self.emit(
                    Instruction::CallRuntime {
                        name: format!("dartforge_nativo_DartForge_ffi_carregar_{}", tc.sufixo_de_memoria()),
                        args: vec![(base, Type::Ref), (d, Type::I64)],
                        ret_ty: ret,
                    },
                    ret,
                );
                match tc {
                    TipoC::Bool => self.emit(Instruction::ICmp(ICmpOp::Ne, v, Operand::Constant(Constant::Int(0))), Type::I1),
                    TipoC::Ptr => self.emit_call_with_check(
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
            TipoCampo::Composto(c) => self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_ffi_composto".to_string(),
                    args: vec![(Operand::Constant(Constant::Int(self.ctx.id_rti(c))), Type::I64), (base, Type::Ref), (d, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            TipoCampo::Array { dims, elem } => {
                // `Array._(base, deslocamento, primeira dimensão, as demais)`.
                let mut tabela = Vec::new();
                for &n in &dims[1..] {
                    tabela.push(b'i');
                    tabela.extend_from_slice(&(n as i64).to_le_bytes());
                }
                let palavras: Vec<i64> = tabela
                    .chunks(8)
                    .map(|c| {
                        let mut w = [0u8; 8];
                        w[..c.len()].copy_from_slice(c);
                        i64::from_le_bytes(w)
                    })
                    .collect();
                let dados = self.emit(Instruction::ConstArray(palavras), Type::Ptr);
                let resto = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_lista_de_tabela".to_string(),
                        args: vec![(dados, Type::Ptr), (Operand::Constant(Constant::Int(tabela.len() as i64)), Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                );
                let Some(f) = self.funcao_de_topo("dart:ffi", "_dartforgeArray") else {
                    return self.nao_suportado("dart:ffi sem `_dartforgeArray`", dartforge_diagnostics::Span { start: 0, end: 0 });
                };
                let bytes = tamanho_de(self.ctx, &elem) as i64;
                let args = vec![
                    (None, base),
                    (None, d),
                    (None, Operand::Constant(Constant::Int(dims[0] as i64))),
                    (None, resto),
                    (None, Operand::Constant(Constant::Int(bytes))),
                ];
                let args = self.casar_args(f, &args);
                self.chamar_direto(f, None, args)
            }
        }
    }

    /// Gravação de um campo `external` de struct/union (`false`: não é um).
    pub fn gravar_campo_ffi(&mut self, obj: Operand, vid: dartforge_elements::model::VariableId, val: Operand) -> bool {
        let Some((base, d, tipo)) = self.endereco_do_campo_ffi(obj, vid) else { return false };
        self.gravar_memoria_ffi(base, d, tipo, val);
        true
    }

    /// Grava o valor Dart `val` do tipo `tipo` em `base` + `d`.
    fn gravar_memoria_ffi(&mut self, base: Operand, d: Operand, tipo: TipoCampo, val: Operand) {
        match tipo {
            TipoCampo::Prim(tc) => {
                let (v, ty) = match tc {
                    TipoC::F32 | TipoC::F64 => (self.coagir(val, Type::F64), Type::F64),
                    TipoC::Bool => {
                        let b = self.coagir(val, Type::I1);
                        (self.emit(Instruction::ZExt { op: b, from: Type::I1, to: Type::I64 }, Type::I64), Type::I64)
                    }
                    TipoC::Ptr => {
                        let p = self.coagir(val, Type::Ref);
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
                    _ => (self.coagir(val, Type::I64), Type::I64),
                };
                self.emit(
                    Instruction::CallRuntime {
                        name: format!("dartforge_nativo_DartForge_ffi_gravar_{}", tc.sufixo_de_memoria()),
                        args: vec![(base, Type::Ref), (d, Type::I64), (v, ty)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
            }
            TipoCampo::Composto(_) | TipoCampo::Array { .. } => {
                // Por valor: copia os bytes do composto atribuído.
                let tamanho = tamanho_de(self.ctx, &tipo);
                let Some(comp) = compostos(self.ctx) else { return };
                let (vbase, vdesl) = (comp.base, comp.deslocamento);
                let span = dartforge_diagnostics::Span { start: 0, end: 0 };
                let val = self.coagir(val, Type::Ref);
                let ob = self.ler_campo(val.clone(), vbase, span);
                let od = self.ler_campo(val, vdesl, span);
                let od = self.coagir(od, Type::I64);
                self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_nativo_DartForge_ffi_copiar".to_string(),
                        args: vec![
                            (base, Type::Ref),
                            (d, Type::I64),
                            (ob, Type::Ref),
                            (od, Type::I64),
                            (Operand::Constant(Constant::Int(tamanho as i64)), Type::I64),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
            }
        }
    }

    /// O endereço (i64) do símbolo nativo `simbolo` no processo.
    fn endereco_do_simbolo(&mut self, simbolo: &[u8]) -> Operand {
        let palavras: Vec<i64> = simbolo
            .chunks(8)
            .map(|c| {
                let mut w = [0u8; 8];
                w[..c.len()].copy_from_slice(c);
                i64::from_le_bytes(w)
            })
            .collect();
        let nome = self.emit(Instruction::ConstArray(palavras), Type::Ptr);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_ffi_simbolo_nativo".to_string(),
                args: vec![(nome, Type::Ptr), (Operand::Constant(Constant::Int(simbolo.len() as i64)), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// Uma variável `external` com `@Native<T>()`: o símbolo e o tipo do
    /// dado (`None`: não é uma).
    fn variavel_nativa(&self, vid: dartforge_elements::model::VariableId) -> Option<Result<(Vec<u8>, TipoCampo), String>> {
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
        let Some(tipos) = TiposNativos::do_programa(self.ctx) else { return Some(Err("@Native sem dart:ffi".to_string())) };
        let Some(&t) = an.type_args.first() else {
            return Some(Err("@Native de variável sem o argumento de tipo".to_string()));
        };
        let tipo = match tipos.tipo_nativo_da_anotacao(self.ctx, &u.ast, t, v.library) {
            Ok((TipoNativo::Prim(TipoC::Void), _)) => return Some(Err("@Native de variável `Void`".to_string())),
            Ok((TipoNativo::Prim(tc), _)) => TipoCampo::Prim(tc),
            Ok((TipoNativo::Composto(_), Some(c))) => TipoCampo::Composto(c),
            Ok(_) => return Some(Err("tipo de variável @Native".to_string())),
            Err(m) => return Some(Err(m)),
        };
        let simbolo = simbolo_da_anotacao(self.ctx, &u.ast, an).unwrap_or_else(|| self.ctx.symbol_name(v.name).as_bytes().to_vec());
        Some(Ok((simbolo, tipo)))
    }

    /// A base (`Pointer` para o símbolo) de uma variável `@Native`.
    fn base_da_variavel_nativa(&mut self, simbolo: &[u8]) -> Operand {
        let e = self.endereco_do_simbolo(simbolo);
        self.emit_call_with_check(
            Instruction::CallRuntime { name: "dartforge_ffi_ponteiro_novo".to_string(), args: vec![(e, Type::I64)], ret_ty: Type::Ref },
            Type::Ref,
        )
    }

    /// Leitura de uma variável `@Native` (`None`: não é uma).
    pub fn ler_variavel_nativa(&mut self, vid: dartforge_elements::model::VariableId, span: dartforge_diagnostics::Span) -> Option<Operand> {
        let (simbolo, tipo) = match self.variavel_nativa(vid)? {
            Ok(x) => x,
            Err(m) => return Some(self.nao_suportado(&m, span)),
        };
        let base = self.base_da_variavel_nativa(&simbolo);
        Some(self.ler_memoria_ffi(base, Operand::Constant(Constant::Int(0)), tipo))
    }

    /// Gravação de uma variável `@Native` (`false`: não é uma).
    pub fn gravar_variavel_nativa(&mut self, vid: dartforge_elements::model::VariableId, val: Operand, span: dartforge_diagnostics::Span) -> bool {
        let Some(r) = self.variavel_nativa(vid) else { return false };
        let (simbolo, tipo) = match r {
            Ok(x) => x,
            Err(m) => {
                self.nao_suportado(&m, span);
                return true;
            }
        };
        let base = self.base_da_variavel_nativa(&simbolo);
        self.gravar_memoria_ffi(base, Operand::Constant(Constant::Int(0)), tipo, val);
        true
    }

    /// `Native.addressOf<T>(x)` (o que o front-end da VM reescreve): o
    /// `Pointer` para o símbolo da função ou variável `@Native` citada.
    pub fn endereco_de_native(&mut self, ast: &ast::Ast, arg: ast::ExprId, span: dartforge_diagnostics::Span) -> Operand {
        use dartforge_elements::model::Element;
        let resolvido = self.ctx.get_resolved(self.unit_id, arg).cloned();
        let el = match resolvido {
            Some(dartforge_types::resolved::Resolved::Element(e)) => Some(e),
            _ => match &ast.expr(arg).kind {
                ast::ExprKind::Identifier(n) => match self.resolver_por_nome(n.sym) {
                    Some(dartforge_types::resolved::Resolved::Element(e)) => Some(e),
                    _ => None,
                },
                _ => None,
            },
        };
        let simbolo = match el {
            Some(Element::Function(f)) => {
                let fid = f.0 as usize;
                match self.ctx.program.functions[fid].variable {
                    Some(vid) => self.variavel_nativa(vid).and_then(Result::ok).map(|(s, _)| s),
                    None => metadados_da_funcao(self.ctx, fid).and_then(|(_, a, meta)| {
                        let an = meta.iter().find(|an| an.name.last().map(|n| self.ctx.interner.resolve(n.sym)) == Some("Native"))?;
                        Some(simbolo_da_anotacao(self.ctx, a, an).unwrap_or_else(|| self.ctx.symbol_name(self.ctx.program.functions[fid].name).as_bytes().to_vec()))
                    }),
                }
            }
            Some(Element::Variable(vid)) => self.variavel_nativa(vid).and_then(Result::ok).map(|(s, _)| s),
            _ => None,
        };
        let Some(simbolo) = simbolo else {
            return self.erro_de_linguagem("Argument to 'Native.addressOf' must be annotated with @Native.", span);
        };
        self.base_da_variavel_nativa(&simbolo)
    }
}

/// O `symbol:` de uma anotação `@Native`.
fn simbolo_da_anotacao(ctx: &Context, ast: &ast::Ast, an: &ast::Annotation) -> Option<Vec<u8>> {
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
fn tamanho_de(ctx: &Context, t: &TipoCampo) -> usize {
    match t {
        TipoCampo::Prim(tc) => tc.tamanho_c(),
        TipoCampo::Composto(c) => compostos(ctx).and_then(|x| x.classes.get(c)).map_or(0, |l| l.tamanho),
        TipoCampo::Array { elem, dims } => tamanho_de(ctx, elem) * dims.iter().product::<usize>(),
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
fn metadados_da_funcao<'p>(ctx: &'p Context, fid: usize) -> Option<(&'p str, &'p ast::Ast, &'p [ast::Annotation])> {
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

/// Marca de [`TiposNativos::tipo_c_da_anotacao`]: o nome é de uma
/// struct/union (resolvida por [`TiposNativos::tipo_nativo_da_anotacao`]).
const COMPOSTO_NA_ANOTACAO: &str = "struct por valor";

impl TiposNativos {
    /// O tipo nativo escrito na anotação, com struct/union por valor (a
    /// classe do mesmo nome, de preferência da biblioteca `lib`).
    fn tipo_nativo_da_anotacao(
        &self,
        ctx: &Context,
        ast: &ast::Ast,
        t: ast::TypeId,
        lib: dartforge_elements::model::LibraryId,
    ) -> Result<(TipoNativo, Option<ClassId>), String> {
        match self.tipo_c_da_anotacao(ctx, ast, t) {
            Ok(tc) => Ok((TipoNativo::Prim(tc), None)),
            Err(m) if m == COMPOSTO_NA_ANOTACAO => {
                let ast::TypeKind::Named { name, .. } = &ast.ty(t).kind else { return Err(m) };
                let nome = ctx.interner.resolve(name.last().ok_or("tipo sem nome")?.sym);
                let mut candidatas: Vec<ClassId> = self
                    .compostas
                    .iter()
                    .copied()
                    .filter(|c| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == nome)
                    .collect();
                candidatas.sort_by_key(|c| (ctx.program.classes[c.0 as usize].library != lib, c.0));
                let c = *candidatas.first().ok_or(m)?;
                let layout = layout_c(ctx, c).ok_or_else(|| format!("struct `{nome}` sem layout"))?;
                Ok((TipoNativo::Composto(layout), Some(c)))
            }
            Err(m) => Err(m),
        }
    }

    /// O tipo C de um tipo nativo escrito na anotação (`Pointer<Utf8>`,
    /// `Size`, `Int32`), pelo nome da classe.
    fn tipo_c_da_anotacao(&self, ctx: &Context, ast: &ast::Ast, t: ast::TypeId) -> Result<TipoC, String> {
        match &ast.ty(t).kind {
            ast::TypeKind::Void => Ok(TipoC::Void),
            ast::TypeKind::Named { name, .. } => {
                let nome = ctx.interner.resolve(name.last().ok_or("tipo sem nome")?.sym);
                if let Some(tc) = primitivo(nome) {
                    return Ok(tc);
                }
                if let Some((_, tc)) = self.classes.iter().find(|(c, _)| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == nome) {
                    return Ok(*tc);
                }
                if self.compostas.iter().any(|c| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == nome) {
                    return Err(COMPOSTO_NA_ANOTACAO.to_string());
                }
                if let Some((_, m)) = self.recusadas.iter().find(|(c, _)| ctx.symbol_name(ctx.program.classes[c.0 as usize].name) == nome) {
                    return Err(format!("{m} ainda não é suportado no backend nativo"));
                }
                Err(format!("`{nome}` não é um tipo nativo suportado em @Native"))
            }
            _ => Err("tipo nativo inesperado em @Native".to_string()),
        }
    }
}

/// A assinatura nativa do tipo de função `t` escrito numa anotação
/// (seguindo `typedef`s, inclusive de outra unidade).
fn assinatura_da_anotacao(
    ctx: &Context,
    tipos: &TiposNativos,
    ast: &ast::Ast,
    t: ast::TypeId,
    lib: dartforge_elements::model::LibraryId,
    profundidade: usize,
) -> Result<AssinaturaNativa, String> {
    let de_params = |ast: &ast::Ast, lib, ret: Option<ast::TypeId>, params: &[ast::Parameter]| -> Result<AssinaturaNativa, String> {
        let (r, classe_ret) = match ret {
            Some(r) => tipos.tipo_nativo_da_anotacao(ctx, ast, r, lib)?,
            None => (TipoNativo::Prim(TipoC::Void), None),
        };
        let mut ps = Vec::with_capacity(params.len());
        let mut cs = Vec::with_capacity(params.len());
        for p in params {
            let t = p.ty.ok_or_else(|| "parâmetro nativo sem tipo".to_string())?;
            let (tn, c) = tipos.tipo_nativo_da_anotacao(ctx, ast, t, lib)?;
            ps.push(tn);
            cs.push(c);
        }
        Ok(AssinaturaNativa { ret: r, classe_ret, params: ps, classes_params: cs, variadica: None })
    };
    match &ast.ty(t).kind {
        ast::TypeKind::Function { return_type, parameters, .. } => de_params(ast, lib, *return_type, parameters),
        ast::TypeKind::Named { name, .. } if profundidade < 8 => {
            let nome = ctx.interner.resolve(name.last().ok_or("tipo sem nome")?.sym);
            // O `typedef` com esse nome, de preferência da mesma biblioteca.
            let mut candidatos: Vec<&dartforge_elements::model::TypedefElement> =
                ctx.program.typedefs.iter().filter(|td| ctx.symbol_name(td.name) == nome).collect();
            candidatos.sort_by_key(|td| td.library != lib);
            let Some(td) = candidatos.first() else {
                return Err("@Native de variável (ponteiro para dado nativo) ainda não é suportado".to_string());
            };
            let u = ctx.program.unit(td.decl.unit);
            let ast::DeclKind::Typedef(decl) = &u.ast.decls[td.decl.decl.0 as usize].kind else {
                return Err(format!("`{nome}` não é um typedef"));
            };
            match &decl.kind {
                ast::TypedefKind::Alias(alvo) => assinatura_da_anotacao(ctx, tipos, &u.ast, *alvo, td.library, profundidade + 1),
                ast::TypedefKind::Legacy { return_type, parameters } => de_params(&u.ast, td.library, *return_type, parameters),
            }
        }
        _ => Err("assinatura nativa inesperada em @Native".to_string()),
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O corpo de um `external` com `@Native` (`None`: não tem a anotação).
    pub fn chamar_native_anotado(&mut self, fid: usize, args: &[Operand], span: dartforge_diagnostics::Span) -> Option<Operand> {
        let (fonte, ast, meta) = metadados_da_funcao(self.ctx, fid)?;
        let an = meta.iter().find(|an| an.name.last().map(|n| self.ctx.interner.resolve(n.sym)) == Some("Native"))?;
        let Some(tipos) = TiposNativos::do_programa(self.ctx) else {
            return Some(self.nao_suportado("@Native sem dart:ffi", span));
        };
        // A assinatura: `NF` da anotação.
        let Some(&nf) = an.type_args.first() else {
            return Some(self.nao_suportado("@Native sem o argumento de tipo da assinatura nativa", span));
        };
        let lib = self.ctx.program.functions[fid].library;
        let assinatura = assinatura_da_anotacao(self.ctx, &tipos, ast, nf, lib, 0);
        let assinatura = match assinatura {
            Ok(x) => x,
            Err(m) => return Some(self.nao_suportado(&m, span)),
        };
        // O símbolo: `symbol: 'x'`, ou o nome da função.
        let simbolo = simbolo_da_anotacao(self.ctx, ast, an)
            .unwrap_or_else(|| self.ctx.symbol_name(self.ctx.program.functions[fid].name).as_bytes().to_vec());
        let _ = fonte;
        let alvo = self.endereco_do_simbolo(&simbolo);
        if args.len() != assinatura.params.len() {
            return Some(self.nao_suportado("@Native com aridade diferente da assinatura nativa", span));
        }
        let dart_ret = self.repr_retorno(fid);
        let r = self.chamada_nativa_de_assinatura(alvo, args, &assinatura, None);
        Some(if dart_ret == Type::Void { Operand::Constant(Constant::Null) } else { self.coagir(r, dart_ret) })
    }

    /// A chamada nativa de `alvo` com os valores Dart `args` pela
    /// assinatura: ponteiros e structs pelos endereços, o retorno de volta
    /// como valor Dart (`Pointer`, struct nova sobre `Uint8List`, primitivo)
    /// — o que o trampolim de `asFunction` e o corpo de um `@Native` fazem.
    /// `closure`: a do trampolim, para o tipo do `Pointer` devolvido.
    fn chamada_nativa_de_assinatura(
        &mut self,
        alvo: Operand,
        args: &[Operand],
        a: &AssinaturaNativa,
        closure: Option<Operand>,
    ) -> Operand {
        let endereco = |b: &mut Self, nome: &str, v: Operand| {
            let v = b.coagir(v, Type::Ref);
            b.emit_call_with_check(
                Instruction::CallRuntime { name: nome.to_string(), args: vec![(v, Type::Ref)], ret_ty: Type::I64 },
                Type::I64,
            )
        };
        // `Handle`: as células dos objetos passados vivem num escopo aberto
        // em volta da chamada (os handles locais da VM).
        let com_handles = std::iter::once(&a.ret).chain(a.params.iter()).any(|t| matches!(t, TipoNativo::Prim(TipoC::Handle)));
        let escopo = com_handles.then(|| {
            self.emit(
                Instruction::CallRuntime { name: "dartforge_ffi_handles_abrir".to_string(), args: Vec::new(), ret_ty: Type::I64 },
                Type::I64,
            )
        });
        let mut nativos = Vec::with_capacity(a.params.len());
        for (v, t) in args.iter().cloned().zip(a.params.iter()) {
            let x = match t {
                TipoNativo::Prim(TipoC::Handle) => endereco(self, "dartforge_ffi_handle_novo", v),
                TipoNativo::Prim(TipoC::Ptr) => endereco(self, "dartforge_ffi_endereco_do_ponteiro", v),
                TipoNativo::Composto(_) => endereco(self, "dartforge_ffi_endereco_do_composto", v),
                TipoNativo::Prim(tc) => self.coagir(v, tc.tipo_hir()),
            };
            nativos.push((x, t.clone()));
        }
        match (&a.ret, a.classe_ret) {
            (TipoNativo::Composto(_), Some(c)) => {
                let novo = self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_ffi_composto_novo".to_string(),
                        args: vec![(Operand::Constant(Constant::Int(self.ctx.id_rti(c))), Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                );
                let destino = endereco(self, "dartforge_ffi_endereco_do_composto", novo.clone());
                self.emit(
                    Instruction::ChamadaNativaComposta { alvo, args: nativos, ret: a.ret.clone(), destino: Some(destino), variadica: a.variadica },
                    Type::Void,
                );
                novo
            }
            (TipoNativo::Composto(_), None) => unreachable!("struct devolvida sem classe"),
            (TipoNativo::Prim(ret), _) => {
                let ret = *ret;
                let r = if a.variadica.is_none() && nativos.iter().all(|(_, t)| matches!(t, TipoNativo::Prim(_))) {
                    let prims = nativos
                        .into_iter()
                        .map(|(x, t)| match t {
                            TipoNativo::Prim(tc) => (x, tc),
                            TipoNativo::Composto(_) => unreachable!(),
                        })
                        .collect();
                    self.emit(Instruction::ChamadaNativa { alvo, args: prims, ret }, ret.tipo_hir())
                } else {
                    self.emit(
                        Instruction::ChamadaNativaComposta { alvo, args: nativos, ret: a.ret.clone(), destino: None, variadica: a.variadica },
                        ret.tipo_hir(),
                    )
                };
                let trampolim = closure.is_some();
                let resultado = match (ret, closure) {
                    (TipoC::Void, _) => Operand::Constant(Constant::Null),
                    (TipoC::Handle, _) => self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_ffi_objeto_do_handle".to_string(),
                            args: vec![(r, Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (TipoC::Ptr, Some(clo)) => self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_ffi_ponteiro_de_retorno".to_string(),
                            args: vec![(r, Type::I64), (clo, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (TipoC::Ptr, None) => self.emit_call_with_check(
                        Instruction::CallRuntime { name: "dartforge_ffi_ponteiro_novo".to_string(), args: vec![(r, Type::I64)], ret_ty: Type::Ref },
                        Type::Ref,
                    ),
                    // O trampolim devolve `Ref`; um `@Native`, o valor cru
                    // (quem chama o coage ao retorno Dart, sem caixa).
                    _ if trampolim => self.coagir(r, Type::Ref),
                    _ => r,
                };
                if let Some(e) = escopo {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_ffi_handles_fechar".to_string(),
                            args: vec![(e, Type::I64)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    );
                }
                resultado
            }
        }
    }
}
