const bool kIsWeb = identical(0, 0.0);

void f() {
  const A(kIsWeb ? 0 : 1);
}

class A {
  const A(int _);
}
