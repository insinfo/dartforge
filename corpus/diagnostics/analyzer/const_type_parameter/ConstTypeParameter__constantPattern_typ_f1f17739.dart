void f<T>(x) {
  if (x case T) {}
//           ^
// [diag.constTypeParameter] Type parameters can't be used in a constant expression.
}
