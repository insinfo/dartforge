import 'test.dart' as prefix;

void NonType<T>() {}
f() {
  new prefix.NonType<int>.named();
//           ^^^^^^^
// [diag.newWithNonType] The name 'NonType' isn't a class.
}
