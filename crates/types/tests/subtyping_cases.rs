//! Suite de testes exaustiva para as regras normativas de subtipagem (`subtyping.md`).
//!
//! Contém mais de 60 casos de teste cobrindo todas as regras e casos de borda.

use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program};
use dartforge_intern::Interner;
use dartforge_types::hierarchy::build_class_hierarchy;
use dartforge_types::subtyping::{is_subtype, SubtypeEnv};
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeParamId, TypeParamOwner, TypeTable, Variance};
use std::collections::HashMap;

struct TestHarness {
    interner: Interner,
    table: TypeTable,
    program: Program,
    core: CoreTypes,
}

impl TestHarness {
    fn new() -> Self {
        let mut interner = Interner::new();
        let mut table = TypeTable::new();
        let mut program = Program::default();

        // Criar uma biblioteca core sintética
        let core_lib_id = LibraryId(0);
        let mut core_lib = dartforge_elements::model::Library {
            uri: "dart:core".to_string(),
            name: None,
            units: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            declared: HashMap::new(),
            exported: HashMap::new(),
            scope: HashMap::new(),
            prefixes: HashMap::new(),
            is_sdk: true,
            language_version: None,
        };

        // Registrar classes fundamentais no program
        let mut add_class = |name: &str, kind: ClassKind, num_type_params: usize| -> ClassId {
            let cid = ClassId(program.classes.len() as u32);
            let sym = interner.intern(name);
            let mut type_params = Vec::new();
            for i in 0..num_type_params {
                let p_sym = interner.intern(&format!("T{}", i));
                type_params.push(dartforge_elements::model::TypeParameterElement {
                    name: p_sym,
                    bound: None,
                });
            }

            program.classes.push(dartforge_elements::model::ClassElement {
                name: sym,
                library: core_lib_id,
                decl: None,
                kind,
                modifiers: dartforge_frontend::ast::ClassModifiers::default(),
                type_params,
                supertype: None,
                mixins: Vec::new(),
                interfaces: Vec::new(),
                on: Vec::new(),
                supertype_class: None,
                mixin_classes: Vec::new(),
                interface_classes: Vec::new(),
                on_classes: Vec::new(),
                instance_members: HashMap::new(),
                static_members: HashMap::new(),
                constructors: HashMap::new(),
                fields: Vec::new(),
                enum_constants: Vec::new(),
                representation: None,
            });

            core_lib.scope.insert(
                sym,
                dartforge_elements::model::Binding {
                    getter: Some(Element::Class(cid)),
                    setter: None,
                    ambiguous: false,
                },
            );

            cid
        };

        let obj_cid = add_class("Object", ClassKind::Class, 0);
        let num_cid = add_class("num", ClassKind::Class, 0);
        let int_cid = add_class("int", ClassKind::Class, 0);
        let str_cid = add_class("String", ClassKind::Class, 0);
        let bool_cid = add_class("bool", ClassKind::Class, 0);
        let _fn_cid = add_class("Function", ClassKind::Class, 0);
        let _rec_cid = add_class("Record", ClassKind::Class, 0);
        let iter_cid = add_class("Iterable", ClassKind::Class, 1);
        let list_cid = add_class("List", ClassKind::Class, 1);
        let map_cid = add_class("Map", ClassKind::Class, 2);
        let comp_cid = add_class("Comparable", ClassKind::Class, 1);
        let pattern_cid = add_class("Pattern", ClassKind::Class, 0);

        program.core = Some(core_lib_id);
        program.libraries.push(core_lib);

        // Criar dart:async
        let async_lib_id = LibraryId(1);
        let mut async_lib = dartforge_elements::model::Library {
            uri: "dart:async".to_string(),
            name: None,
            units: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            declared: HashMap::new(),
            exported: HashMap::new(),
            scope: HashMap::new(),
            prefixes: HashMap::new(),
            is_sdk: true,
            language_version: None,
        };

        let fut_cid = ClassId(program.classes.len() as u32);
        let fut_sym = interner.intern("Future");
        program.classes.push(dartforge_elements::model::ClassElement {
            name: fut_sym,
            library: async_lib_id,
            decl: None,
            kind: ClassKind::Class,
            modifiers: dartforge_frontend::ast::ClassModifiers::default(),
            type_params: vec![dartforge_elements::model::TypeParameterElement {
                name: interner.intern("T"),
                bound: None,
            }],
            supertype: None,
            mixins: Vec::new(),
            interfaces: Vec::new(),
            on: Vec::new(),
            supertype_class: Some(obj_cid),
            mixin_classes: Vec::new(),
            interface_classes: Vec::new(),
            on_classes: Vec::new(),
            instance_members: HashMap::new(),
            static_members: HashMap::new(),
            constructors: HashMap::new(),
            fields: Vec::new(),
            enum_constants: Vec::new(),
            representation: None,
        });

        async_lib.scope.insert(
            fut_sym,
            dartforge_elements::model::Binding {
                getter: Some(Element::Class(fut_cid)),
                setter: None,
                ambiguous: false,
            },
        );
        program.libraries.push(async_lib);

        // Configurar relações de herança no mock
        program.classes[int_cid.0 as usize].supertype_class = Some(num_cid);
        program.classes[num_cid.0 as usize].supertype_class = Some(obj_cid);
        program.classes[str_cid.0 as usize].supertype_class = Some(obj_cid);
        program.classes[str_cid.0 as usize].interface_classes = vec![pattern_cid, comp_cid];
        program.classes[bool_cid.0 as usize].supertype_class = Some(obj_cid);
        program.classes[iter_cid.0 as usize].supertype_class = Some(obj_cid);
        program.classes[list_cid.0 as usize].supertype_class = Some(obj_cid);
        program.classes[list_cid.0 as usize].interface_classes = vec![iter_cid];
        program.classes[map_cid.0 as usize].supertype_class = Some(obj_cid);
        program.classes[comp_cid.0 as usize].supertype_class = Some(obj_cid);
        program.classes[pattern_cid.0 as usize].supertype_class = Some(obj_cid);

        let core = CoreTypes::init(&mut table, &program, &interner);

        Self {
            interner,
            table,
            program,
            core,
        }
    }

