typedef F<X> = X Function();
mixin M<X> {}
class B<X> = Object with M<F<X>>;
