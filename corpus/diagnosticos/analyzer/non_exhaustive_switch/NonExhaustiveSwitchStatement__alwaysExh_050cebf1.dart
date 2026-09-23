enum E {
  v1(v2), v2(v1);
//^^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
//        ^^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  const E(Object f);
}

void f(E x) {
  switch (x) {
    case E.v1:
    case E.v2:
      break;
  }
}
