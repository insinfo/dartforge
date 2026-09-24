import 'test.dart' as prefix;

void NonType() {}
f() {
  prefix.NonType.named();
//               ^^^^^
// [diag.undefinedMethod] The method 'named' isn't defined for the type 'void Function()'.
}
