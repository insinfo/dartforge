class A<T> {
  final Object x;
  const A.a() : x = const <T>[];
  const A.b() : x = const <T, int>{};
  const A.c() : x = const <T>{};
  const A.d(Object o) : x = o is T;
  const A.e(Object o) : x = o as List<T>;
  const A.f() : x = T;
  const A.g() : x = List<T>;
  const A.h() : x = A<T>.a;
  const A.i() : x = const A<T>.a();
  const A.j() : x = <T>[];
  const A.k() : x = idf<T>;
}
X idf<X>(X x) => x;
