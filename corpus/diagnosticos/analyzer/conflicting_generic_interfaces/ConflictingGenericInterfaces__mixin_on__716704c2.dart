class I<T> {}
class A implements I<int> {}
class B implements I<String> {}
mixin M on A implements B {}
//    ^
// [diag.conflictingGenericInterfaces] The mixin 'M' can't implement both 'I<int>' and 'I<String>' because the type arguments are different.
