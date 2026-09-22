import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';
import 'a02_texto_estatico.template.dart';

/// Componente filho dentro de um elemento, com irmãos.
@Component(
  selector: 'd02-filho-aninhado',
  templateUrl: 'd02_filho_aninhado.html',
  directives: [A02TextoEstatico],
)
class D02FilhoAninhado {}