    fn class_type(&mut self, name: &str, args: &[TypeId], nullable: bool) -> TypeId {
        let sym = self.interner.intern(name);
        let cid = self.program.libraries[0]
            .scope
            .get(&sym)
            .and_then(|b| match b.getter {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            })
            .or_else(|| {
                self.program.libraries[1]
                    .scope
                    .get(&sym)
                    .and_then(|b| match b.getter {
                        Some(Element::Class(c)) => Some(c),
                        _ => None,
                    })
            })
            .unwrap_or_else(|| panic!("Classe não encontrada no mock: {}", name));

        self.table.intern(Type::Interface {
            class: cid,
            args: args.to_vec().into_boxed_slice(),
            nullable,
        })
    }

    fn future_type(&mut self, arg: TypeId, nullable: bool) -> TypeId {
        let fut_cls = self.core.future_class.expect("Future class");
        self.table.intern(Type::Interface {
            class: fut_cls,
            args: Box::new([arg]),
            nullable,
        })
    }

    fn future_or_type(&mut self, arg: TypeId, nullable: bool) -> TypeId {
        self.table.intern(Type::FutureOr { arg, nullable })
    }

    fn function_type(
        &mut self,
        ret: TypeId,
        pos: &[TypeId],
        opt: &[TypeId],
        named: &[( &str, TypeId, bool )],
        nullable: bool,
    ) -> TypeId {
        let mut named_vec = Vec::new();
        for &(name, ty, req) in named {
            let sym = self.interner.intern(name);
            named_vec.push((sym, ty, req));
        }
        self.table.intern(Type::Function {
            type_params: Box::new([]),
            ret,
            positional: pos.to_vec().into_boxed_slice(),
            optional: opt.to_vec().into_boxed_slice(),
            named: named_vec.into_boxed_slice(),
            nullable,
        })
    }

