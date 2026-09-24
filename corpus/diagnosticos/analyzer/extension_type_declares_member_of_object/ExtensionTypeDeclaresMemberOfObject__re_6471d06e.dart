extension type E0(Object? hashCode) {}
//                        ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
extension type E1(Object? noSuchMethod) {}
//                        ^^^^^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
extension type E2(Object? runtimeType) {}
//                        ^^^^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
extension type E3(Object? toString) {}
//                        ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
