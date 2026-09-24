import 'test.dart' as prefix;

f() {
  new prefix.UnresolvedClass.named();
//           ^^^^^^^^^^^^^^^
// [diag.newWithNonType] The name 'UnresolvedClass' isn't a class.
}
