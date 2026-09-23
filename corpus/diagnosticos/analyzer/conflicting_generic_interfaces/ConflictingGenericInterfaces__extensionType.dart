class I<T> {}
class A implements I<int> {}
class B implements I<num> {}
extension type C(Never it) implements A, B {}
//             ^
// [diag.conflictingGenericInterfaces] The extension type 'C' can't implement both 'I<int>' and 'I<num>' because the type arguments are different.
//               ^^^^^
// [diag.extensionTypeRepresentationTypeBottom] The representation type can't be a bottom type.
