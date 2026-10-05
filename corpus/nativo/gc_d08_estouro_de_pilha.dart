// D8 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): o `StackOverflowError`
// capturado, três vezes, e depois de cada um uma alocação grande. A
// recursão não é de cauda (o incremento depois da chamada impede o LLVM de
// virar laço), e nenhum quadro chega ao incremento: todos saem pela exceção.
//
// Saída: 3003 0

var profundidade = 0;

int fundo(int n) {
  final r = fundo(n + 1);
  profundidade++;
  return r;
}

void main() {
  var capturados = 0;
  for (var k = 0; k < 3; k++) {
    try {
      fundo(0);
    } on StackOverflowError {
      capturados++;
    }
    final l = <String>[];
    for (var i = 0; i < 1000; i++) {
      l.add('v$i');
    }
    capturados += l.length;
  }
  print('$capturados $profundidade');
}
