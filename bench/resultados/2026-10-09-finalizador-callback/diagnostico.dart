import 'dart:ffi';

final class Recurso implements Finalizable {}
void finalizar(Pointer<Void> token) {
  throw StateError('callback inválido foi executado');
}

bool recusado(Pointer<NativeFinalizerFunction> callback) {
  final nf = NativeFinalizer(callback);
  final recurso = Recurso();
  try {
    nf.attach(recurso, nullptr, externalSize: 17);
    return false;
  } on ArgumentError {
    return true;
  }
}

void main() {
  final persistente = Pointer.fromFunction<Void Function(Pointer<Void>)>(finalizar);
  final local = NativeCallable<Void Function(Pointer<Void>)>.isolateLocal(finalizar);
  try {
    print([recusado(persistente), recusado(local.nativeFunction)]);
  } finally {
    local.close();
  }
}

