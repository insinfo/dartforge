const dynamic a = null;

enum E {
  v(a);
//^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.
//  ^
// [context 1] The exception is 'A value of type 'Null' can't be assigned to a parameter of type 'int' in a const constructor.' and occurs here.
  const E(int a);
}
