class C {
  T m<T extends num>(T x, T y) {
    throw 'error';
  }
}
void g(int cb(Object a, Object b)) {}
void main() {
  g(C().m);
//  ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'num Function(num, num)' can't be assigned to the parameter type 'int Function(Object, Object)'.
}
