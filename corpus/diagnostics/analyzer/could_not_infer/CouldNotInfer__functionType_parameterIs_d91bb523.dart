external T f<T extends num>(T a, T b);
void g(int cb(Object a, Object b)) {}
void main() {
  g(f);
//  ^
// [diag.argumentTypeNotAssignable] The argument type 'num Function(num, num)' can't be assigned to the parameter type 'int Function(Object, Object)'.
}
