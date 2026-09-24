// %before-language-feature: augmentations
extension type A(int it) implements int, int {}
//                                       ^^^
// [diag.implementsRepeated] 'int' can only be implemented once.
