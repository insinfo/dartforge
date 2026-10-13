class A<T> {}
class B implements A<Object> {}
class C implements A<Object?> {}
class D extends B implements C {}
//    ^
// [diag.conflictingGenericInterfaces] The class 'D' can't implement both 'A<Object>' and 'A<Object?>' because the type arguments are different.
