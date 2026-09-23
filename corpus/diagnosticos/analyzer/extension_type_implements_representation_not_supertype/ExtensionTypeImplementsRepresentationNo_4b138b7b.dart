extension type A(String it) {}
extension type B(int it) implements A {}
//                                  ^
// [diag.extensionTypeImplementsRepresentationNotSupertype] 'String', the representation type of 'A', is not a supertype of 'int', the representation type of 'B'.
