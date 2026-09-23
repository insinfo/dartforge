import 'test.dart' as prefix;

f() {
  prefix.UnresolvedClass();
//       ^^^^^^^^^^^^^^^
// [diag.undefinedFunction] The function 'UnresolvedClass' isn't defined.
}
