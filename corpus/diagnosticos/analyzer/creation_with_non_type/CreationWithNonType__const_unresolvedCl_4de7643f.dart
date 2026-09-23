import 'test.dart' as prefix;

f() {
  const prefix.UnresolvedClass<int>();
//             ^^^^^^^^^^^^^^^
// [diag.constWithNonType] The name 'UnresolvedClass' isn't a class.
}
