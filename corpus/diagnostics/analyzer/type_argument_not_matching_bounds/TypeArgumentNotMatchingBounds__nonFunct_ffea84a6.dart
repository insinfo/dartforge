class A {}
class B extends A {}
typedef X<T extends A> = Map<int, T>;
void f(X<B> a) {}
