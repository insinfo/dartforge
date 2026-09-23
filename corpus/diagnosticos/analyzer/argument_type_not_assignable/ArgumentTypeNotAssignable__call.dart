typedef bool Predicate<T>(T object);

Predicate<String> f() => (String s) => false;

void main() {
  f().call(3);
//         ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
}