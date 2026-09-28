import 'dart:html';

import 'package:ngdart/angular.dart';

/// Um `#ref` em várias visões: um resultado estático, um em `*ngIf` e um
/// em cada item do `*ngFor` — a lista junta todos, na ordem do template, e o
/// `@ViewChild` fica com o primeiro.
@Component(
  selector: 'j17-varios-resultados',
  templateUrl: 'j17_varios_resultados.html',
  directives: [coreDirectives],
)
class J17VariosResultados {
  bool a = true;
  List<int> itens = [1, 2];

  @ViewChildren('s')
  List<Element>? todos;

  @ViewChild('s')
  Element? primeiro;
}
