mixin class A {}
mixin class B {}
class C = Object with A, B;
enum E with C {
//          ^
// [diag.classUsedAsMixin] The class 'C' can't be used as a mixin because it's neither a mixin class nor a mixin.
  v
}
