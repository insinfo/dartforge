// Finalizer (dart:core) e NativeFinalizer (dart:ffi) sobre o coletor.
//
// A especificação não promete QUANDO nem SE um callback de Finalizer roda, então
// a saída só contém o que é garantido: o valor destacado (`detach`) e o valor
// ainda vivo nunca são finalizados, e todo callback que chegou é de um valor
// anexado. O NativeFinalizer usa o `free` da libc sobre memória de `malloc` (uma
// função nativa que não volta à VM — chamar Dart de um finalizador nativo é
// proibido, dart-lang/sdk#54939); ele roda na coleta ou no fim do isolado.
import 'dart:async';
import 'dart:ffi';

typedef MallocC = Pointer<Void> Function(IntPtr);
typedef MallocD = Pointer<Void> Function(int);

final finalizados = <String>[];
final fin = Finalizer<String>((t) => finalizados.add(t));

final class Recurso implements Finalizable {
  final int id;
  Recurso(this.id);
}

void anexar(int n) {
  for (var i = 0; i < n; i++) {
    final o = Object();
    fin.attach(o, 'o$i', detach: i == 1 ? o : null);
    if (i == 1) fin.detach(o);
  }
}

late final void Function(Pointer<Void>) _free;

int anexarNativos(NativeFinalizer nf, MallocD malloc, int n) {
  var soma = 0;
  for (var i = 0; i < n; i++) {
    final r = Recurso(i);
    final token = malloc(32);
    nf.attach(r, token, detach: r, externalSize: 32);
    if (i == 2) {
      // O destacado não é liberado pelo finalizador: libera aqui.
      nf.detach(r);
      _free(token);
    }
    soma += r.id;
  }
  return soma;
}

/// Gera lixo e cede o laço algumas vezes (para os callbacks prontos rodarem).
Future<void> pressionar() async {
  for (var r = 0; r < 10; r++) {
    var lixo = <List<int>>[];
    for (var i = 0; i < 2000; i++) {
      lixo.add(List.filled(8, i));
    }
    lixo = [];
    await Future.delayed(Duration(milliseconds: 1));
  }
}

Future<void> main() async {
  anexar(3);
  final vivo = Object();
  fin.attach(vivo, 'vivo');
  await pressionar();
  print([
    finalizados.contains('o1'),
    finalizados.contains('vivo'),
    finalizados.every((t) => t == 'o0' || t == 'o2'),
  ]);

  final lib = DynamicLibrary.process();
  final malloc = lib.lookupFunction<MallocC, MallocD>('malloc');
  final free = lib.lookup<NativeFunction<Void Function(Pointer<Void>)>>('free');
  _free = free.asFunction<void Function(Pointer<Void>)>();
  final nf = NativeFinalizer(free.cast());
  print(anexarNativos(nf, malloc, 50));
  await pressionar();
  final l = malloc(16).cast<Uint8>().asTypedList(16, finalizer: free.cast());
  l[3] = 9;
  print([l[3], identical(vivo, vivo)]);
}
