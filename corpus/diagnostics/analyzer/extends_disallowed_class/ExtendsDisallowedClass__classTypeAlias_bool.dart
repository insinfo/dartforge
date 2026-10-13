class M {}
class C = bool with M;
//        ^^^^
// [diag.extendsDisallowedClass] Classes can't extend 'bool'.
