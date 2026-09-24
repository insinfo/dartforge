import 'dart:async' as a;
f() {
  return const a.Future.noSuchConstructor();
//                      ^^^^^^^^^^^^^^^^^
// [diag.constWithUndefinedConstructor] The class 'a.Future' doesn't have a constant constructor 'noSuchConstructor'.
}
