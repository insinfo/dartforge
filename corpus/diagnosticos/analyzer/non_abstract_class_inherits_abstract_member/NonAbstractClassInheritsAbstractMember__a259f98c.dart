// %before-language-feature: class-modifiers
abstract class A { set s1(v); set s2(v); }
abstract class B implements A { set s1(v) {} }
class C extends Object with B {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'setter A.s2'.
