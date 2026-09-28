import 'package:ngdart/angular.dart';

import 'd07_filho_on_push.dart';
import 'j23_abas_on_push.dart';

/// Conteúdo com dois componentes `onPush` achados pelas consultas do filho:
/// o `ChangeDetectorRef` de cada resultado é registrado antes da
/// atribuição.
@Component(
  selector: 'j24-usa-abas-on-push',
  templateUrl: 'j24_usa_abas_on_push.html',
  directives: [J23AbasOnPush, D07FilhoOnPush],
)
class J24UsaAbasOnPush {}
