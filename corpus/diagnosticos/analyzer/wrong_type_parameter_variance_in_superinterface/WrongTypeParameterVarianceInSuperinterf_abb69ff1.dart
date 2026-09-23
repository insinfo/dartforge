typedef F<X> = X Function();
class A<X> {}
mixin B<X> implements A<F<X>> {}
