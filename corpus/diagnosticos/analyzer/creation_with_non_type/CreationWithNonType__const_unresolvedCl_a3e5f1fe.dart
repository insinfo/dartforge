import 'test.dart' as prefix;

f() {
  const prefix.UnresolvedClass();
//             ^^^^^^^^^^^^^^^
// [diag.constWithNonType] The name 'UnresolvedClass' isn't a class.
}
