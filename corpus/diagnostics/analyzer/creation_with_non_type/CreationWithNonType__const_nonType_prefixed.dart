import 'test.dart' as prefix;

void NonType() {}
f() {
  const prefix.NonType();
//             ^^^^^^^
// [diag.constWithNonType] The name 'NonType' isn't a class.
}
