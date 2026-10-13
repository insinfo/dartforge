typedef F1<X> = void Function(X);
typedef F2<X> = void Function(F1<X>);
class A<X> {}
class B<X> extends A<F2<X>> {}
