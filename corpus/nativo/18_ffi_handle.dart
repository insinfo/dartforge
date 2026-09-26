// dart:ffi Handle: um objeto Dart na fronteira nativa — ida e volta por uma
// função C (memcpy(h, p, 0) devolve o próprio h), em lote e aninhado, e um
// callback que recebe e devolve Handle.
import 'dart:ffi';

final class Caixa {
  final String nome;
  Caixa(this.nome);
  @override
  String toString() => 'Caixa($nome)';
}

Object ecoar(Object o) => o is Caixa ? Caixa('${o.nome}!') : o;

void main() {
  final memcpy = DynamicLibrary.process()
      .lookupFunction<Handle Function(Handle, Pointer<Void>, Size), Object Function(Object, Pointer<Void>, int)>('memcpy');
  final c = Caixa('a');
  final volta = memcpy(c, nullptr, 0);
  print([identical(volta, c), volta]);
  final objetos = <Object>[1, 'dois', 3.5, [4], {'cinco': 5}, c, null as Object? ?? 'nulo'];
  print([for (final o in objetos) identical(memcpy(o, nullptr, 0), o)]);

  final cb = Pointer.fromFunction<Handle Function(Handle)>(ecoar).asFunction<Object Function(Object)>();
  print([cb(c), cb(42), cb('texto')]);
  final nc = NativeCallable<Handle Function(Handle, Int32)>.isolateLocal((Object o, int n) => [o, n]);
  print(nc.nativeFunction.asFunction<Object Function(Object, int)>()(c, 7));
  nc.close();
}
