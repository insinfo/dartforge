// %before-language-feature: class-modifiers
abstract class A { m1(); m2(); }
abstract class B implements A { m1() => 1; }
class C extends Object with B {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'A.m2'.
