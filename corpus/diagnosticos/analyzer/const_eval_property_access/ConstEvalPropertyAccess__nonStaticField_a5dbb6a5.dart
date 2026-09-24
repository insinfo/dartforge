class C<T> {
  const C();
  T? get t => null;
}

const x = const C().t;
//        ^^^^^^^^^^^
// [diag.constEvalPropertyAccess] The property 't' can't be accessed on the type 'C<dynamic>' in a constant expression.
