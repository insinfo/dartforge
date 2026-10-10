// Exercita palavras escalares e referências, inclusive após a posição 32.
class Base {
  int inteiro = 1;
  double fracao = 2.5;
  bool ligado = false;
  Object? referencia;
  late int pendente;
  late final double unico;
}

mixin Mistura { bool misto = false; }

class Folha extends Base with Mistura {
  int p00 = 0, p01 = 0, p02 = 0, p03 = 0, p04 = 0;
  int p05 = 0, p06 = 0, p07 = 0, p08 = 0, p09 = 0;
  int p10 = 0, p11 = 0, p12 = 0, p13 = 0, p14 = 0;
  int p15 = 0, p16 = 0, p17 = 0, p18 = 0, p19 = 0;
  int p20 = 0, p21 = 0, p22 = 0, p23 = 0, p24 = 0;
  int p25 = 0, p26 = 0, p27 = 0, p28 = 0, p29 = 0;
  double extensa = 3.5;
  bool extenso = false;
  Object? externo;
}

enum Cor { azul, vermelho }

Folha? publicada;

void conferir(bool resultado) {
  if (!resultado) throw StateError('campo incorreto');
}

void main() {
  final objeto = Folha();
  publicada = objeto;
  final valor = Object();
  for (var i = 0; i < 3; i++) {
    objeto.inteiro = 9223372036854775807;
    objeto.fracao = -0.0;
    objeto.ligado = true;
    objeto.referencia = valor;
    objeto.misto = true;
    objeto.extensa = -0.0;
    objeto.extenso = true;
    objeto.externo = valor;
    conferir(objeto.inteiro == 9223372036854775807);
    conferir(1.0 / objeto.fracao == double.negativeInfinity);
    conferir(objeto.ligado && objeto.misto && objeto.extenso);
    conferir(1.0 / objeto.extensa == double.negativeInfinity);
    conferir(identical(objeto.referencia, valor));
    conferir(identical(objeto.externo, valor));
    objeto.referencia = null;
    objeto.externo = null;
    objeto.extenso = false;
    conferir(objeto.referencia == null && objeto.externo == null);
    conferir(!objeto.extenso);
  }
  var erros = 0;
  try { print(objeto.pendente); } catch (_) { erros++; }
  objeto.pendente = 17;
  objeto.unico = 4.5;
  try { objeto.unico = 5.5; } catch (_) { erros++; }
  conferir(erros == 2 && objeto.pendente == 17 && objeto.unico == 4.5);
  conferir(Cor.vermelho.index == 1 && Cor.azul.name == 'azul');
  print('campos tipados: ok');
}
