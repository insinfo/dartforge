// %before-language-feature: class-modifiers
class A {}
class B {}
class C = Object with A, B;
enum E with C {
//          ^
// [diag.mixinInheritsFromNotObject] The class 'C' can't be used as a mixin because it extends a class other than 'Object'.
  v
}
