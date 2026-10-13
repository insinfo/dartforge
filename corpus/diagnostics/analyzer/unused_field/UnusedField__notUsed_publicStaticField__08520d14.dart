mixin _A {
  static String f1 = "x";
//              ^^
// [diag.unusedField] The value of the field 'f1' isn't used.
}
void main() => print(_A);
