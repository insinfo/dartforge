class A(this.a, this.b) {
  int a;
  int b;
}
class C(this.x, super.x) extends A {
//    ^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
//           ^
// [context 1] The first definition of this name.
//                    ^
// [diag.duplicateDefinition][context 1] The name 'x' is already defined.
  final int x;
}
