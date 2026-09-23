import 'test.dart' as prefix;

f() {
  prefix.UnresolvedClass.named();
//       ^^^^^^^^^^^^^^^
// [diag.undefinedPrefixedName] The name 'UnresolvedClass' is being referenced through the prefix 'prefix', but it isn't defined in any of the libraries imported using that prefix.
}
