external T f<T extends num>(T a, T b);
void g(int cb(int a, double b)) {}
void main() {
  g(f);
//  ^
// [diag.couldNotInfer] Couldn't infer type parameter 'T'.\n\nTried to infer 'num' for 'T' which doesn't work:\n  Function type declared as 'T Function<T extends num>(T, T)'\n                used where  'int Function(int, double)' is required.\n\nConsider passing explicit type argument(s) to the generic.
// [diag.argumentTypeNotAssignable] The argument type 'num Function(num, num)' can't be assigned to the parameter type 'int Function(int, double)'.
}
