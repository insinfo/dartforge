import 'test.dart' as prefix;

void NonType<T>() {}
f() {
  const prefix.NonType<int>.named();
//             ^^^^^^^
// [diag.constWithNonType] The name 'NonType' isn't a class.
}
