// Substitui `_internal/vm/lib/identical_patch.dart` (sobreposição
// `sdk_nativo/`, P5d): o conteúdo original, mais as funções `_dartforge*`
// com que o runtime do DartForge constrói os erros que ele mesmo lança
// (cast que falha, `assert`, faixa, `!` sobre null) como os objetos da fonte
// — o registro de `dart:core` entrega os endereços delas ao runtime
// (`dartforge_registrar_ajudante`).

part of "core_patch.dart";

@patch
@pragma("vm:recognized", "other")
@pragma("vm:exact-result-type", bool)
@pragma("vm:external-name", "Identical_comparison")
external bool identical(Object? a, Object? b);

@patch
@pragma("vm:entry-point", "call")
int identityHashCode(Object? object) => object._identityHashCode;

@pragma("vm:entry-point")
Object _dartforgeErroDeTipo(String mensagem) =>
    new _TypeError._create("", 0, 0, mensagem);

@pragma("vm:entry-point")
Object _dartforgeErroDeAssercao(Object? mensagem) => new AssertionError(mensagem);

@pragma("vm:entry-point")
Object _dartforgeErroDeFaixa(int valor, int minimo, int maximo, String? nome) =>
    new RangeError.range(valor, minimo, maximo, nome);

@pragma("vm:entry-point")
Object _dartforgeErroDeEstado(String mensagem) => new StateError(mensagem);

@pragma("vm:entry-point")
Object _dartforgeRastroVazio() => StackTrace.empty;
