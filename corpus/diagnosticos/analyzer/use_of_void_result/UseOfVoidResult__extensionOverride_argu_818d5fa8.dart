extension E on String {
  int get g => 0;
}

void f() {}

void h() {
  E(f()).g;
//  ^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
