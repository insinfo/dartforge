import 'test.dart' as prefix;

f() {
  new prefix.UnresolvedClass<int>();
//           ^^^^^^^^^^^^^^^
// [diag.newWithNonType] The name 'UnresolvedClass' isn't a class.
}
