// `super.m()` sem implementação no programa vai ao forwarder de
// `noSuchMethod` da superclasse (que chama o `noSuchMethod` de `this`), ou à
// implementação de uma superclasse do SDK; getter e setter homônimos de
// bibliotecas importadas diferentes (inclusive com prefixo); `call` implícito
// em contexto `Function?`; chamada dinâmica com aridade ou quantidade de
// argumentos de tipo errada vai ao `noSuchMethod` do receptor, assim como
// chamar um objeto sem `call`; o `call` de uma extensão em `Object?` invocado
// sobre `null`.
import 'dart:collection';

import 'getter.dart';
import 'getter.dart' as g;
import 'setter.dart';
import 'setter.dart' as s;

class A {
  int foo();
  noSuchMethod(im) => 42;
}

class B extends A {
  noSuchMethod(im) => 87;
  int foo() => super.foo();
}

class Lista extends ListBase<int> {
  final _l = <int?>[];
  int get length => _l.length;
  set length(int n) => _l.length = n;
  int operator [](int i) => _l[i]!;
  void operator []=(int i, int v) => _l[i] = v;
  void add(int v) {
    super.add(v * 10);
  }
}

class CallableClass<T> {
  T call() => throw 0;
}

class F {
  call() => 0;
  foo() => 1;
  noSuchMethod(Invocation i) => 'nsm ${i.memberName} ${i.positionalArguments}';
}

class SemCall {
  noSuchMethod(Invocation i) => 'sem call ${i.memberName}';
}

class Gen {
  noSuchMethod(m) => 123;
  bar<U, V>(x) => x + 1;
}

extension E on Object? {
  String call<T, U>([Object? v]) => 'call<$T,$U>($v) on $this';
}

void main() {
  print(B().foo());
  final l = Lista()..add(1)..add(2);
  print(l);

  getValue = 123;
  print(x);
  x = 456;
  print(setValue);
  s.x = 789;
  print(setValue);
  print(g.x);

  final c = CallableClass<int>();
  Function? f;
  f ??= c;
  print(f.runtimeType);
  Function? h = c;
  print(h.runtimeType);

  dynamic d = F();
  print(d.foo(1, 2));
  print(d.call(1, 2));
  print(d(1, 2));
  dynamic sc = SemCall();
  print(sc(1));
  dynamic gen = Gen();
  print(gen.bar<int, String>(1));
  print(gen.bar<int>(1));
  print(gen.bar());

  print(null<int, String>(1));
  print(0<int, String>(2));
}
