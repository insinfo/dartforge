import 'test.dart' as prefix;

void NonType() {}
f() {
  new prefix.NonType();
//           ^^^^^^^
// [diag.newWithNonType] The name 'NonType' isn't a class.
}
