// A leitura de uma `const` primitiva (`int`, `double`, `bool`) vira a
// constante, sem o getter do global (`lower/const_primitiva.rs`): o
// `switch` sobre estados `static const int` (o molde do `_HttpParser`),
// literais hexadecimais, negativos, aritmética entre constantes, cadeias de
// referências, `double` escrito com literal inteiro, `num`/`Object`/`dynamic`
// (ficam com o getter) e `identical`. A saída tem de ser a da VM.

const int topo = 7;
const int derivado = (topo << 3) - 1; // 55
const negativo = -0x10;
const bool ligado = true;
const double meio = 0.5;
const double umDouble = 1; // literal inteiro no contexto double
const num umNum = 3;
const Object umObjeto = 4;
const dynamic umDinamico = 5;
const int maior = 0x7fffffffffffffff;
const int menor = -9223372036854775808;
const int mascara = (1 << 11) - 1;
const int combinado = mascara & 0x3f0 | 0x5 ^ 0x1;

class _Estado {
  static const int inicio = 0;
  static const int metodo = 1;
  static const int uri = 2;
  static const int versao = 3;
  static const int cabecalho = inicio + 10;
  static const int fim = cabecalho + 1;
  static const proximo = fim + _Codigo.lf;
}

class _Codigo {
  static const int lf = 10;
  static const int cr = 13;
  static const int sp = 32;
  static const bool cedo = !false == true; // não é aritmética de int: getter
}

String passo(int estado, int byte) {
  switch (estado) {
    case _Estado.inicio:
      return byte == _Codigo.sp ? 'inicio/sp' : 'inicio';
    case _Estado.metodo:
      return 'metodo';
    case _Estado.uri:
    case _Estado.versao:
      return 'uri-ou-versao';
    case _Estado.cabecalho:
      if (byte == _Codigo.cr) return 'cabecalho/cr';
      if (byte == _Codigo.lf) return 'cabecalho/lf';
      return 'cabecalho';
    case _Estado.fim:
      return 'fim';
    case _Estado.proximo:
      return 'proximo';
    default:
      return 'outro($estado)';
  }
}

int maquina(List<int> bytes) {
  var estado = _Estado.inicio;
  var n = 0;
  for (final b in bytes) {
    switch (estado) {
      case _Estado.inicio:
        estado = b == _Codigo.sp ? _Estado.metodo : _Estado.inicio;
      case _Estado.metodo:
        estado = _Estado.uri;
      case _Estado.uri:
        estado = b == _Codigo.cr ? _Estado.cabecalho : _Estado.uri;
      case _Estado.cabecalho:
        estado = b == _Codigo.lf ? _Estado.fim : _Estado.cabecalho;
      default:
        n++;
        estado = _Estado.inicio;
    }
  }
  return n * 100 + estado;
}

double metade(double x) => x * meio;

void main() {
  print([topo, derivado, negativo, ligado, meio, umDouble, umNum, umObjeto, umDinamico]);
  print([maior, menor, mascara, combinado, maior + 1 == menor]);
  print('${umDouble.runtimeType} ${umNum.runtimeType} ${umObjeto.runtimeType} ${umDinamico.runtimeType}');
  print('${umDouble is double} ${umDouble is int} ${umNum is int}');
  print([_Estado.cabecalho, _Estado.fim, _Estado.proximo, _Codigo.cedo]);
  for (final e in [0, 1, 2, 3, 10, 11, 21, 99]) {
    print('$e ${passo(e, 32)} ${passo(e, 13)} ${passo(e, 10)}');
  }
  print(maquina('GET / HTTP/1.1\r\nHost: x\r\n\r\n'.codeUnits));
  print(identical(topo, 7));
  print(identical(meio, 0.5));
  print(identical(ligado, true));
  print(metade(umDouble));
  Object o = topo;
  print(o is int ? 'int ${o + 1}' : 'outro');
  dynamic d = derivado;
  print(d.toRadixString(16));
  print(topo.hashCode == 7.hashCode);
  final lista = <int>[_Estado.inicio, _Estado.metodo, _Codigo.lf];
  print(lista);
  final mapa = {_Codigo.cr: 'cr', _Codigo.lf: 'lf'};
  print(mapa[13]);
  const composta = [topo, derivado, _Estado.fim];
  print(identical(composta, const [7, 55, 11]));
  print(ligado ? 'sim' : 'nao');
  print(negativo.abs());
}
