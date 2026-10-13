extension type A(int it) {}
extension type B(A it) implements num {}
//                                ^^^
// [diag.extensionTypeImplementsNotSupertype] 'num' is not a supertype of 'A', the representation type.
