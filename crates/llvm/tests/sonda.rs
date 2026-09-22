//! Sonda temporária 4 (será removida).
use dartforge_diagnostics::Diagnostic;

fn check(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let program = dartforge_parser::parse(&tokens, source.len())?;
    let resolution = dartforge_semantic::analyze(&program)?;
    dartforge_llvm::emit(&dartforge_hir::lower_resolved(program, resolution))
}

#[test]
fn sonda4() {
    for (nome, fonte) in [
        ("static-fn", "class C { static int Function(int) f = add1; } int add1(int x) => x + 1; void main(){print(C.f(2));}"),
        ("field-init-param", "class C { var y = n; C(int n); } void main(){print(C(1).y);}"),
        ("formal-initlist", "class C { int x; int y; C(this.x) : y = x; } void main(){print(C(3).y);}"),
        ("formal-closure", "class C { int x; int Function() f; C(this.x) : f = (() => 1); } void main(){print(C(3).f());}"),
        ("getter-closure", "class C { int n; C(this.n); int Function() get make => () => n; } void main(){print(C(4).make());}"),
        ("method-param", "class C { int run(int Function(int) f) => f(10); } void main(){print(C().run((int x) => x + 1));}"),
        ("two-captures", "void main(){var a = 1; var b = 2; var f = () => a + b; print(f());}"),
        ("assign-later", "void main(){int Function(int) f; f = (int x) => x * 2; print(f(21));}"),
        ("nested", "void main(){var x = 1; var f = () { var g = () => x + 1; return g(); }; print(f());}"),
        ("nested-share", "void main(){var x = 0; var f = () { var inc = () { x = x + 1; return x; }; inc(); inc(); return x; }; print(f()); print(x);}"),
        ("late-capture", "void main(){var x = 1; var f = () => x; x = 2; print(f());}"),
        ("if-cap", "void main(){var x = 1; int Function() f; if (x > 0) { var y = 10; f = () => x + y; } else { f = () => x; } print(f());}"),
        ("for-shadow", "void main(){var r = 0; for (var i = 0; i < 2; i++) { var f = () => i; r = r + f(); } print(r);}"),
        ("string-list", "void main(){var l = ['a', 'b']; print(l); print(l[0] + l[1]);}"),
        ("empty-list", "void main(){var l = <int>[]; l.add(1); l.add(2); print(l); print(l.length);}"),
        ("idx-assign", "void main(){var l = [1, 2]; l[0] = 9; print(l);}"),
        ("isEmpty", "void main(){var l = <int>[]; print(l.isEmpty); print(l.isNotEmpty); l.add(1); print(l.isEmpty);}"),
    ] {
        match check(fonte) {
            Ok(_) => println!("{nome}: EMITIU"),
            Err(e) => println!("{nome}: ERRO: {}", e.message),
        }
    }
}
