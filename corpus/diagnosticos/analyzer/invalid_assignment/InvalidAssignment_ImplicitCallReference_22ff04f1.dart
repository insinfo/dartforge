class C {
  T call<T>(T t) => t;
}

typedef Fn = T Function<T>(T);

Fn f = C();
