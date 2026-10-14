class Base { int get value => 1; }
class C extends Base {}
class Outer { C get inner => C(); }
int read(Outer o) { if (o case Outer(inner:C(:final value))) { return value; } return 0; }
