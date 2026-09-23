//! Lowering do AST e tabelas de tipos para a HIR nativa.
//!
//! O contrato entre este lowering, o emissor e o runtime está em
//! `docs/NATIVO-PLANO.md` §6 (R, E, N, G).

pub mod atribuicao;
pub mod captura;
pub mod cascata;
pub mod chamadas;
pub mod closures;
pub mod comandos;
pub mod despacho;
pub mod enums;
pub mod expressoes;
pub mod extensoes;
pub mod fn_builder;
pub mod heranca;
pub mod literais;
pub mod locais;
pub mod membros;
pub mod operadores;
pub mod padroes;
pub mod registros;
pub mod sdk_por_nome;
pub mod verificador;

use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{FunctionKind, FunctionRef, VariableId, VariableRef};
use dartforge_frontend::ast::{FunctionBody, MemberKind};

pub fn sanitize_symbol(name: &str) -> String {
    name.chars().map(|c| match c {
        '+' => "_plus_".to_string(),
        '-' => "_minus_".to_string(),
        '*' => "_mul_".to_string(),
        '/' => "_div_".to_string(),
        '%' => "_mod_".to_string(),
        '~' => "_tilde_".to_string(),
        '<' => "_lt_".to_string(),
        '>' => "_gt_".to_string(),
        '=' => "_eq_".to_string(),
        '!' => "_excl_".to_string(),
        '[' => "_lbr_".to_string(),
        ']' => "_rbr_".to_string(),
        '&' => "_amp_".to_string(),
        '|' => "_pipe_".to_string(),
        '^' => "_caret_".to_string(),
        c if c.is_ascii_alphanumeric() || c == '_' => c.to_string(),
        _ => format!("_x{:02x}_", c as u32),
    }).collect()
}

/// A função é do programa do usuário (não do SDK)?
///
/// Com a seção `vm` carregada de verdade, `Program::functions` tem o
/// `dart:core` inteiro. O backend nativo não compila o SDK a partir da fonte
/// (ainda — `docs/NATIVO-PLANO.md` §4, item 7): o runtime em Rust implementa
/// o que o corpus usa. Então só as funções do usuário viram símbolo, e uma
/// chamada a uma função do SDK que não tem implementação no runtime é
/// diagnóstico (N1), não `call` para um símbolo que não existe.
pub fn funcao_do_usuario(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    !ctx.program.library(f.library).is_sdk
}

/// A função é construtor generativo (inclusive o sintético): não devolve
/// nada e recebe `this` já alocado (N5).
pub fn construtor_generativo(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    match f.node {
        FunctionRef::Constructor { .. } => !f.factory,
        FunctionRef::None => f.kind == FunctionKind::SyntheticConstructor,
        FunctionRef::Function { .. } => false,
    }
}

/// O dono de um membro no símbolo: a classe, a extensão (`ext:<nome>`) ou
/// vazio para o topo.
fn dono_no_simbolo(ctx: &Context, class: Option<dartforge_elements::model::ClassId>, ext: Option<dartforge_elements::model::ExtensionId>) -> String {
    if let Some(c) = class {
        return ctx.symbol_name(ctx.program.classes[c.0 as usize].name).to_string();
    }
    if let Some(e) = ext {
        let x = ctx.program.extension(e);
        return match x.name {
            Some(n) => format!("ext:{}", ctx.symbol_name(n)),
            None => format!("ext:@{}", x.decl.decl.0),
        };
    }
    String::new()
}

