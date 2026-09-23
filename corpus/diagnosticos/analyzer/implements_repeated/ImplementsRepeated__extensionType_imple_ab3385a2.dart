// %before-language-feature: augmentations
typedef A = int;
extension type B(int it) implements int, A {}
//                                       ^
// [diag.implementsRepeated] 'int' can only be implemented once.
