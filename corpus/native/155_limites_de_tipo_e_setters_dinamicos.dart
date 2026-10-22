// Numa chamada dinâmica, os argumentos de tipo passados respeitam os limites
// dos parâmetros de tipo (de método, de closure genérica e de função estática
// arrancada), com o texto da VM ("… of 'U'"); a escrita dinâmica num campo
// verifica o valor contra o tipo do campo ("… of 'value'"), inclusive `T` da
// classe e `int`.
class A {}

class C<T> {
  void f1<U extends A>() {}
  void f2<U extends T>() {}
  static void s<U extends num>() {}
}

class D {}

class B {}

class Campos<T> {
  late D c;
  T? t;
  int i = 0;
  set s(D v) {}
}

void t(void Function() f) {
  try {
    f();
    print('ok');
  } catch (e) {
    print('${e.runtimeType}: $e');
  }
}

void main() {
  dynamic d = C<int>();
  t(() => d.f1<Object>());
  t(() => d.f1<A>());
  t(() => d.f2<String>());
  t(() => d.f2<int>());
  t(() => d.f2());
  dynamic g = <U extends num>(U x) => x;
  t(() => g<String>('s'));
  t(() => g(1));
  dynamic s = C.s;
  t(() => s<String>());
  t(() => s<int>());

  dynamic a = Campos<String>();
  t(() => a.c = B());
  t(() => a.t = 1);
  t(() => a.i = 's');
  t(() => a.s = B());
  t(() => a.c = null);
  t(() => a.t = 'x');
}
