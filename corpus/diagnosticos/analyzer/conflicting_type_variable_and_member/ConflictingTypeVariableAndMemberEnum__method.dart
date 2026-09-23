enum A<T> {
//     ^
// [diag.conflictingTypeVariableAndMemberEnum] 'T' can't be used to name both a type parameter and a member in this enum.
  v;
  void T() {}
}
