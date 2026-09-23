class C {
  final int f;
  const C(a) : f = a;
//                 ^
// [context 1] The exception is 'In a const constructor, a value of type 'Null' can't be assigned to the field 'f', which has type 'int'.' and occurs here.
}

const a = const C(null);
//        ^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.
