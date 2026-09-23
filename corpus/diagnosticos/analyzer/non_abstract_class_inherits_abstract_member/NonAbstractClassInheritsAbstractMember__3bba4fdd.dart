abstract class A {
  set field(_);
}
abstract class I {
  var field;
}
class B extends A implements I {
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'setter A.field'.
  get field => 0;
}
