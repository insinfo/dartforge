class A {}
mixin class B extends A {}
//                    ^
// [diag.mixinClassDeclarationExtendsNotObject] The class 'B' can't be declared a mixin because it extends a class other than 'Object'.
