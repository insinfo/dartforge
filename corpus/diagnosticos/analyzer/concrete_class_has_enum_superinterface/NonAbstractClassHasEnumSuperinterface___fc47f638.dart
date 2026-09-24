abstract class A implements Enum {}
class B implements A {}
//    ^
// [diag.concreteClassHasEnumSuperinterface] Concrete classes can't have 'Enum' as a superinterface.
