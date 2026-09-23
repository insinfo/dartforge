const bool kIsWeb = identical(0, 0.0);

void f() {
  var x = 2;
  const A(kIsWeb ? 0 : x);
//                     ^
// [diag.invalidConstant] Invalid constant value.
}

class A {
  const A(int _);
}
