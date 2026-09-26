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

// A chamada de uma closure com aridade incompatível precisa lançar a classe
// real do SDK; o id sintético 1012 do runtime não participa do despacho de
// métodos nem do grafo de subtipos da biblioteca compilada da fonte.
@pragma("vm:entry-point")
Object _dartforgeErroDeChamada(String nome) =>
    new NoSuchMethodError.withInvocation(
        null, new Invocation.method(new Symbol(nome), const []));

@pragma("vm:entry-point")
Object _dartforgeRastroVazio() => StackTrace.empty;

/// O gancho de `Uri.base` que o embedder instala na partida
/// (`DartUtils::PrepareCoreLibrary`: o `_getUriBaseClosure` de `dart:io`).
@pragma("vm:entry-point")
void _dartforgeDefinirUriBase(_UriBaseClosure gancho) {
  _uriBaseClosure = gancho;
}
