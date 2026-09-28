import 'package:ngdart/angular.dart';

/// Vários componentes com folha de estilo no mesmo arquivo (o
/// `li-accordion`/`li-accordion-item` do limitless_ui): cada um com o
/// import da sua folha na tabela única do arquivo.
@Component(
  selector: 'j44-primeiro',
  template: '<b>1</b>',
  styleUrls: ['j44_primeiro.css'],
)
class J44Primeiro {}

@Component(
  selector: 'j44-segundo',
  template: '<i>2</i><j44-primeiro></j44-primeiro>',
  styles: ['i { color: red; }'],
  directives: [J44Primeiro],
)
class J44Segundo {}

@Component(
  selector: 'j44-terceiro',
  template: '<u>3</u><j44-segundo></j44-segundo>',
  styleUrls: ['j44_terceiro.css', 'j44_primeiro.css'],
  directives: [J44Segundo],
)
class J44Terceiro {}
