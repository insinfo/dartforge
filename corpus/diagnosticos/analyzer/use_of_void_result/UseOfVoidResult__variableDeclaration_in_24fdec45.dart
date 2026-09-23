void f(void x) {
  dynamic z = x;
//        ^
// [diag.unusedLocalVariable] The value of the local variable 'z' isn't used.
//            ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
