abstract class F<T>  {
  T get value;
}

abstract class G<U> {
  U test(F<U> arg) => arg.value;
}
