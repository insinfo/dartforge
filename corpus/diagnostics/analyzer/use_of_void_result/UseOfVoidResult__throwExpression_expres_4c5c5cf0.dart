void f(void x) {
  throw x;
//      ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
// [diag.throwOfInvalidType] The type 'void' of the thrown expression must be assignable to 'Object'.
}
