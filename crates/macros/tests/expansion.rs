//! Expansão transacional e proveniência determinística de JsonCodable experimental.
use dartforge_macros::expand;
use dartforge_syntax::{ExprKind, StatementKind, Type, TypeShape};

/// Constrói AST a partir da mesma sintaxe aceita pelo pipeline público.
fn parse(source: &str) -> dartforge_syntax::Program<'_> {
    let tokens = dartforge_lexer::lex(source).unwrap();
    dartforge_parser::parse(&tokens, source.len()).unwrap()
}

/// Uma falha tardia não deixa a primeira classe nem a arena parcialmente expandidas.
#[test]
fn later_failure_preserves_entire_original_program() {
    let source =
        "@JsonCodable() class Good{final int id;} @JsonCodable() class Bad{int id=1;}void main(){}";
    let mut program = parse(source);
    let before = format!("{program:?}");
    assert!(expand(&mut program, source.len()).is_err());
    assert_eq!(format!("{program:?}"), before);
    assert!(program.classes[0].constructor.is_none());
    assert!(program.classes[0].factories.is_empty());
}

/// Spans exclusivos além da fonte voltam à anotação correta e não dependem de aleatoriedade.
#[test]
fn deterministic_unique_spans_and_provenance() {
    let source = "@JsonCodable() class A{final int id;} @JsonCodable() class B{final String? name;}void main(){}";
    let mut first = parse(source);
    let annotations = first
        .classes
        .iter()
        .map(|c| c.annotations[0].span)
        .collect::<Vec<_>>();
    let mut second = parse(source);
    let a = expand(&mut first, source.len()).unwrap();
    let b = expand(&mut second, source.len()).unwrap();
    assert_eq!(format!("{first:?}"), format!("{second:?}"));
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert_eq!(a.applications, 2);
    assert_eq!(a.generated_declarations, 6);
    let mut end = source.len();
    for origin in &a.origins {
        assert!(origin.generated.start > end);
        assert!(origin.generated.end > origin.generated.start);
        assert!(annotations.contains(&origin.annotation));
        let mapped = a.remap(dartforge_diagnostics::Diagnostic::new(
            "generated error",
            origin.generated,
        ));
        assert_eq!(mapped.span, origin.annotation);
        assert!(mapped.message.starts_with("JsonCodable: "));
        end = origin.generated.end;
    }
    assert_eq!(a.extent, end);
    assert!(
        annotations
            .iter()
            .all(|span| a.origins.iter().any(|o| o.annotation == *span))
    );
    let before = format!("{first:?}");
    let again = expand(&mut first, a.extent).unwrap();
    assert_eq!(again.applications, 0);
    assert_eq!(again.origins.capacity(), 0);
    assert_eq!(format!("{first:?}"), before);
}

/// Sem anotações, o relatório mantém vetor sem reserva e os buffers originais da AST.
#[test]
fn ordinary_path_preserves_storage_and_empty_report() {
    let source = "class A{int x=1;}void main(){print(1);}";
    let mut program = parse(source);
    let classes = program.classes.as_ptr();
    let statements = program.statements.as_ptr();
    let before = format!("{program:?}");
    let report = expand(&mut program, source.len()).unwrap();
    assert_eq!(report.applications, 0);
    assert_eq!(report.generated_declarations, 0);
    assert_eq!(report.extent, source.len());
    assert_eq!(report.origins.capacity(), 0);
    assert_eq!(program.classes.as_ptr(), classes);
    assert_eq!(program.statements.as_ptr(), statements);
    assert_eq!(format!("{program:?}"), before);
}

/// Lookup de JSON é Object?; cada argumento recebe cast ao tipo exato do campo.
#[test]
fn generated_factory_casts_and_map_types_are_explicit() {
    let source =
        "@JsonCodable() class A{final int id;final String? name;final bool active;}void main(){}";
    let mut program = parse(source);
    expand(&mut program, source.len()).unwrap();
    let class = &program.classes[0];
    let factory = &class.factories[0];
    assert_eq!(factory.return_type, Type::Class(class.id));
    let Type::Applied(map_id) = factory.parameters[0].ty else {
        panic!("tipo Map esperado")
    };
    assert_eq!(
        program.types[map_id as usize],
        TypeShape::Map {
            key: Type::String,
            value: Type::NullableObject
        }
    );
    let StatementKind::Return(Some(expr)) = &factory.body[0].kind else {
        panic!("return esperado")
    };
    let ExprKind::Construct {
        class_id,
        arguments,
    } = &expr.kind
    else {
        panic!("construtor esperado")
    };
    assert_eq!(*class_id, class.id);
    for (argument, field) in arguments.iter().zip(&class.fields) {
        let ExprKind::Cast { operand, ty } = &argument.kind else {
            panic!("cast esperado")
        };
        assert_eq!(*ty, field.ty);
        let ExprKind::Index { receiver, index } = &operand.kind else {
            panic!("lookup esperado")
        };
        assert!(matches!(receiver.kind, ExprKind::Identifier("json")));
        assert!(matches!(&index.kind,ExprKind::OwnedString(name) if name==field.name));
    }
    assert_eq!(arguments.len(), class.fields.len());
    assert_eq!(class.methods[0].return_type, factory.parameters[0].ty);
}
