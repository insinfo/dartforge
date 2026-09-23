class C<T> {
  final t;
  const C(dynamic x) : t = x as T;
//                         ^^^^^^
// [context 1] The error is in the field initializer of 'C.new', and occurs here.
// [context 2] The error is in the field initializer of 'C.new', and occurs here.
}

main() {
  const C<int>(0);
  const C<int>('foo');
//^^^^^^^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.
  const C<int>(null);
//^^^^^^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 2] Evaluation of this constant expression throws an exception.
}
