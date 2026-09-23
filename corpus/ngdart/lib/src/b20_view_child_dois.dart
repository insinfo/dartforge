import 'dart:html';

import 'package:ngdart/angular.dart';

/// Dois `@ViewChild` declarados em ordem diferente da do template, um deles
/// num elemento que vira campo, e uma referência que ninguém consulta.
@Component(
  selector: 'b20-view-child-dois',
  templateUrl: 'b20_view_child_dois.html',
)
class B20ViewChildDois {
  @ViewChild('segundo')
  DivElement? segundo;

  @ViewChild('primeiro')
  Element? primeiro;

  String titulo = 'x';

  void clicou() {}
}