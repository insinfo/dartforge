// while e do-while: condição falsa de início, break/continue, efeitos na condição, contador decrescente.
int chamadas = 0;

bool condicao(int i) {
  chamadas++;
  print('condicao($i)');
  return i < 3;
}

void main() {
  // while simples
  var i = 0;
  while (i < 4) {
    print('while $i');
    i++;
  }

  // while com condição falsa: nunca executa
  while (i < 0) {
    print('nunca');
  }
  print('depois de while falso, i=$i');

  // do-while executa ao menos uma vez
  var j = 100;
  do {
    print('do-while $j');
    j++;
  } while (j < 50);

  // do-while normal
  var k = 0;
  do {
    k += 2;
  } while (k < 7);
  print('k=$k');

  // break e continue no while
  var n = 0;
  while (true) {
    n++;
    if (n % 2 == 0) continue;
    if (n > 7) break;
    print('ímpar $n');
  }
  print('n final $n');

  // continue no do-while ainda avalia a condição
  var m = 0;
  do {
    m++;
    if (m == 2) continue;
    print('m=$m');
  } while (m < 4);

  // condição com efeitos colaterais
  var c = 0;
  while (condicao(c)) {
    c++;
  }
  print('chamadas=$chamadas');

  // contador decrescente
  var d = 5;
  while (d > 0) {
    print('desce $d');
    d -= 2;
  }
  print('d=$d');

  // while acumulando string
  var s = '';
  var q = 0;
  while (s.length < 5) {
    s += q.toString();
    q++;
  }
  print(s);

  // while aninhado
  var linha = 0;
  while (linha < 3) {
    var col = 0;
    var texto = '';
    while (col <= linha) {
      texto += '*';
      col++;
    }
    print(texto);
    linha++;
  }

  // do-while com break imediato
  do {
    print('uma vez');
    break;
  } while (true);
  print('fim');
}
