class Base { int value = 1; }
class C extends Base {}
class Outer { C get inner => C(); }
