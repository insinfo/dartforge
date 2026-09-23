f(int? a) {
  throw a;
//      ^
// [diag.throwOfInvalidType] The type 'int?' of the thrown expression must be assignable to 'Object'.
}
