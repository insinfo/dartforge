class I<T> {}
class A implements I<int> {}
class B implements I<int?> {}
class C extends A implements B {}
//    ^
// [diag.conflictingGenericInterfaces] The class 'C' can't implement both 'I<int>' and 'I<int?>' because the type arguments are different.
