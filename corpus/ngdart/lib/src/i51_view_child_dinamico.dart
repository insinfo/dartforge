import 'dart:html';

import 'package:ngdart/angular.dart';

/// Sonda: `@ViewChildren` em `*ngFor` e `@ViewChild` em `*ngIf`, depois de um estático e entre ligações de texto.
@Component(
  selector: 'i51-view-child-dinamico',
  templateUrl: 'i51_view_child_dinamico.html',
  directives: [coreDirectives],
)
class I51ViewChildDinamico {
  int n = 1;
  bool mostrar = true;
  List<String> itens = ['a'];
  @ViewChild('fixo')
  Element? fixo;
  @ViewChildren('item')
  List<Element>? todos;
  @ViewChild('campo')
  InputElement? campo;
}
