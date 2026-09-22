// Consome os `.template.dart` do corpus. O builder do ngdart é
// `is_optional: true`: sem alguém pedindo a saída, ele não roda.
import 'package:corpus_ngdart/src/a01_interpolacao.template.dart' as a01;
import 'package:corpus_ngdart/src/a02_texto_estatico.template.dart' as a02;

void main() {
  print([a01.A01InterpolacaoNgFactory, a02.A02TextoEstaticoNgFactory].length);
}
