class M {}
class A = Object with M implements Enum;
//                                 ^^^^
// [diag.concreteClassHasEnumSuperinterface] Concrete classes can't have 'Enum' as a superinterface.
