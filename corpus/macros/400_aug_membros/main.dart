// experimentos: macros
// Augmentation escrita à mão na forma do SDK 3.6 (`import augment` +
// `augment library`), que o CFE 3.6.2 aceita com --enable-experiment=macros:
// corpo para método `external`, getter e setter, operador, método novo,
// campo novo e função de topo completada. (O CFE 3.6.2 ignora `implements` e
// `with` numa `augment class`; as cláusulas estão no 405, contra o 3.13.4.)
import augment 'membros_aug.dart';

class Conta {
  final String dono;
  int _saldo = 0;
  Conta(this.dono);

  external void depositar(int valor);
  external int get saldo;
  external set limite(int v);
  external Conta operator +(int valor);
  String resumo() => '$dono: $_saldo';
}

external String saudacao(String nome);

void main() {
  var c = Conta('Ana');
  c.depositar(10);
  c.depositar(5);
  print(c.saldo);
  c.limite = 3;
  print(c.resumo());
  c = c + 7;
  print(c.saldo);
  print(c.dobro());
  print(c.historico);
  print(c.descrever());
  print(saudacao('mundo'));
}
