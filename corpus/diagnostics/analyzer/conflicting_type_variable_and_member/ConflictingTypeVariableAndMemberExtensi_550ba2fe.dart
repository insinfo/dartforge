extension type A<T>(int it) {
//               ^
// [diag.conflictingTypeVariableAndMemberExtensionType] 'T' can't be used to name both a type parameter and a member in this extension type.
  A.T(int it) : this(it);
}
