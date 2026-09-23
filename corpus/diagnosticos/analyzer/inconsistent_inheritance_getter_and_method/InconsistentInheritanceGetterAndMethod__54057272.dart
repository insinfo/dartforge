abstract interface class I {
  String foo();
}

mixin M {
  int get foo => 42;
}

abstract class C with M implements I {
  String foo() => 'C';
//       ^^^
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'M') and also a method (from 'I').
}
