class C {
  T call<T>(T t) => t;
}
typedef Fn<T> = T Function(T);
class D<U> {
  Fn<U> f = C();
}

