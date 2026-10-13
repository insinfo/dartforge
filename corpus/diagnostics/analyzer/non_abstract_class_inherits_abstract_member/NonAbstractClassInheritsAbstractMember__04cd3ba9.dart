// %before-language-feature: class-modifiers
abstract class M {}
abstract class A {}
abstract class I {
  m();
}
class B = A with M implements I;
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'I.m'.
