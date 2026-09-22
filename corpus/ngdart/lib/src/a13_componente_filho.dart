import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';
import 'a02_texto_estatico.template.dart';

/// Um componente dentro do template de outro: casa o seletor, instancia a
/// visão-filha e liga a detecção de mudança.
@Component(
  selector: 'a13-componente-filho',
  templateUrl: 'a13_componente_filho.html',
  directives: [A02TextoEstatico],
)
class A13ComponenteFilho {}
