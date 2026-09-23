class A(this.a, this.b) {
  int a;
  int b;
}
class B(super.a, super.a) extends A {}
//            ^
// [context 1] The first definition of this name.
//                     ^
// [diag.duplicateDefinition][context 1] The name 'a' is already defined.
