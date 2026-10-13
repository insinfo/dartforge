void f(A x) {
  if (x case R _) {}
}

enum A with R { v }
mixin R {}
