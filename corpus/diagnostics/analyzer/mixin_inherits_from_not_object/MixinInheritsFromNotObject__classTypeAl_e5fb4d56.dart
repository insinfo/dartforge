// %before-language-feature: class-modifiers
class A {}
class B extends Object with A {}
class C = Object with B;
//                    ^
// [diag.mixinInheritsFromNotObject] The class 'B' can't be used as a mixin because it extends a class other than 'Object'.
