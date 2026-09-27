import 'package:ngdart/angular.dart';

/// `#ref` com o nome de um membro do componente: a leitura no template é o
/// local (o nó), não o membro.
@Component(
  selector: 'j13-ref-com-membro',
  templateUrl: 'j13_ref_com_membro.html',
  directives: [coreDirectives],
)
class J13RefComMembro {
  Object? campo = 'membro';
  bool mostrar = true;
}
