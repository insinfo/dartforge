import 'dart:html';

import 'package:ngdart/angular.dart';

@Component(
  selector: 'j81-editor',
  template: '<i></i>',
)
class J81Editor {}

/// `@ViewChild` em setter (o `editorRef` da página do editor Quill), junto
/// de campos: os setters vêm antes na ordem das consultas.
@Component(
  selector: 'j81-view-child-em-setter',
  templateUrl: 'j81_view_child_em_setter.html',
  directives: [J81Editor, NgIf],
)
class J81ViewChildEmSetter {
  bool mostrar = true;

  @ViewChild('caixa')
  DivElement? caixa;

  J81Editor? editor;

  @ViewChild('editor')
  set editorRef(J81Editor? valor) {
    editor = valor;
  }

  @ViewChild('dentro')
  set dentroRef(Element? valor) {}
}
