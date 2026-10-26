// Um mixin que aplica mixins ele mesmo (`mixin class A<U> = Object with
// M<U>`, aplicado em `class B<V> = Object with A<V>`) traz os membros de `M`;
// uma classe que implementa (sem herdar) uma classe concreta e tem
// `noSuchMethod` recebe forwarders para os membros dela.
mixin M<T> {
  Type t() => T;
  String who() => 'M<$T>';
}

mixin class A<U> = Object with M<U>;

class B<V> = Object with A<V>;

class F extends B<String> {}

mixin class C<U> = Object with M<List<U>>;

class D<V> = Object with C<Set<V>>;

class H<T> extends D<Map<String, T>> {}

class Real {
  int foo() => 42;
  String get name => 'real';
  set name(String v) {}
}

class Mock implements Real {
  noSuchMethod(Invocation i) =>
      i.isMethod ? 84 : (i.isGetter ? 'mock' : null);
}

void main() {
  print([F().t(), F().who(), B<int>().t(), H<int>().t()]);
  Real r = Mock();
  print([r.foo(), r.name]);
  r.name = 'x';
  dynamic d = Mock();
  print(d.foo());
}
