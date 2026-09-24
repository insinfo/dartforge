class I<T> {}
class A implements I<int> {}
mixin B implements I<String> {}
class C extends A with B {}
//    ^
// [diag.conflictingGenericInterfaces] The class 'C' can't implement both 'I<int>' and 'I<String>' because the type arguments are different.
