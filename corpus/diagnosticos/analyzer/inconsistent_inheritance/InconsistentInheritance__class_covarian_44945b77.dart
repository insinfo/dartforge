class D1 {}
class D2 {}
class D implements D1, D2 {}

class A { void m(covariant D d) {} }
abstract class B1 { void m(D1 d1); }
abstract class B2 { void m(D2 d2); }
class C extends A implements B1, B2 {}
//    ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'm': A.m (void Function(D)), B1.m (void Function(D1)), B2.m (void Function(D2)).
