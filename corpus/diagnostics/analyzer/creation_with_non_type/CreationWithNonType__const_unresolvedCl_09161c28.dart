import 'test.dart' as prefix;

f() {
  const prefix.UnresolvedClass<int>.named();
//             ^^^^^^^^^^^^^^^
// [diag.constWithNonType] The name 'UnresolvedClass' isn't a class.
}
