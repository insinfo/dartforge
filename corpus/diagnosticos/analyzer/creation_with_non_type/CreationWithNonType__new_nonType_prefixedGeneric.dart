import 'test.dart' as prefix;

void NonType<T>() {}
f() {
  new prefix.NonType<int>();
//           ^^^^^^^
// [diag.newWithNonType] The name 'NonType' isn't a class.
}
