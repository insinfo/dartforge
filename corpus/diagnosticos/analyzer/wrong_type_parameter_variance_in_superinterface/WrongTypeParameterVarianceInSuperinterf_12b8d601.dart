typedef F<X> = X Function();
mixin A<X> {}
enum E<X> with A<F<X>> {
  v
}
