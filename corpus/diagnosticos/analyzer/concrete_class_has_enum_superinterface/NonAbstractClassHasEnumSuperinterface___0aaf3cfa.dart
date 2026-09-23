mixin M {}
abstract class A implements Enum {}
class B = Object with M implements A;
//    ^
// [diag.concreteClassHasEnumSuperinterface] Concrete classes can't have 'Enum' as a superinterface.
