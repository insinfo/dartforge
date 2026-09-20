//! Bounds, argumentos reificados e promoção conservadora de testes de tipo.
/// Analisa uma fonte real sem gerar código nem executar o programa.
fn check(source: &str) -> Result<dartforge_syntax::Resolution, String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::analyze(&program).map_err(|e| e.message)
}
/// Os corpos genéricos são verificados pelos bounds, inclusive sem instanciações.
#[test]
fn bounds_validate_bodies_and_every_instantiation() {
    for source in [
        "abstract class LoginService{int login();} class Login implements LoginService{int login()=>1;} int use<T extends LoginService>(T x)=>x.login(); void main(){print(use(Login()));}",
        "T identity<T>(T value)=>value; void main(){print(identity<int?>(null));print(identity(1));}",
        "T? maybe<T extends Object>(T? value)=>value; void main(){print(maybe<int>(null));}",
        "T? absent<T>(){} void main(){print(absent<int>());}",
        "bool accepts<T>(Object? x)=>x is T; void main(){print(accepts<int?>(null));print(accepts(null));}",
        "T read<T>(Object? x)=>x as T; void main(){print(read<int>(1));}",
        "void main(){List<int> xs=[1];List<int?> ys=xs;ys.add(null);var mixed=[1,false];print(mixed.length);}",
        "void main(){List<int>? xs=null;if(xs!=null){print(xs.length);}}",
        "int plus<T extends int>(T x)=>x+1;int first<T extends List<int>>(T xs)=>xs[0];void main(){print(plus(1));print(first([1]));}",
        "bool f<T extends List<U>,U extends int>()=>null is T;void main(){print(f());}",
        "bool f<T>(T value)=>value is List<List<Object>>;void main(){print(f([<int>[1],<String>['s']]));}",
        "class A{} class B extends A{} class C extends A{} bool f<T>(T value)=>value is List<A>;void main(){print(f([B(),C()]));}",
    ] {
        check(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
    for source in [
        "int invalid<T>(T value)=>value.missing();void main(){}",
        "T invalid<T>()=>null;void main(){}",
        "T identity<T extends Object>(T x)=>x;void main(){identity<int?>(null);}",
        "abstract class S{int value();} int invalid<T extends S>(T value)=>value.missing();void main(){}",
        "abstract class S{} void f<T extends S>(T x){} void main(){f<int>(1);}",
        "void f<T extends T>(){} void main(){}",
        "void f<T extends U,U extends T>(){} void main(){}",
        "void f<T extends List<T>>(){} void main(){}",
        "void f<T extends int?>(T value){int? x=value;print(x+1);}void main(){}",
        "void main(){Object? x=1;print(x+1);}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
/// Testes de tipo promovem somente locais seguros; capturas mutáveis invalidam a promoção.
#[test]
fn runtime_tests_casts_and_reified_arguments() {
    let resolution=check("bool accepts<T>(Object? x)=>x is T;bool nested<T>(Object? x)=>accepts<List<T>>(x); void main(){print(nested<int>([1]));}").unwrap();
    assert_eq!(resolution.generic_arguments.len(), 2);
    for source in [
        "int f(Object? x){if(x is int){return x+1;}return 0;}void main(){print(f(1));}",
        "int f(Object? x){if(x is! int){return 0;}return x+1;}void main(){print(f(1));}",
        "bool f(Object? x)=>x is int && x>0;void main(){}",
        "bool f(Object? x)=>x is! int || x>0;void main(){}",
        "int twice(int x)=>x*2;void main(){Object? f=twice;print(f is int Function(int));print((f as int Function(int))(2));}",
    ] {
        check(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
    for source in [
        "void f(Object? x){var clear=(){x=null;};if(x is int){clear();print(x+1);}}void main(){}",
        "void f(Object? x){if(x is int){x='bad';print(x+1);}}void main(){}",
        "void f(Object? x){if(x is int){while(true){x=null;break;}print(x+1);}}void main(){}",
        "class C{Object? x;int f(){if(x is int){return x+1;}return 0;}}void main(){}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
