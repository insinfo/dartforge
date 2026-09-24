class A {}
class B extends A {}
class G<E extends A> {}
f() { return new G<B>(); }
