import 'package:ngdart/angular.dart';

@Component(
  selector: 'j52-filho',
  templateUrl: 'j52_filho.html',
  directives: [NgTemplateOutlet],
)
class J52Filho {
  @Input()
  TemplateRef? modelo;

  Map<String, Object?> contexto = {r'$implicit': 1};
}
