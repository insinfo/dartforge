import 'test.dart' as prefix;

f() {
  const prefix.UnresolvedClass.named();
//             ^^^^^^^^^^^^^^^
// [diag.constWithNonType] The name 'UnresolvedClass' isn't a class.
}
