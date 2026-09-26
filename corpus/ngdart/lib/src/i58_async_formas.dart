import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';

/// Sonda: `async` (impuro, `OnDestroy`) junto de pipe puro, de filho e de `*ngIf`.
@Component(
  selector: 'i58-async-formas',
  templateUrl: 'i58_async_formas.html',
  directives: [coreDirectives, A02TextoEstatico],
  pipes: [commonPipes],
)
class I58AsyncFormas {
  String nome = 'x';
  bool mostrar = true;
  Stream<String> fluxo = Stream.value('a');
  Future<String> futuro = Future.value('b');
}
