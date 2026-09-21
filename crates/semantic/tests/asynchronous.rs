//! Contratos de Future, await e agendamento sem confundir corpos síncronos e async.
/// Habilita dart:async como faria o linker depois de conferir a importação.
fn check(source: &str) -> Result<dartforge_syntax::Resolution, String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::analyze_with_async_library(&program, true).map_err(|e| e.message)
}
/// Await suspende escalares e retira somente uma camada; resultados são contextualizados.
#[test]
fn async_results_and_scheduling_are_typed() {
    for source in [
        "Future<int> f() async{return 1;}Future<void> main() async{print(await f());}",
        "Future<int> f() async{return Future<int>.value(1);}void main() async{print(await f());}",
        "Future<int?> f() async{}void main() async{print(await f());}",
        "void main() async{print(await 1);print(await null);}",
        "void main() async{Future<Future<int>> f=Future<Future<int>>.value(Future<int>.value(1));Future<int> inner=await f;print(await inner);}",
        "void main() async{var f=() async=>1;print(await f());}",
        "Future<void> f() async=>1;void main() async{await f();}",
        "Future<void> f() async=>Future<int>.value(1);void main() async{await f();}",
        "Future<void> main() async=>Future<int>.value(1);",
        "void f() async=>Future<int>.value(1);void main(){f();}",
        "void main() async{Future<void> Function() f=() async=>Future<int>.value(1);await f();}",
        "void main(){scheduleMicrotask(()=>print(1));var t=Timer(Duration.zero,()=>print(2));print(t.isActive);print(t.tick);t.cancel();}",
        "void main() async{var f=Future<int>.delayed(Duration(milliseconds:1),()=>2);print(await f);}",
        "void main() async{Future<int?>? f=null;int? x=await f;print(x);}",
    ] {
        check(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
}
/// Retornos inválidos e await fora de async são erros mesmo em código não chamado.
#[test]
fn invalid_async_contexts_fail() {
    for source in [
        "void main(){await 1;}",
        "int f() async=>1;void main(){}",
        "Future<int> f() async{}void main(){}",
        "Future<void> f() async{return 1;}void main(){}",
        "Future<void> f() async{return Future<int>.value(1);}void main(){}",
        "void main(){Future<void> Function() f=() async{return Future<int>.value(1);};}",
        "void main() async{var f=(){await 1;};}",
        "void main(){Timer(1,()=>print(1));}",
        "void main(){scheduleMicrotask((int x)=>print(x));}",
        "void main() async{print(await Future<void>.value());}",
        "void main(){Future<int>.value('bad');}",
        "void main(){Future<int>.delayed(Duration.zero,()=> 'bad');}",
        "void main(){Future<int>.value(1).then((x)=>x);}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
/// Intrínsecos identificados por span não capturam chamadas de funções homônimas do usuário.
#[test]
fn builtin_resolution_and_import_gate() {
    let r = check("void scheduleMicrotask(int x){print(x);}void main(){scheduleMicrotask(1);}")
        .unwrap();
    assert!(r.async_builtins.is_empty());
    let r = check("void main(){scheduleMicrotask(()=>print(1));}").unwrap();
    assert_eq!(r.async_builtins.len(), 1);
    let source = "void main(){scheduleMicrotask(()=>print(1));}";
    let tokens = dartforge_lexer::lex(source).unwrap();
    let program = dartforge_parser::parse(&tokens, source.len()).unwrap();
    assert!(dartforge_semantic::analyze(&program).is_err());
}
