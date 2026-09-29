// O subconjunto de `dart:mirrors` do nativo: `reflectClass` com o nome e a
// biblioteca da classe (o `TypeChecker.fromRuntime` do `source_gen`), e
// `reflect(x).type`. O resto da reflexão lança `UnsupportedError`.
import 'dart:mirrors';

class Local {}

class _Privada {}

class Generica<T> {}

String descrever(ClassMirror m) =>
    '${MirrorSystem.getName(m.simpleName)} @ ${(m.owner as LibraryMirror).uri.scheme}:'
    '${(m.owner as LibraryMirror).uri.pathSegments.last}';

void main() {
  print(descrever(reflectClass(Local)));
  print(descrever(reflectClass(_Privada)));
  print(descrever(reflectClass(Generica)));
  print(descrever(reflectClass(Object)));
  print(MirrorSystem.getName(reflectClass(List).simpleName));
  print((reflectClass(Map).owner as LibraryMirror).uri);
  print((reflectClass(StringBuffer).owner as LibraryMirror).uri);
  print(MirrorSystem.getName(reflect(Local()).type.simpleName));
  print(reflectClass(Local) == reflectClass(Local));
  print(MirrorSystem.getName(#simbolo));
  print(reflectClass(Local).reflectedType == Local);
}
