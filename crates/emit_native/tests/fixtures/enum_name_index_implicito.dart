enum Planeta {
  mercurio,
  terra;

  String descreve() => '$name:$index';
  String esteNome() => this.name;
  int esteIndice() => this.index;
}

void main() {
  print(Planeta.terra.descreve());
  print(Planeta.mercurio.descreve());
  print(Planeta.terra.esteNome());
  print(Planeta.mercurio.esteIndice());
  print(Planeta.terra.name);
  print(Planeta.mercurio.index);
}
