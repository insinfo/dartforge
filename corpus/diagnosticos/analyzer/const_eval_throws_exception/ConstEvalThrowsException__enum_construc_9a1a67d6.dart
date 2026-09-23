enum E {
  v();
//^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.
  final int x;
  const E({int? x}) : x = x as int;
//                        ^^^^^^^^
// [context 1] The error is in the field initializer of 'E.new', and occurs here.
}
