class A {}
class B extends A {}
class G<E extends A> {
  const G();
}
f() { return const G<B>(); }
