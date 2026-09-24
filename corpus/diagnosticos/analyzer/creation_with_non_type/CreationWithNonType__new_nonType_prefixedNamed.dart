import 'test.dart' as prefix;

void NonType() {}
f() {
  new prefix.NonType.named();
//           ^^^^^^^
// [diag.newWithNonType] The name 'NonType' isn't a class.
}
