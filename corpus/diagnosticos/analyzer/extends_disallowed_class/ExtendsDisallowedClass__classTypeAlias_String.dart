class M {}
class C = String with M;
//        ^^^^^^
// [diag.extendsDisallowedClass] Classes can't extend 'String'.
