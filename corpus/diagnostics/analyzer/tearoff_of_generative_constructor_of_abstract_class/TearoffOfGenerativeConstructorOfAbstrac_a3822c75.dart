abstract class A {
  A();
}

void foo() {
  A.new;
//^^^^^
// [diag.tearoffOfGenerativeConstructorOfAbstractClass] A generative constructor of an abstract class can't be torn off.
}