/// Símbolo estável de uma função do programa (P2, docs/NATIVO-PLANO.md
/// §7.4): `df.<biblioteca>.<dono>.<membro>`, derivado do **caminho** da
/// declaração — nunca de índices internos do front-end —, com cada parte
/// escapada (`context::escapar`). O membro é o nome; o setter termina em
/// `=`; o construtor é `new` ou `new:<nome>`. `main` da biblioteca de
/// entrada é `dart_main`. Inserir ou reordenar declarações não muda o
/// símbolo de nenhuma outra: é a chave do cache de objeto por biblioteca e da
/// recarga (hot reload R1).
pub fn simbolo_de(ctx: &Context, fid: usize) -> String {
    let f = &ctx.program.functions[fid];
    let nome = ctx.symbol_name(f.name);
    if f.class.is_none() && f.extension.is_none() && nome == "main" && Some(f.library) == ctx.entry_lib {
        return "dart_main".to_string();
    }
    let membro = match f.kind {
        FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
            if nome.is_empty() { "new".to_string() } else { format!("new:{nome}") }
        }
        FunctionKind::Setter if !nome.ends_with('=') => format!("{nome}="),
        _ => nome.to_string(),
    };
    format!(
        "df.{}.{}.{}",
        crate::context::escapar(&ctx.nome_da_biblioteca(f.library)),
        crate::context::escapar(&dono_no_simbolo(ctx, f.class, f.extension)),
        crate::context::escapar(&membro)
    )
}

/// Variável de topo ou campo estático: mora num global do módulo (N6).
pub fn e_global(ctx: &Context, vid: VariableId) -> bool {
    let v = &ctx.program.variables[vid.0 as usize];
    (v.class.is_none() || v.static_)
        && v.extension.is_none()
        && matches!(
            v.node,
            VariableRef::TopLevel { .. } | VariableRef::Field { .. } | VariableRef::EnumConstant { .. }
        )
}

/// Caminho estável de uma variável de topo ou campo estático (P2).
fn caminho_da_variavel(ctx: &Context, vid: VariableId) -> String {
    let v = &ctx.program.variables[vid.0 as usize];
    format!(
        "{}.{}.{}",
        crate::context::escapar(&ctx.nome_da_biblioteca(v.library)),
        crate::context::escapar(&dono_no_simbolo(ctx, v.class, v.extension)),
        crate::context::escapar(ctx.symbol_name(v.name))
    )
}

/// Getter preguiçoso de um global: `df.<caminho>`.
pub fn simbolo_global(ctx: &Context, vid: VariableId) -> String {
    format!("df.{}", caminho_da_variavel(ctx, vid))
}

/// O valor de um global: `dfg.<caminho>` (a bandeira é `<valor>$ok`).
pub fn simbolo_valor_global(ctx: &Context, vid: VariableId) -> String {
    format!("dfg.{}", caminho_da_variavel(ctx, vid))
}

