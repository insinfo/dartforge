enum E { ONE }
class A extends Object with E {}
//                          ^
// [diag.mixinOfNonClass] Classes can only mix in mixins and classes.
