//! Regressões de assinaturas externas, metadata e limites da ABI escalar.
/// Executa o frontend real para validar anotações e tipos sem gerar código nativo.
fn check(source: &str) -> Result<(), String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::validate(&program).map_err(|e| e.message)
}
/// Um binding externo completo dispensa retorno no corpo Dart inexistente.
#[test]
fn external_scalar_signatures_and_metadata() {
    for source in [
        "@Native<Int32 Function(Int32)>(symbol:'increment') external int inc(int value); void main(){print(inc(1));}",
        "@Native<Void Function(Int64)>(symbol:'consume',isLeaf:true) external void consume(int value); void main(){consume(1);}",
        "@Native<Int64 Function()>() external int clock(); void main(){print(clock());}",
        "@Deprecated('use next') int old()=>1; @deprecated class C {} void main(){print(old());}",
        "class A { int f()=>1; } class B extends A { @override int f()=>2; } void main(){print(B().f());}",
    ] {
        check(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}
/// Tipos gerenciados, nullable, aridades distintas e declarações inválidas falham cedo.
#[test]
fn native_rejects_incompatible_or_unsupported_signatures() {
    for source in [
        "@Native<Int32 Function(Int32)>() external int f(bool x); void main(){}",
        "@Native<Int32 Function(Int64)>() external int f(int? x); void main(){}",
        "@Native<Void Function()>() external int f(); void main(){}",
        "@Native<Int64 Function()>() external void f(); void main(){}",
        "@Native<Int32 Function(Int32)>() external int f(); void main(){}",
        "@Native<Int32 Function(Void)>() external int f(int x); void main(){}",
        "@Native<Int32 Function()>(symbol:'bad-name') external int f(); void main(){}",
        "@Native<Int32 Function()>(assetId:'x') external int f(); void main(){}",
        "@Native<Int32 Function()>() int f()=>1; void main(){}",
        "external int f(); void main(){}",
        "@Native<Int32 Function()>() external int f<T>(); void main(){}",
        "class C { @Native<Int32 Function()>() external int f(); } void main(){}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
