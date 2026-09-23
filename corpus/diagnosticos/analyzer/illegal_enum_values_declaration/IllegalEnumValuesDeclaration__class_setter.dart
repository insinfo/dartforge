abstract class A implements Enum {
  set values(int _) {}
//    ^^^^^^
// [diag.illegalEnumValuesDeclaration] An instance member named 'values' can't be declared in a class that implements 'Enum'.
}
