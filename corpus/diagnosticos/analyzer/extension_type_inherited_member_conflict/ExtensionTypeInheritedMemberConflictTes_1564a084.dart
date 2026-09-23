extension type A(String bar) {}
//                      ^^^
// [context 1] Inherited from 'A'

extension type B(String bar) {}
//                      ^^^
// [context 2] Inherited from 'B'

extension type C(String foo) implements A, B {}
//             ^
// [diag.extensionTypeInheritedMemberConflict][context 1][context 2] The extension type 'C' has more than one distinct member named 'bar' from implemented types.
