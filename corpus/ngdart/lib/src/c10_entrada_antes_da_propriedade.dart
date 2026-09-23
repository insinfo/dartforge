import 'package:ngdart/angular.dart';

/// Propriedade de elemento antes de um `*ngIf` no template: na detecção, a
/// entrada da diretiva e as visões aninhadas vêm antes das ligações de
/// propriedade (`writeChangeDetectionStatements`).
@Component(
  selector: 'c10-entrada-antes-da-propriedade',
  templateUrl: 'c10_entrada_antes_da_propriedade.html',
  directives: [coreDirectives],
)
class C10EntradaAntesDaPropriedade {
  String titulo = 'x';
  bool mostrar = true;
}