class A {
  int v;
  this : v = 0;
//^^^^
// [diag.primaryConstructorBodyWithoutDeclaration] A primary constructor body requires a primary constructor declaration.
}
