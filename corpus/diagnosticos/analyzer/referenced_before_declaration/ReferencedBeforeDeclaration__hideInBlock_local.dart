var v = 1;
main() {
  print(v);
//      ^
// [diag.referencedBeforeDeclaration][context 1] Local variable 'v' can't be referenced before it is declared.
  var v = 2;
//    ^
// [context 1] The declaration of 'v' is here.
}
print(x) {}
