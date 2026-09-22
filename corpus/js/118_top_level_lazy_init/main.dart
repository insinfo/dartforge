// Inicialização preguiçosa: top-levels e estáticos com inicializador que imprime, ordem de primeira leitura, late, const, top-level de outra lib.
import 'outra.dart';

int registra(String s, int v) {
  print('  init $s');
  return v;
}

final primeiro = registra('primeiro', 1);
var segundo = registra('segundo', 2);
final terceiro = registra('terceiro', primeiro + segundo);
late int quarto = registra('quarto', 4);
late final quinto = registra('quinto', 5);
const sexto = 6;
final lista = [registra('lista[0]', 7), registra('lista[1]', 8)];
final naoLido = registra('naoLido NUNCA', -1);
int? anulavel = registra('anulavel', 0);

class Estaticos {
  static final a = registra('Estaticos.a', 10);
  static int b = registra('Estaticos.b', 20);
  static late int c = registra('Estaticos.c', 30);
  static const d = 40;
  static final dependente = registra('Estaticos.dependente', a + b);
  static int metodo() => registra('Estaticos.metodo', 50);
}

int comExcecao() {
  print('  init lancador');
  throw StateError('falhou na inicialização');
}

late int lancador = comExcecao();

void main() {
  print('main início');
  print('const: $sexto ${Estaticos.d} $deOutraConst');
  print('lendo terceiro:');
  print(terceiro);
  print('lendo primeiro (já inicializado):');
  print(primeiro);
  print('lendo segundo:');
  print(segundo);
  segundo = 22;
  print(segundo);
  print('--');
  print('quarto antes de ler: escrevendo');
  quarto = 44;
  print(quarto);
  print('quinto:');
  print(quinto);
  print(quinto);
  print('lista:');
  print(lista);
  print(anulavel);
  print('--');
  print(Estaticos.dependente);
  print(Estaticos.a);
  Estaticos.b = 21;
  print(Estaticos.b);
  print(Estaticos.c);
  print(Estaticos.metodo());
  print('--');
  try {
    print(lancador);
  } catch (e) {
    print('capturado: $e');
  }
  try {
    print(lancador);
  } catch (e) {
    print('capturado de novo (reexecuta): $e');
  }
  print('--');
  print('outra lib:');
  print(deOutraLibVar);
  print(deOutraLib);
  print(Config.fixo);
  print(Config.tardia);
  print(Config.carregado);
  print(deOutraLate);
  print(Config.mutavel);
  Config.mutavel = 0;
  print(Config.mutavel);
  print(deOutraLib);
  print('fim');
}
