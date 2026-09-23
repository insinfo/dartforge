// R-MEM-10: cascata tem o tipo do alvo; `this`, `super`.
class A {
  String nome() => 'a';
}

class B extends A {
  @override
  String nome() => /*@*/super.nome();
  B eu() => /*@*/this;
}

void main() {
  var l = /*@*/[1]..add(2)..length;
  var s = /*@*/StringBuffer()..write('a');
  print([l, s, B().eu()]);
}
