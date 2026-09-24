// %before-language-feature: class-modifiers
class A {}
class B {}
class C = Object with A, B;
class D extends Object with C {}
//                          ^
// [diag.mixinInheritsFromNotObject] The class 'C' can't be used as a mixin because it extends a class other than 'Object'.
