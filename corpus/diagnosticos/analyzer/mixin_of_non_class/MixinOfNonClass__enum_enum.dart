enum E1 { v }
enum E2 with E1 { v }
//           ^^
// [diag.mixinOfNonClass] Classes can only mix in mixins and classes.
