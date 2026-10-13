class I<T> {}
class A implements I<int> {}
class B implements I<String> {}
class C extends A implements B {}
//    ^
// [diag.conflictingGenericInterfaces] The class 'C' can't implement both 'I<int>' and 'I<String>' because the type arguments are different.
