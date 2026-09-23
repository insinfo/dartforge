void testTypeRef() {
  String s = '';
//^^^^^^
// [diag.referencedBeforeDeclaration][context 1] Local variable 'String' can't be referenced before it is declared.
  int String(int x) => x + 1;
//    ^^^^^^
// [context 1] The declaration of 'String' is here.
  print(s + String);
}
