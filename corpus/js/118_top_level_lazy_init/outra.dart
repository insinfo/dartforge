// Outra biblioteca com top-levels preguiçosos lidos pela primeira vez a partir do main.
int _n = 0;

int marca(String s) {
  _n++;
  print('  init $s (#$_n)');
  return _n;
}

final deOutraLib = marca('deOutraLib');
var deOutraLibVar = marca('deOutraLibVar');
late final int deOutraLate = marca('deOutraLate');
const deOutraConst = 'const de outra';

class Config {
  static final int carregado = marca('Config.carregado');
  static int mutavel = marca('Config.mutavel');
  static const fixo = 99;
  static late String tardia = 'tardia ${marca('Config.tardia')}';
}
