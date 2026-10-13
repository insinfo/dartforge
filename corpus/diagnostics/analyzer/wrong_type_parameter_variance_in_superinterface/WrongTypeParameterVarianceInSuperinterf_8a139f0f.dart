typedef F<X> = X Function();
class A<X> {}
mixin M {}
class B<X> = A<F<X>> with M;