pub fn lower_program(ctx: &Context) -> Module {
    let mut module = Module::new();

    // 1. Registra classes conhecidas
    // Classe Object padrão como id 0
    module.classes.push(ClassDef {
        id: 0,
        name: "Object".to_string(),
        field_count: 0,
        vtable: Vec::new(),
        to_string_symbol: None,
    });

    // Classes e interfaces de erro da biblioteca padrão
    module.classes.push(ClassDef { id: 1000, name: "Exception".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1001, name: "FormatException".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1002, name: "StateError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1003, name: "ArgumentError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1004, name: "RangeError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1005, name: "UnsupportedError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1006, name: "StackTrace".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1007, name: "Error".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1008, name: "UnimplementedError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1009, name: "AssertionError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1010, name: "ConcurrentModificationError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1011, name: "TypeError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1012, name: "NoSuchMethodError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });

    module.subtyping_edges.push((1000, 0)); // Exception <: Object
    module.subtyping_edges.push((1001, 1000)); // FormatException <: Exception
    module.subtyping_edges.push((1001, 0)); // FormatException <: Object
    module.subtyping_edges.push((1007, 0)); // Error <: Object
    module.subtyping_edges.push((1002, 1007)); // StateError <: Error
    module.subtyping_edges.push((1002, 0));
    module.subtyping_edges.push((1003, 1007)); // ArgumentError <: Error
    module.subtyping_edges.push((1003, 0));
    module.subtyping_edges.push((1004, 1003)); // RangeError <: ArgumentError
    module.subtyping_edges.push((1004, 1007));
    module.subtyping_edges.push((1004, 0));
    module.subtyping_edges.push((1005, 1007)); // UnsupportedError <: Error
    module.subtyping_edges.push((1005, 0));
    module.subtyping_edges.push((1008, 1005)); // UnimplementedError <: UnsupportedError
    module.subtyping_edges.push((1008, 1007)); // UnimplementedError <: Error
    module.subtyping_edges.push((1008, 0));
    module.subtyping_edges.push((1009, 1007)); // AssertionError <: Error
    module.subtyping_edges.push((1009, 0));
    module.subtyping_edges.push((1010, 1007)); // ConcurrentModificationError <: Error
    module.subtyping_edges.push((1010, 0));
    module.subtyping_edges.push((1011, 1007)); // TypeError <: Error
    module.subtyping_edges.push((1011, 0));
    module.subtyping_edges.push((1012, 1007)); // NoSuchMethodError <: Error
    module.subtyping_edges.push((1012, 0));
    module.subtyping_edges.push((1006, 0)); // StackTrace <: Object

    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        // Classes do SDK não viram objetos do nosso heap (o runtime tem as
        // suas próprias representações); só as do usuário são registradas.
        if ctx.program.library(class.library).is_sdk {
            continue;
        }
        let name = ctx.symbol_name(class.name).to_string();
        // Id estável (P2): a ordem do caminho da classe, não a de carga.
        let Some(class_id) = ctx.id_de_classe(dartforge_elements::model::ClassId(c_idx as u32)) else {
            continue;
        };
        module.classes.push(ClassDef {
            id: class_id,
            name,
            field_count: membros::layout(ctx, dartforge_elements::model::ClassId(c_idx as u32)).len()
                + enums::base_do_layout(ctx, dartforge_elements::model::ClassId(c_idx as u32)),
            vtable: Vec::new(),
            to_string_symbol: None,
        });

        // Registra arestas de subtipagem
        module.subtyping_edges.push((class_id, 0)); // Todo tipo nominal herda de Object
        if let Some(sup) = class.supertype_class.and_then(|s| ctx.id_de_classe(s)) {
            module.subtyping_edges.push((class_id, sup));
        }
        let mut nomes_de_supertipo: Vec<&str> = Vec::new();
        if let Some(sup) = class.supertype_class {
            nomes_de_supertipo.push(ctx.symbol_name(ctx.program.classes[sup.0 as usize].name));
        }
        for mix in &class.mixin_classes {
            if let Some(m) = ctx.id_de_classe(*mix) {
                module.subtyping_edges.push((class_id, m));
            }
        }
        for iface in &class.interface_classes {
            if let Some(i) = ctx.id_de_classe(*iface) {
                module.subtyping_edges.push((class_id, i));
            }
            nomes_de_supertipo.push(ctx.symbol_name(ctx.program.classes[iface.0 as usize].name));
        }
        for sup_name in nomes_de_supertipo {
            let builtin = match sup_name {
                "Exception" => Some(1000),
                "FormatException" => Some(1001),
                "StateError" => Some(1002),
                "ArgumentError" => Some(1003),
                "RangeError" => Some(1004),
                "UnsupportedError" => Some(1005),
                "StackTrace" => Some(1006),
                "Error" => Some(1007),
                "UnimplementedError" => Some(1008),
                "AssertionError" => Some(1009),
                "ConcurrentModificationError" => Some(1010),
                "TypeError" => Some(1011),
                "NoSuchMethodError" => Some(1012),
                _ => None,
            };
            if let Some(b) = builtin {
                module.subtyping_edges.push((class_id, b));
            }
        }
    }

    // 2. Funções do usuário
    for (f_idx, func_elem) in ctx.program.functions.iter().enumerate() {
        if !funcao_do_usuario(ctx, f_idx) {
            continue;
        }

        let name = ctx.symbol_name(func_elem.name);
        let symbol = simbolo_de(ctx, f_idx);

        match func_elem.node {
            FunctionRef::Function { unit, function } => {
                let ast = &ctx.program.unit(unit).ast;
                let ast_func = ast.function(function);
                if matches!(ast_func.body, FunctionBody::Empty | FunctionBody::Native(_)) {
                    // Abstrato ou externo: não há corpo a compilar.
                    continue;
                }

                let is_main = symbol == "dart_main";
                if is_main {
                    module.entry_symbol = Some(symbol.clone());
                }

                if let Some(c_id) = func_elem.class
                    && name == "toString"
                    && !func_elem.static_
                    && let Some(id) = ctx.id_de_classe(c_id)
                    && let Some(c) = module.classes.iter_mut().find(|c| c.id == id)
                {
                    c.to_string_symbol = Some(symbol.clone());
                }

                let ret_ty = if is_main {
                    Type::Void
                } else if let Some(fdata) = ctx.outline.functions.get(f_idx) {
                    ctx.to_hir_type(fdata.return_type)
                } else {
                    Type::Void
                };

                let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret_ty);
                builder.preparar_capturas(
                    ast,
                    captura::Raiz {
                        parametros: ast_func.parameters.as_deref().unwrap_or(&[]),
                        corpo: Some(&ast_func.body),
                        inicializadores: &[],
                    },
                );
                let is_instance_member = func_elem.class.is_some() && !func_elem.static_;
                if let Some(e) = func_elem.extension.filter(|_| !func_elem.static_) {
                    // Membro de instância de extensão (P4): o receptor é o
                    // primeiro parâmetro, na representação do tipo `on`.
                    let r = builder.repr_do_receptor_de_extensao(e);
                    let this = builder.add_param("this".to_string(), r);
                    builder.this_param = Some(Operand::Val(this));
                    builder.declarar_parametros(f_idx, false);
                    let on = ctx.outline.extensions[e.0 as usize].on;
                    if let dartforge_types::table::Type::Interface { class, .. } = ctx.table.get(on)
                        && !ctx.program.library(ctx.program.classes[class.0 as usize].library).is_sdk
                    {
                        builder.enclosing_class = Some(*class);
                    }
                } else {
                    builder.declarar_parametros(f_idx, is_instance_member);
                }
                if is_instance_member {
                    builder.enclosing_class = func_elem.class;
                }

                match &ast_func.body {
                    FunctionBody::Block(stmt_id) => {
                        builder.lower_stmt(ast, *stmt_id);
                    }
                    FunctionBody::Expression(expr_id) => {
                        let ret_op = builder.lower_expr(ast, *expr_id);
                        builder.terminate(Terminator::Return(Some(ret_op)));
                    }
                    _ => {}
                }
                builder.finalizar(&mut module);
            }
            FunctionRef::Constructor { unit, member } => {
                let ast = &ctx.program.unit(unit).ast;
                let MemberKind::Constructor(ctor) = &ast.member(member).kind else {
                    continue;
                };
                let Some(cid) = func_elem.class else { continue };
                if func_elem.factory {
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), Type::Ref);
                    builder.preparar_capturas(
                        ast,
                        captura::Raiz {
                            parametros: &ctor.parameters,
                            corpo: Some(&ctor.body),
                            inicializadores: &ctor.initializers,
                        },
                    );
                    builder.declarar_parametros(f_idx, false);
                    if let Some(r) = &ctor.redirect {
                        // `factory C(…) = D.nome;` (P4): os parâmetros passam
                        // como estão (posicionais em ordem, nomeados pelo
                        // nome) para o construtor alvo.
                        let span = ast.member(member).span;
                        let alvo = builder.construtor_do_tipo(ast.ty(r.ty), r.constructor.map(|n| n.sym));
                        match alvo {
                            Some(t) => {
                                let mut avaliados = Vec::new();
                                for (i, p) in ctx.outline.functions[f_idx].parameters.iter().enumerate() {
                                    let Some(sym) = p.name else { continue };
                                    let v = builder.ler_local_por_nome(sym).unwrap_or(Operand::Constant(Constant::Null));
                                    let nome = (ctor.parameters.get(i).map(|q| q.kind)
                                        == Some(dartforge_frontend::ast::ParameterKind::Named))
                                    .then_some(sym);
                                    avaliados.push((nome, v));
                                }
                                let r = builder.instanciar_avaliados(t, &avaliados, span);
                                builder.terminate(Terminator::Return(Some(r)));
                            }
                            None => {
                                builder.nao_suportado("factory redirecionadora", span);
                                builder.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
                            }
                        }
                    } else {
                        match &ctor.body {
                            FunctionBody::Block(stmt_id) => builder.lower_stmt(ast, *stmt_id),
                            FunctionBody::Expression(expr_id) => {
                                let r = builder.lower_expr(ast, *expr_id);
                                builder.terminate(Terminator::Return(Some(r)));
                            }
                            _ => {}
                        }
                    }
                    builder.finalizar(&mut module);
                } else {
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), Type::Void);
                    builder.preparar_capturas(
                        ast,
                        captura::Raiz {
                            parametros: &ctor.parameters,
                            corpo: Some(&ctor.body),
                            inicializadores: &ctor.initializers,
                        },
                    );
                    builder.declarar_parametros(f_idx, true);
                    builder.enclosing_class = Some(cid);
                    builder.lower_construtor(ast, cid, ctor, ast.member(member).span);
                    builder.finalizar(&mut module);
                }
            }
            FunctionRef::None => {
                // Construtor padrão sintético (`class A { int x = 1; }`):
                // inicializadores de campo e `super()` implícito.
                if func_elem.kind != FunctionKind::SyntheticConstructor {
                    continue;
                }
                let Some(cid) = func_elem.class else { continue };
                let Some(decl) = ctx.program.classes[cid.0 as usize].decl else { continue };
                let mut builder = fn_builder::FnBuilder::new(ctx, decl.unit, symbol, name.to_string(), Type::Void);
                builder.declarar_parametros(f_idx, true);
                builder.enclosing_class = Some(cid);
                let ast = &ctx.program.unit(decl.unit).ast;
                builder.lower_construtor_sintetico(ast, cid);
                builder.finalizar(&mut module);
            }
        }
    }

    // 3. Variáveis de topo e campos estáticos do usuário: um getter
    // preguiçoso por global (N6).
    for (v_idx, v) in ctx.program.variables.iter().enumerate() {
        let vid = VariableId(v_idx as u32);
        if ctx.program.library(v.library).is_sdk || !e_global(ctx, vid) {
            continue;
        }
        let unit = match v.node {
            VariableRef::TopLevel { unit, .. }
            | VariableRef::Field { unit, .. }
            | VariableRef::EnumConstant { unit, .. } => unit,
            _ => continue,
        };
        if matches!(v.node, VariableRef::EnumConstant { .. }) {
            // Valor de enum (P3): objeto canônico criado na primeira leitura.
            module.globais.push((vid.0, Type::Ref, simbolo_valor_global(ctx, vid)));
            let mut builder = fn_builder::FnBuilder::new(
                ctx,
                unit,
                simbolo_global(ctx, vid),
                ctx.symbol_name(v.name).to_string(),
                Type::Ref,
            );
            builder.lower_valor_de_enum(vid);
            builder.finalizar(&mut module);
            continue;
        }
        let ty = membros::tipo_da_variavel(ctx, vid);
        let repr = ctx.to_hir_type(ty);
        let repr = if repr == Type::Void { Type::Ref } else { repr };
        module.globais.push((vid.0, repr, simbolo_valor_global(ctx, vid)));
        let mut builder = fn_builder::FnBuilder::new(
            ctx,
            unit,
            simbolo_global(ctx, vid),
            ctx.symbol_name(v.name).to_string(),
            repr,
        );
        builder.lower_getter_global(vid, repr);
        builder.finalizar(&mut module);
    }

    // As entradas de tear-off são geradas por quem as usa; duas funções que
    // tiram o mesmo tear-off geram a mesma entrada. Fica a primeira.
    let mut vistos = std::collections::HashSet::new();
    module.functions.retain(|f| vistos.insert(f.symbol.clone()));

    // E3: o verificador roda sobre o módulo pronto; problema aqui é bug do
    // compilador, e o módulo não é emitido.
    if module.erros.is_empty() {
        let problemas = verificador::verificar(&module);
        module.erros.extend(problemas);
    }

    // Formas de record com campo nomeado (P3, `registros.rs`): uma "classe"
    // por forma, com `toString()` gerado, e a igualdade estrutural.
    for k in 0..ctx.formas_de_record.len() {
        let id = crate::context::ID_BASE_DE_FORMA + k as u32;
        let (npos, nomes) = &ctx.formas_de_record[k];
        let simbolo = format!("df.$registro.{k}.toString");
        module.classes.push(ClassDef {
            id,
            name: "Record".to_string(),
            field_count: npos + nomes.len(),
            vtable: Vec::new(),
            to_string_symbol: Some(simbolo.clone()),
        });
        module.subtyping_edges.push((id, 0));
        let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
            continue;
        };
        let mut builder = fn_builder::FnBuilder::new(ctx, u, simbolo, "toString".to_string(), Type::Ref);
        builder.lower_to_string_de_forma(k);
        builder.finalizar(&mut module);
    }
    if !ctx.formas_de_record.is_empty()
        && let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied())
    {
        let mut builder = fn_builder::FnBuilder::new(
            ctx,
            u,
            registros::SIMBOLO_IGUAL.to_string(),
            "==".to_string(),
            Type::I1,
        );
        builder.lower_igualdade_de_registros();
        builder.finalizar(&mut module);
    }

    // `toString()` padrão dos enums do programa (`Enum.valor`), quando o
    // enum não declara o seu.
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        let cid = dartforge_elements::model::ClassId(c_idx as u32);
        if !enums::e_enum(ctx, cid) {
            continue;
        }
        let Some(id) = ctx.id_de_classe(cid) else { continue };
        if module.classes.iter().any(|c| c.id == id && c.to_string_symbol.is_some()) {
            continue;
        }
        let Some(decl) = class.decl else { continue };
        let simbolo = format!(
            "df.{}.{}.toString",
            crate::context::escapar(&ctx.nome_da_biblioteca(class.library)),
            crate::context::escapar(ctx.symbol_name(class.name))
        );
        let mut builder = fn_builder::FnBuilder::new(ctx, decl.unit, simbolo.clone(), "toString".to_string(), Type::Ref);
        builder.lower_to_string_de_enum(cid);
        builder.finalizar(&mut module);
        if let Some(c) = module.classes.iter_mut().find(|c| c.id == id) {
            c.to_string_symbol = Some(simbolo);
        }
    }

    // Propaga to_string_symbol para subclasses que não o sobrescreveram
    for c_idx in 0..ctx.program.classes.len() {
        let Some(class_id) = ctx.id_de_classe(dartforge_elements::model::ClassId(c_idx as u32)) else {
            continue;
        };
        let mut curr_cid = ctx.program.classes[c_idx].supertype_class;
        while let Some(sup) = curr_cid {
            let Some(sup_class_id) = ctx.id_de_classe(sup) else {
                break;
            };
            let sup_ts = module.classes.iter().find(|c| c.id == sup_class_id).and_then(|c| c.to_string_symbol.clone());
            if let Some(ts) = sup_ts {
                if let Some(c_def) = module.classes.iter_mut().find(|c| c.id == class_id) {
                    if c_def.to_string_symbol.is_none() {
                        c_def.to_string_symbol = Some(ts);
                    }
                }
                break;
            }
            curr_cid = ctx.program.classes[sup.0 as usize].supertype_class;
        }
    }

    module
}
