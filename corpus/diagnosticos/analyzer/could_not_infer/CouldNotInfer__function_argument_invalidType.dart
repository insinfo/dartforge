void foo<T extends num>(T t) {}

void f(X x) {
//     ^
// [diag.undefinedClass] Undefined class 'X'.
  foo(x);
}
