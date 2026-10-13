base class A {}
sealed class B extends A {}
mixin M {}
base class C = Object with M implements B;
