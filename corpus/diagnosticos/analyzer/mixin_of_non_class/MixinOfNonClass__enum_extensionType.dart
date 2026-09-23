extension type A(int it) {}
enum E with A { v }
//          ^
// [diag.mixinOfNonClass] Classes can only mix in mixins and classes.
