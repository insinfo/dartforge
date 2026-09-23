class I<T> {}
class A implements I<int> {}
mixin M implements I<String> {}
class C = A with M;
//    ^
// [diag.conflictingGenericInterfaces] The class 'C' can't implement both 'I<int>' and 'I<String>' because the type arguments are different.
