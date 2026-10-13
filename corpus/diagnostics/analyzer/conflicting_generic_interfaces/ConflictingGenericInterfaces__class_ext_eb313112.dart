class I<T> {}
class A implements I<int> {}
class B extends A {}
//    ^
// [diag.conflictingGenericInterfaces] The class 'B' can't implement both 'I<int>' and 'I<String>' because the type arguments are different.
augment class B implements I<String> {}
