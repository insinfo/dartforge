// R-GEN-13: instanciação para o limite com limites que dependem de outros
// parâmetros (algoritmo por componentes fortemente conexos).
class A<T extends U, U extends num> {}

class O<T extends Object?> {}

void main() {
  A a = /*@*/A();
  var o = /*@*/O();
  List<A> l = /*@*/[];
  print([a, o, l]);
}
