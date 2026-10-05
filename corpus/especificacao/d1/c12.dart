class A {
  const A(int x);
}
class B<T> {
  final T f;
  const B(this.f);
}
const dynamic d = 'a';
const a = const A(d);
const b = const A('a');
const c = const B<int>(d);
