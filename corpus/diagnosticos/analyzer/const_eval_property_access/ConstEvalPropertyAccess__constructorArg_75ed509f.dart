class C {
  final bool x;
  const C(this.x);
}
const C a = C(true);
const C b = C(false || a.x);
//            ^^^^^^^^^^^^
// [diag.constEvalPropertyAccess] The property 'x' can't be accessed on the type 'C' in a constant expression.
