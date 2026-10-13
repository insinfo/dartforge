// requer-dart: 3.13
// Escopo primário (spec :817-890): os inicializadores de campo não-`late` e a
// lista de inicializadores veem os PARÂMETROS; o corpo da parte `this` e os
// campos `late` veem o CAMPO (ou o que estiver fora da classe).
String x = 'top level';

class Cap(var String x) {
  void Function() naDeclaracao = () => print(x);
  void Function() noInicializador;
  void Function()? noCorpo;

  this : noInicializador = (() => print(x)) {
    noCorpo = () => print(x);
  }
}

class Tarde(String x) {
  String instancia = x;
  late String tardia = x;
}

class Contador(int inicio) {
  int valor = inicio * 10;
  this {
    valor += 1;
  }
}

void main() {
  var c = Cap('parâmetro');
  c.x = 'atualizado';
  c.naDeclaracao();
  c.noInicializador();
  c.noCorpo!();
  var t = Tarde('parâmetro');
  print(t.instancia);
  print(t.tardia);
  print(Contador(4).valor);
}
