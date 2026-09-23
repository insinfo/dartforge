typedef F<X> = X Function();
class A<X> {}
class B<X> implements A<F<X>> {}
