import 'test.dart' as prefix;

f() {
  prefix.UnresolvedClass<int>();
//       ^^^^^^^^^^^^^^^
// [diag.undefinedFunction] The function 'UnresolvedClass' isn't defined.
}
