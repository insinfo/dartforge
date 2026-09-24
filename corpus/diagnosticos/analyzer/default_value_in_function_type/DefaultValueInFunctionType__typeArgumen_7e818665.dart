class A<T> {}

void f() {
  A<void Function([int x = 42])>();
//                       ^
// [diag.defaultValueInFunctionType] Parameters in a function type can't have default values.
}
