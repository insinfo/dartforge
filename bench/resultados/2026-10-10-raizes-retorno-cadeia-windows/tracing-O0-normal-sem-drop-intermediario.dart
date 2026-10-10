Object? identidade(Object? valor) => valor;
Object? repassar(Object? valor) => identidade(identidade(valor));
void main() {}
