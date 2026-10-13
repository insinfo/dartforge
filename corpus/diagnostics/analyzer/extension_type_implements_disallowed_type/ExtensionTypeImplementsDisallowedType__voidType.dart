extension type A(int it) implements X {}
//                                  ^
// [diag.extensionTypeImplementsDisallowedType] Extension types can't implement 'void'.
typedef X = void;
