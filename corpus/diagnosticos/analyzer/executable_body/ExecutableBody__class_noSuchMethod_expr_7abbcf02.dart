class I {
  noSuchMethod(_) => '';
}
class A implements I {
  foo();
//^^^^^^
// [diag.concreteClassWithAbstractMember] 'foo' must have a method body because 'A' isn't abstract.
}
