mixin class A {}
mixin class B extends Object with A {}
//                           ^^^^^^
// [diag.mixinClassDeclarationWithClause] The class 'B' can't be declared a mixin because it has a 'with' clause.
class C extends Object with B {}
//                          ^
// [diag.mixinInheritsFromNotObject] The class 'B' can't be used as a mixin because it extends a class other than 'Object'.
