abstract class Foo<T> {
  const factory Foo.a() = _Foo;
  const Foo();
}
class _Foo<T> extends Foo<T> {
  const _Foo();
}
Foo<int> bar<T>() => const .a<int>();
