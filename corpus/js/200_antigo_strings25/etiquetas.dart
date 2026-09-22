// Biblioteca separada: garante que nomes usados dentro de `${...}` continuam
// resolvidos e qualificados depois da combinação das unidades pelo linker.
String etiqueta() {
  return 'lib';
}

int dobro(int valor) {
  return valor + valor;
}
