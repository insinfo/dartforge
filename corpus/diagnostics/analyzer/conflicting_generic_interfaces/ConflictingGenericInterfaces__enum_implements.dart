class I<T> {}
class A implements I<int> {}
class B implements I<String> {}
enum E implements A, B {
//   ^
// [diag.conflictingGenericInterfaces] The enum 'E' can't implement both 'I<int>' and 'I<String>' because the type arguments are different.
  v
}