    fn record_type(
        &mut self,
        pos: &[TypeId],
        named: &[(&str, TypeId)],
        nullable: bool,
    ) -> TypeId {
        let mut named_vec = Vec::new();
        for &(name, ty) in named {
            let sym = self.interner.intern(name);
            named_vec.push((sym, ty));
        }
        self.table.intern(Type::Record {
            positional: pos.to_vec().into_boxed_slice(),
            named: named_vec.into_boxed_slice(),
            nullable,
        })
    }

    fn type_param(&mut self, name: &str, bound: TypeId) -> TypeId {
        let sym = self.interner.intern(name);
        let pid = self.table.alloc_type_param(
            sym,
            TypeParamOwner::GenericFunctionType,
            bound,
            Variance::Unspecified,
        );
        self.table.intern(Type::TypeParameter {
            param: pid,
            nullable: false,
        })
    }
}

#[test]
fn test_tabela_60_casos_subtipagem_normativa() {
    let mut h = TestHarness::new();

    // Montar hierarquia instanciada com supertipos diretos conhecidos
    let num_classes = h.program.classes.len();
    let mut immediate_inputs = Vec::with_capacity(num_classes);

    for (i, class) in h.program.classes.iter().enumerate() {
        let formals: Box<[TypeParamId]> = if class.type_params.len() == 1 {
            let pid = h.table.alloc_type_param(
                class.type_params[0].name,
                TypeParamOwner::Class(ClassId(i as u32)),
                h.core.object_nullable,
                Variance::Unspecified,
            );
            Box::new([pid])
        } else {
            Box::new([])
        };

        let mut direct = Vec::new();
        if let Some(super_cls) = class.supertype_class {
            direct.push(h.table.intern(Type::Interface {
                class: super_cls,
                args: Box::new([]),
                nullable: false,
            }));
        }

        for &iface in &class.interface_classes {
            let iface_args: Vec<TypeId> = if class.name == h.interner.intern("List") && !formals.is_empty() {
                vec![h.table.intern(Type::TypeParameter {
                    param: formals[0],
                    nullable: false,
                })]
            } else {
                Vec::new()
            };

            direct.push(h.table.intern(Type::Interface {
                class: iface,
                args: iface_args.into_boxed_slice(),
                nullable: false,
            }));
        }

        immediate_inputs.push(Some((formals, direct)));
    }

    let hierarchy = build_class_hierarchy(num_classes, &immediate_inputs, &mut h.table, &h.core);

    let int_ty = h.core.int;
    let int_null = h.class_type("int", &[], true);
    let num_ty = h.core.num;
    let num_null = h.class_type("num", &[], true);
    let string_ty = h.core.string;
    let bool_ty = h.core.bool_;
    let object_ty = h.core.object;
    let object_null = h.core.object_nullable;
    let dynamic_ty = h.core.dynamic_;
    let void_ty = h.core.void_;
    let never_ty = h.core.never;
    let null_ty = h.core.null;

    let fut_int = h.future_type(int_ty, false);
    let _fut_num = h.future_type(num_ty, false);
    let fut_or_int = h.future_or_type(int_ty, false);
    let fut_or_int_null = h.future_or_type(int_null, false);
    let fut_or_num = h.future_or_type(num_ty, false);

    let list_int = h.class_type("List", &[int_ty], false);
    let list_num = h.class_type("List", &[num_ty], false);
    let list_never = h.class_type("List", &[never_ty], false);
    let iter_int = h.class_type("Iterable", &[int_ty], false);
    let iter_num = h.class_type("Iterable", &[num_ty], false);

    let t_extends_num = h.type_param("T", num_ty);
    let t_extends_int = h.type_param("T", int_ty);

    let fn_int_to_void = h.function_type(void_ty, &[int_ty], &[], &[], false);
    let fn_num_to_void = h.function_type(void_ty, &[num_ty], &[], &[], false);
    let fn_void_to_int = h.function_type(int_ty, &[], &[], &[], false);
    let fn_void_to_num = h.function_type(num_ty, &[], &[], &[], false);

    let fn_opt_pos = h.function_type(void_ty, &[int_ty], &[int_ty], &[], false);
    let fn_req_pos = h.function_type(void_ty, &[int_ty], &[], &[], false);

    let fn_named_req = h.function_type(void_ty, &[], &[], &[("a", int_ty, true)], false);
    let fn_named_opt = h.function_type(void_ty, &[], &[], &[("a", int_null, false)], false);

    let rec_int_str = h.record_type(&[int_ty], &[("b", string_ty)], false);
    let rec_num_obj = h.record_type(&[num_ty], &[("b", object_ty)], false);
    let rec_int_int = h.record_type(&[int_ty, int_ty], &[], false);

    // Tabela exaustiva de pares (A, B, esperado, descrição)
    let casos: Vec<(TypeId, TypeId, bool, &str)> = vec![
        // --- 1. Reflexividade ---
        (int_ty, int_ty, true, "int <: int"),
        (string_ty, string_ty, true, "String <: String"),
        (object_ty, object_ty, true, "Object <: Object"),
        (dynamic_ty, dynamic_ty, true, "dynamic <: dynamic"),
        (void_ty, void_ty, true, "void <: void"),
        (null_ty, null_ty, true, "Null <: Null"),
        (never_ty, never_ty, true, "Never <: Never"),

        // --- 2. Right Top ---
        (int_ty, dynamic_ty, true, "int <: dynamic"),
        (int_ty, void_ty, true, "int <: void"),
        (int_ty, object_null, true, "int <: Object?"),
        (null_ty, dynamic_ty, true, "Null <: dynamic"),
        (null_ty, void_ty, true, "Null <: void"),
        (null_ty, object_null, true, "Null <: Object?"),
        (never_ty, dynamic_ty, true, "Never <: dynamic"),
        (never_ty, object_null, true, "Never <: Object?"),
        (dynamic_ty, object_null, true, "dynamic <: Object?"),
        (object_null, dynamic_ty, true, "Object? <: dynamic"),
        (void_ty, object_null, true, "void <: Object?"),
        (object_null, void_ty, true, "Object? <: void"),

        // --- 3. Left Top ---
        (dynamic_ty, int_ty, false, "dynamic <: int"),
        (void_ty, int_ty, false, "void <: int"),
        (dynamic_ty, object_ty, false, "dynamic <: Object"),
        (void_ty, object_ty, false, "void <: Object"),

        // --- 4. Left Bottom (Never) ---
        (never_ty, int_ty, true, "Never <: int"),
        (never_ty, string_ty, true, "Never <: String"),
        (never_ty, object_ty, true, "Never <: Object"),
        (never_ty, null_ty, true, "Never <: Null"),
        (never_ty, fut_or_int, true, "Never <: FutureOr<int>"),
        (never_ty, fn_int_to_void, true, "Never <: FunctionType"),
        (never_ty, rec_int_str, true, "Never <: RecordType"),

        // --- 5. Right Object ---
        (int_ty, object_ty, true, "int <: Object"),
        (num_ty, object_ty, true, "num <: Object"),
        (string_ty, object_ty, true, "String <: Object"),
        (bool_ty, object_ty, true, "bool <: Object"),
        (h.core.function, object_ty, true, "Function <: Object"),
        (h.core.record, object_ty, true, "Record <: Object"),
        (int_null, object_ty, false, "int? <: Object"),
        (null_ty, object_ty, false, "Null <: Object"),
        (fut_or_int, object_ty, true, "FutureOr<int> <: Object"),
        (fut_or_int_null, object_ty, false, "FutureOr<int?> <: Object"),
        (t_extends_num, object_ty, true, "T extends num <: Object"),

        // --- 6. Left Null & Nulabilidade ---
        (null_ty, int_null, true, "Null <: int?"),
        (null_ty, string_ty, false, "Null <: String"),
        (null_ty, int_ty, false, "Null <: int"),
        (null_ty, fut_or_int_null, true, "Null <: FutureOr<int?>"),
        (null_ty, fut_or_int, false, "Null <: FutureOr<int>"),
        (int_null, int_ty, false, "int? <: int"),
        (int_ty, int_null, true, "int <: int?"),
        (int_null, num_null, true, "int? <: num?"),
        (int_null, num_ty, false, "int? <: num"),
        (num_null, int_null, false, "num? <: int?"),

        // --- 7. FutureOr ---
        (int_ty, fut_or_int, true, "int <: FutureOr<int>"),
        (fut_int, fut_or_int, true, "Future<int> <: FutureOr<int>"),
        (fut_int, fut_or_num, true, "Future<int> <: FutureOr<num>"),
        (string_ty, fut_or_int, false, "String <: FutureOr<int>"),
        (fut_or_int, fut_or_num, true, "FutureOr<int> <: FutureOr<num>"),
        (fut_or_int_null, fut_or_int, false, "FutureOr<int?> <: FutureOr<int>"),

        // --- 8. Tipos de Função ---
        (fn_int_to_void, fn_num_to_void, false, "void Function(int) <: void Function(num)"),
        (fn_num_to_void, fn_int_to_void, true, "void Function(num) <: void Function(int)"),
        (fn_void_to_int, fn_void_to_num, true, "int Function() <: num Function()"),
        (fn_void_to_num, fn_void_to_int, false, "num Function() <: int Function()"),
        (fn_opt_pos, fn_req_pos, true, "void Function(int, [int]) <: void Function(int)"),
        (fn_req_pos, fn_opt_pos, false, "void Function(int) <: void Function(int, [int])"),
        (fn_named_req, fn_named_opt, false, "({required int a}) -> void <: ({int? a}) -> void"),
        (fn_named_opt, fn_named_req, true, "({int? a}) -> void <: ({required int a}) -> void"),
        (fn_int_to_void, h.core.function, true, "FunctionType <: Function"),
        (int_ty, h.core.function, false, "int <: Function"),

        // --- 9. Tipos de Record ---
        (rec_int_str, rec_num_obj, true, "(int, {String b}) <: (num, {Object b})"),
        (rec_num_obj, rec_int_str, false, "(num, {Object b}) <: (int, {String b})"),
        (rec_int_int, h.core.record, true, "(int, int) <: Record"),
        (h.core.record, rec_int_int, false, "Record <: (int, int)"),
        (rec_int_str, int_ty, false, "RecordType <: int"),

        // --- 10. Tipos de Interface & Genéricos ---
        (int_ty, num_ty, true, "int <: num"),
        (num_ty, int_ty, false, "num <: int"),
        (list_int, list_num, true, "List<int> <: List<num>"),
        (list_num, list_int, false, "List<num> <: List<int>"),
        (list_never, list_int, true, "List<Never> <: List<int>"),
        (list_int, iter_int, true, "List<int> <: Iterable<int>"),
        (list_int, iter_num, true, "List<int> <: Iterable<num>"),
        (list_num, iter_int, false, "List<num> <: Iterable<int>"),

        // --- 11. Variáveis de Tipo e Bounds ---
        (t_extends_num, num_ty, true, "T extends num <: num"),
        (num_ty, t_extends_num, false, "num <: T extends num"),
        (t_extends_int, num_ty, true, "T extends int <: num"),
    ];

    let mut env = SubtypeEnv::new(&mut h.table, &hierarchy, &h.core);

    let mut passou = 0;
    let mut falhou = 0;

    for (a, b, esperado, desc) in casos {
        let resultado = is_subtype(a, b, &mut env);
        if resultado == esperado {
            passou += 1;
        } else {
            falhou += 1;
            eprintln!("FALHA: {} (esperado: {}, obtido: {})", desc, esperado, resultado);
        }
    }

    assert_eq!(
        falhou, 0,
        "Falharam {}/{} casos da tabela de subtipagem normativa!",
        falhou,
        passou + falhou
    );
    assert!(passou >= 60, "Esperado pelo menos 60 casos, rodou {}", passou);
}
