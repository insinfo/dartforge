import 'test.dart' as prefix;

void NonType<T>() {}
f() {
  prefix.NonType<int>.named();
//                    ^^^^^
// [diag.undefinedMethod] The method 'named' isn't defined for the type 'void Function()'.
}
