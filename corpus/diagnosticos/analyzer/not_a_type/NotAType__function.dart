f() {}
// [context 1][column 1][length 1] The declaration of 'f' is here.
main() {
  f v = null;
//^
// [diag.notAType][context 1] f isn't a type.
//  ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}