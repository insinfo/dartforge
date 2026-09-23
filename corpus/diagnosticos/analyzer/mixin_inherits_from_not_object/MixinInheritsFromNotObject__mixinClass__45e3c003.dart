mixin M {}
mixin class A with M {}
//            ^^^^^^
// [diag.mixinClassDeclarationWithClause] The class 'A' can't be declared a mixin because it has a 'with' clause.
