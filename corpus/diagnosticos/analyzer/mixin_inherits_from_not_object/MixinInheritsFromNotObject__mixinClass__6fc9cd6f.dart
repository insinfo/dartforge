mixin class A {}
mixin class B extends Object with A {}
//                           ^^^^^^
// [diag.mixinClassDeclarationWithClause] The class 'B' can't be declared a mixin because it has a 'with' clause.
