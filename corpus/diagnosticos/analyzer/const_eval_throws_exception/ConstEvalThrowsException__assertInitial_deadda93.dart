class A {
  const A(int i)
  : assert(i == 1); // (2)
//  ^^^^^^^^^^^^^^
// [context 2] The exception is 'The assertion in this constant expression failed.' and occurs here.
}
class B extends A {
  const B(int i) : super(i);
//      ^
// [context 1] The evaluated constructor 'A.new' is called by 'B.new' and 'B.new' is defined here.
}
main() {
  print(const B(2)); // (1)
//      ^^^^^^^^^^
// [diag.constEvalThrowsException][context 1][context 2] Evaluation of this constant expression throws an exception.
}
