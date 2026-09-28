import 'package:ngdart/angular.dart';

/// Três ou mais `{{ }}` num valor: sempre `interpolateN([...])`
/// (`interpolateFallback`), mesmo com tudo `String`.
@Component(
  selector: 'j113-alvo',
  template: '<i>{{ rotulo }}</i>',
)
class J113Alvo {
  @Input()
  String? rotulo;
}

@Component(
  selector: 'j113-interpolacao-n',
  template: '''
<div title="{{a}}-{{b}}-{{c}}" attr.aria-label="x {{a}} {{n}} {{b}} y"></div>
<j113-alvo rotulo="{{a}}{{b}}{{c}}"></j113-alvo>''',
  directives: [J113Alvo],
)
class J113InterpolacaoN {
  String a = 'a';
  String b = 'b';
  String c = 'c';
  int n = 1;
}
