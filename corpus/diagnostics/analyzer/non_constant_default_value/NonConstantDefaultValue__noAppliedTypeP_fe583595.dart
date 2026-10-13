void f<T>(T t) => t;

void bar<T>([void Function<T>(T) p = f]) {}
