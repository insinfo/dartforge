import 'dart:math' show max, FooBar;
//                           ^^^^^^
// [diag.undefinedShownName] The library 'dart:math' doesn't export a member with the shown name 'FooBar'.
main() {
  print(max(1, 2));
}
