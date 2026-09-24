import 'dart:ffi';
class C extends Finalizable {}
//              ^^^^^^^^^^^
// [diag.noGenerativeConstructorsInSuperclass] The class 'C' can't extend 'Finalizable' because 'Finalizable' only has factory constructors (no generative constructors), and 'C' has at least one generative constructor.
