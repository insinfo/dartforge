typedef F<X> = X Function();
class A<X> {}
mixin B<X> on A<F<X>> {}
