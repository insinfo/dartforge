// D12 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): os caches estáticos
// do runtime que guardam handles têm de ser raiz. O objeto `Type` canônico
// (`dartforge_rti_objeto_tipo`, o cache `OBJETOS_TIPO`) e o texto de um
// caractere (`String_charAt`, o cache `UM_CARACTERE`) são pedidos de novo
// a cada volta, sem que nada do programa os segure entre as voltas; com o
// estresse, cada volta coleta. Sabotagem que o derruba: `tipo_sem_raiz` (o
// runtime não registra o `Type` como raiz; a volta seguinte recebe do cache
// um handle para bloco liberado, e a validação de handle o recusa).
//
// Saída: 1000 1000 4000

class A {}

class B {}

void main() {
  var contaA = 0;
  var contaB = 0;
  var iguais = 0;
  for (var i = 0; i < 2000; i++) {
    final t = i.isEven ? A : B;
    // Aloca: sob estresse, coleta com `t` vivo.
    final lixo = 'x$i';
    if (identical(t, A)) contaA++;
    if (identical(t, B)) contaB++;
    if (identical(t, i.isEven ? A : B) && lixo.isNotEmpty) iguais++;
    final c = 'abc$i'[1];
    if (identical(c, 'b')) iguais++;
  }
  print('$contaA $contaB $iguais');
}
