//! Contadores e transação das barreiras globais da expansão experimental.
use dartforge_macros::{MacroPhase, expand};

/// Constrói uma AST independente para cada execução da expansão.
fn parse(source: &str) -> dartforge_syntax::Program<'_> {
    let tokens = dartforge_lexer::lex(source).unwrap();
    dartforge_parser::parse(&tokens, source.len()).unwrap()
}

/// Três classes geram seis métodos e somente dois construtores novos.
#[test]
fn phases_count_only_their_own_work() {
    let source = "@JsonCodable() class A{final int x;} @JsonCodable() class B{final bool x;} @JsonCodable() class C{final String x;C(this.x);}void main(){}";
    let mut program = parse(source);
    let report = expand(&mut program, source.len()).unwrap();
    assert_eq!(report.applications, 3);
    assert_eq!(report.generated_declarations, 8);
    assert_eq!(report.phases[0].phase, MacroPhase::Types);
    assert_eq!(report.phases[0].applications, 0);
    assert_eq!(report.phases[0].generated_declarations, 0);
    assert_eq!(report.phases[1].phase, MacroPhase::Declarations);
    assert_eq!(report.phases[1].applications, 3);
    assert_eq!(report.phases[1].generated_declarations, 8);
    assert_eq!(report.phases[2].phase, MacroPhase::Definitions);
    assert_eq!(report.phases[2].applications, 3);
    assert_eq!(report.phases[2].generated_declarations, 0);
    for class in &program.classes {
        assert!(class.constructor.is_some());
        assert_eq!(class.factories[0].body.len(), 1);
        assert_eq!(class.methods[0].body.len(), 1);
    }
}

/// Falha na validação da última classe preserva todas as anteriores.
#[test]
fn all_targets_validate_before_declarations_commit() {
    let source = "@JsonCodable() class A{final int x;} @JsonCodable() class B{final bool x;} @JsonCodable() class Bad{int x=1;}void main(){}";
    let mut program = parse(source);
    let original = format!("{program:?}");
    assert!(expand(&mut program, source.len()).is_err());
    assert_eq!(format!("{program:?}"), original);
}

/// Falha na reserva de spans também descarta a cópia transacional inteira.
#[test]
fn span_exhaustion_preserves_original_ast() {
    let source = "@JsonCodable() class A{final int x;}void main(){}";
    let mut program = parse(source);
    let original = format!("{program:?}");
    assert!(expand(&mut program, usize::MAX - 2).is_err());
    assert_eq!(format!("{program:?}"), original);
}
