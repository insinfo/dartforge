class A {}

class B([super.a]) extends A {}
//             ^
// [diag.superFormalParameterWithoutAssociatedPositional] No associated positional super constructor parameter.
