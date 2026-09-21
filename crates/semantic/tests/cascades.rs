//! Cascatas preservam receptor, escopos e segurança de null sem descartar erros mortos.
/// Analisa uma fonte completa sem executar nem gerar código.
fn check(source: &str) -> Result<dartforge_syntax::Resolution, String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::analyze(&program).map_err(|e| e.message)
}
/// Seções descartam resultados de métodos e retornam o receptor original, inclusive nullable.
#[test]
fn receivers_sections_and_context_are_typed() {
    for source in [
        "class C{int x=0;void put(int n){x=n;}}void main(){C c=C()..put(1)..x=2;print(c.x);}",
        "class C{int x=0;void put(int n){x=n;}}void main(){C? c=null;C? r=c?..put(1)..x=2;print(r==null);}",
        "void main(){List<int> xs=[]..add(1)..[0]=2;print(xs);}",
        "class C{int x=0;}class B{C child=C();}void main(){B b=B()..child=(C()..x=3);print(b.child.x);}",
        "void main(){var r=(1,name:'s')..$1..name;print(r);}",
        "void main(){List<int> xs=[1];List<Object> ys=xs;ys..add('s');}",
        "class C{int x=0;}T change<T extends C>(T c)=>c..x=1;void main(){C c=change(C());print(c.x);}",
    ] {
        check(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
}
/// Erros em seções não executadas e escritas capturadas continuam verificáveis.
#[test]
fn nullable_readonly_and_capture_errors_remain_errors() {
    for source in [
        "class C{int x=0;}void main(){C? c=null;c..x=1;}",
        "class C{int x=0;}void main(){C? c=null;c?..x='s';}",
        "class C{int x=0;}void main(){C? c=null;c?..missing();}",
        "class C{int x=0;}void main(){C? c=null;c?..x=1;print(c.x);}",
        "class C{final int x=0;}void main(){C()..x=1;}",
        "void main(){(1,2)..$1=3;}",
        "void main(){<int>[1]..[0]='s';}",
        "void main(){null?..missing();}",
        "void main(){int? x=1;var callbacks=<void Function()>[]..add((){x=null;});if(x!=null){callbacks[0]();print(x+1);}}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}

/// ASTs públicas não podem usar o receptor sintético fora de seu contexto léxico.
#[test]
fn synthetic_receiver_requires_cascade_context() {
    use dartforge_syntax::{Expr, ExprKind, Program, Statement, StatementKind};
    let span = dartforge_diagnostics::Span { start: 0, end: 1 };
    let program = Program {
        main_is_arrow: false,
        main_is_async: false,
        types: vec![],
        extensions: vec![],
        classes: vec![],
        functions: vec![],
        statements: vec![Statement {
            span,
            kind: StatementKind::Expression(Expr {
                span,
                kind: ExprKind::CascadeReceiver,
            }),
        }],
    };
    assert!(dartforge_semantic::validate(&program).is_err());
}
