abstract class A implements Enum {
  int get hashCode => 0;
//        ^^^^^^^^
// [diag.illegalConcreteEnumMemberDeclaration] A concrete instance member named 'hashCode' can't be declared in a class that implements 'Enum'.
}
