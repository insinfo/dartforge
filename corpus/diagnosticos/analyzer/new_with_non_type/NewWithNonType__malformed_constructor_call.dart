class C {
  C.x();
}
main() {
  new C.x.y();
//    ^^^
// [diag.newWithNonType] The name 'x' isn't a class.
}
