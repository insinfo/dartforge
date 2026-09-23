abstract class A<T> {
  (Object?, dynamic, dynamic) method(T t);
}

abstract class B<T> {
  (dynamic, Object?, dynamic) method(T t);
}

abstract class C<T> implements A<T>, B<T> {}

abstract class D<T> {
  (dynamic, dynamic, Object?) method(T t);
}

abstract class E<T> implements C<T>, D<T> {}

extension type F<T>(C<T> c) implements A<T>, B<T> {}

extension type G<T>(E<T> e) implements F<T>, D<T> {}
