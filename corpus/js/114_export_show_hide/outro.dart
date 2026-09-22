// Outra biblioteca: api.dart reexporta tudo menos Z.
class Z {
  @override
  String toString() => 'Z oculta';
}

class W {
  final String s;
  W(this.s);
  @override
  String toString() => 'W($s)';
}

String saudacao(String nome) => 'olá, $nome';

const limite = 42;

enum Estado { ligado, desligado }

var mutavel = 'inicial';
