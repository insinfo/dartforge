// rótulos: break/continue para laço externo, rótulo em bloco, em while/do/for-in, switch dentro de laço rotulado.
void main() {
  // break externo
  externo:
  for (var i = 0; i < 3; i++) {
    for (var j = 0; j < 3; j++) {
      if (i == 1 && j == 1) break externo;
      print('break-ext $i,$j');
    }
  }

  // continue externo
  linhas:
  for (var i = 0; i < 3; i++) {
    for (var j = 0; j < 3; j++) {
      if (j == 1) continue linhas;
      print('cont-ext $i,$j');
    }
    print('nunca chega aqui');
  }

  // rótulo em bloco
  bloco:
  {
    print('dentro do bloco');
    if (true) break bloco;
    print('não impresso');
  }
  print('depois do bloco');

  // rótulo em while
  var n = 0;
  fora:
  while (n < 10) {
    n++;
    var m = 0;
    while (m < 10) {
      m++;
      if (m == 2) continue fora;
      if (n == 4) break fora;
      print('while $n,$m');
    }
  }
  print('n=$n');

  // rótulo em do-while
  var d = 0;
  rotDo:
  do {
    d++;
    for (var q = 0; q < 5; q++) {
      if (q == 1) continue rotDo;
      print('do $d,$q');
    }
  } while (d < 3);
  print('d=$d');

  // rótulo em for-in
  itens:
  for (final item in ['a', 'b', 'c']) {
    for (final sub in [1, 2, 3]) {
      if (item == 'b') continue itens;
      if (sub == 3) break itens;
      print('$item$sub');
    }
  }

  // break dentro de switch dentro de laço rotulado
  laco:
  for (var i = 0; i < 6; i++) {
    switch (i) {
      case 1:
        continue laco;
      case 3:
        break laco;
      default:
        print('switch $i');
    }
    print('depois switch $i');
  }

  // break simples em switch só sai do switch
  for (var i = 0; i < 3; i++) {
    switch (i) {
      case 1:
        break;
      default:
        print('caso $i');
    }
    print('pós $i');
  }

  // rótulo em bloco aninhado dentro de laço
  for (var i = 0; i < 3; i++) {
    interno:
    {
      if (i == 1) break interno;
      print('bloco i=$i');
    }
    print('fim iteração $i');
  }

  // três níveis
  tres:
  for (var a = 0; a < 2; a++) {
    for (var b = 0; b < 2; b++) {
      for (var c = 0; c < 2; c++) {
        if (c == 1) continue tres;
        print('$a$b$c');
      }
    }
  }
  print('fim');
}
