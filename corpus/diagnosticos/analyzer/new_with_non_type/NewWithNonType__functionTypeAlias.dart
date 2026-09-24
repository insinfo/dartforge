typedef F = void Function();

void foo() {
  new F();
//    ^
// [diag.newWithNonType] The name 'F' isn't a class.
}
