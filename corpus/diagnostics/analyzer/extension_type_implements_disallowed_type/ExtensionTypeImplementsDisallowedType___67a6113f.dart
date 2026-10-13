extension type A(int it) implements X {}
//                                  ^
// [diag.extensionTypeImplementsDisallowedType] Extension types can't implement 'X'.
typedef X = (int, String);
