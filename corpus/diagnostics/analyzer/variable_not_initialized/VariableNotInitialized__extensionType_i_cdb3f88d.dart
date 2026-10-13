extension type A(int it) {
//             ^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'v' isn't.
  final int v;
//          ^
// [diag.extensionTypeDeclaresInstanceField] Extension types can't declare instance fields.
}
