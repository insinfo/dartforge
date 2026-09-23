class I<T> {}
mixin M1 implements I<int> {}
mixin M2 implements I<String> {}
enum E with M1, M2 {
//   ^
// [diag.conflictingGenericInterfaces] The enum 'E' can't implement both 'I<int>' and 'I<String>' because the type arguments are different.
  v
}
