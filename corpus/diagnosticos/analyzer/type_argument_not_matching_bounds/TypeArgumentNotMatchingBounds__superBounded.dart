class A<X extends A<X>> {}

A get foo => throw 0;
