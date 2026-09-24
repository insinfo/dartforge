class A {}
class B extends A {}
typedef F<T extends A>();
F<A> fa = (throw 0);
F<B> fb = (throw 0);
