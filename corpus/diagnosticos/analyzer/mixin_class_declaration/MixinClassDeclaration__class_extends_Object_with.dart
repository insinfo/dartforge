mixin M {}
mixin class A extends Object with M {}
//                           ^^^^^^
// [diag.mixinClassDeclarationWithClause] The class 'A' can't be declared a mixin because it has a 'with' clause.
