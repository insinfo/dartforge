class A {
  const A(x);
}
class B {
}
main() {
  const A(B());
//        ^^^
// [diag.constWithNonConst] The constructor being called isn't a const constructor.
}
