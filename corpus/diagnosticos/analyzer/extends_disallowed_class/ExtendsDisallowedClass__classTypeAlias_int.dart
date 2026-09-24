class M {}
class C = int with M;
//        ^^^
// [diag.extendsDisallowedClass] Classes can't extend 'int'.
