// Nada a gerar: o motor de build não deve fazer trabalho nenhum aqui.
int fatorial(int n) => n <= 1 ? 1 : n * fatorial(n - 1);

List<int> primos(int limite) => [
      for (var i = 2; i <= limite; i++)
        if (Iterable.generate(i - 2, (k) => k + 2).every((d) => i % d != 0)) i,
    ];
