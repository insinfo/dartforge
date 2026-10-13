class A {
  A() {}
}
class B extends Object with A {}
//                          ^
// [diag.classUsedAsMixin] The class 'A' can't be used as a mixin because it's neither a mixin class nor a mixin.
