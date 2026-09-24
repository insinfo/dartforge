typedef F<X> = X Function();
mixin A<X> {}
class B<X> extends Object with A<F<X>> {}
