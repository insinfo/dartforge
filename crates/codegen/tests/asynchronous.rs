//! Regressões originais de async com Future aninhado e temporizadores one-shot.
const SOURCE: &str = r"
Future<int> scalar() async {print('start');return 7;}
Future<Future<int>> nested() async {return Future<int>.value(9);}
Future<int?> absent() async {}
Future<void> main() async {
  var f=scalar();print('caller');print(await f);
  var inner=await nested();print(inner is Future<int>);print(await inner);
  Object adopted=await Future<Object>.value(Future<int>.value(11));
  print(adopted is int);print(adopted as int);
  print(await absent());
  var d=Duration(seconds:1,milliseconds:2);print(d.inMilliseconds);
  var cancelled=Timer(Duration.zero,()=>print('bad'));cancelled.cancel();
  print(cancelled.isActive);print(cancelled.tick);
  scheduleMicrotask(()=>print('micro'));
  print(await Future<int>.delayed(Duration.zero,()=>13));
  var callback=() async {return 17;};print(await callback());
}
";
const EXPECTED: &str = "start\ncaller\n7\ntrue\n9\ntrue\n11\nnull\n1002\nfalse\n0\nmicro\n13\n17\n";

/// Usa análise real antes de emitir os descritores e wrappers assíncronos.
fn compile(source: &str) -> String {
    let tokens = dartforge_lexer::lex(source).unwrap();
    let program = dartforge_parser::parse(&tokens, source.len()).unwrap();
    let resolution = dartforge_semantic::analyze_with_async_library(&program, true).unwrap();
    dartforge_codegen::emit(&dartforge_hir::lower_resolved(program, resolution))
}
/// Verifica que await retira uma caixa e funções normais retornam Future explícito.
#[test]
fn emits_boxed_future_instead_of_thenable() {
    let output = compile(SOURCE);
    assert!(output.contains("return $dartforgeAsync(() => (function* ()"));
    assert!(output.contains("(yield "));
    assert!(output.contains("class $dartforgeFuture"));
}
/// Compara a saída com o oracle Dart 3.6.2, inclusive camada aninhada preservada.
#[test]
#[ignore = "requer Node no PATH"]
fn async_nested_future_timer_and_microtask_match_oracle() {
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &compile(SOURCE)])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        EXPECTED
    );
}

/// Adotar Future<Future<int>> em Future<Object> conserva o Future interno após um await.
#[test]
#[ignore = "requer Node no PATH"]
fn adoption_preserves_the_completed_futures_value() {
    let source = "Future<void> main() async {var inner=Future<int>.value(19);var outer=Future<Future<int>>.value(inner);Object value=await Future<Object>.value(outer);print(value is Future<int>);print(value is int);print(await (value as Future<int>));}";
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &compile(source)])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "true\nfalse\n19\n"
    );
}

/// A adoção conecta listeners imediatamente; só a conclusão escalar cria microtask.
#[test]
#[ignore = "requer Node no PATH"]
fn adopted_pending_future_keeps_microtask_order() {
    let source = "Future<void> run() async {print('start');var inner=Future<int>.value(1);var outer=Future<int>.value(inner);scheduleMicrotask(()=>print('m1'));print(await outer);}void main(){run();scheduleMicrotask(()=>print('m2'));}";
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &compile(source)])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "start\n1\nm1\nm2\n"
    );
}

/// Arrow void descarta escalar sem await extra, mas aguarda Future retornado.
#[test]
#[ignore = "requer Node no PATH"]
fn void_arrows_adopt_futures_without_suspending_scalar_returns() {
    let source = "int mark(){print('scalar');return 1;} Future<void> scalar() async => mark(); Future<void> later() async => Future<int>.delayed(Duration.zero,(){print('value');return 1;}); Future<void> main() async {var f=scalar();scheduleMicrotask(()=>print('micro'));await f;print('scalar-after');await later();print('future-after');}";
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &compile(source)])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "scalar\nmicro\nscalar-after\nvalue\nfuture-after\n"
    );
}
