// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j59_interpolado_em_diretiva.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j59_interpolado_em_diretiva.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$J59InterpoladoEmDiretiva = const [];

class ViewJ59InterpoladoEmDiretiva0 extends import0.ComponentView<import1.J59InterpoladoEmDiretiva> {
  late final import1.J59Rotulo _J59Rotulo_0_5;
  late final import1.J59Rotulo _J59Rotulo_3_5;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  late final import2.HtmlElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewJ59InterpoladoEmDiretiva0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j59-interpolado-em-diretiva'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j59_interpolado_em_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendSpan(doc, parentRenderNode);
    this._J59Rotulo_0_5 = import1.J59Rotulo();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(this._el_0, this._J59Rotulo_0_5);
    }
    final _text_1 = import7.appendText(this._el_0, 'a');
    final _text_2 = import7.appendText(parentRenderNode, '\n');
    final _el_3 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'i');
    this._J59Rotulo_3_5 = import1.J59Rotulo();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_3, this._J59Rotulo_3_5);
    }
    final _text_4 = import7.appendText(_el_3, 'b');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_1 = import9.interpolateString0(_ctx.nome);
    if (import10.checkBinding(this._expr_1, currVal_1, '{{ nome }}', 'package:corpus_ngdart/src/j59_interpolado_em_diretiva.html')) {
      if (import8.isDevToolsEnabled) {
        import8.Inspector.instance.recordInput(this._J59Rotulo_0_5, 'j59-rotulo', currVal_1);
      }
      this._J59Rotulo_0_5.rotulo = currVal_1 /* REF:package:corpus_ngdart/src/j59_interpolado_em_diretiva.html:6:29 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.total;
    if (import10.checkBinding(this._expr_2, currVal_2, '{{total}}', 'package:corpus_ngdart/src/j59_interpolado_em_diretiva.html')) {
      if (import8.isDevToolsEnabled) {
        import8.Inspector.instance.recordInput(this._J59Rotulo_0_5, 'valor', import9.interpolate0(currVal_2));
      }
      this._J59Rotulo_0_5.valor = import9.interpolate0(currVal_2) /* REF:package:corpus_ngdart/src/j59_interpolado_em_diretiva.html:30:47 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = import9.interpolateString1('x ', _ctx.nome, ' y');
    if (import10.checkBinding(this._expr_3, currVal_3, 'x {{nome}} y', 'package:corpus_ngdart/src/j59_interpolado_em_diretiva.html')) {
      if (import8.isDevToolsEnabled) {
        import8.Inspector.instance.recordInput(this._J59Rotulo_3_5, 'j59-rotulo', currVal_3);
      }
      this._J59Rotulo_3_5.rotulo = currVal_3 /* REF:package:corpus_ngdart/src/j59_interpolado_em_diretiva.html:79:104 */;
      this._expr_3 = currVal_3;
    }
    final currVal_0 = import9.interpolateString1('t ', _ctx.nome, '');
    if (import10.checkBinding(this._expr_0, currVal_0, 't {{nome}}', 'package:corpus_ngdart/src/j59_interpolado_em_diretiva.html')) {
      import7.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/j59_interpolado_em_diretiva.html:48:66 */;
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J59InterpoladoEmDiretiva, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J59InterpoladoEmDiretivaNgFactory = ComponentFactory<import1.J59InterpoladoEmDiretiva>('j59-interpolado-em-diretiva', viewFactory_J59InterpoladoEmDiretivaHost0);
ComponentFactory<import1.J59InterpoladoEmDiretiva> get J59InterpoladoEmDiretivaNgFactory {
  return _J59InterpoladoEmDiretivaNgFactory;
}

ComponentFactory<import1.J59InterpoladoEmDiretiva> createJ59InterpoladoEmDiretivaFactory() {
  return ComponentFactory('j59-interpolado-em-diretiva', viewFactory_J59InterpoladoEmDiretivaHost0);
}

final List<Object> styles$J59InterpoladoEmDiretivaHost = const [];

class _ViewJ59InterpoladoEmDiretivaHost0 extends import12.HostView<import1.J59InterpoladoEmDiretiva> {
  @override
  void build() {
    this.componentView = ViewJ59InterpoladoEmDiretiva0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J59InterpoladoEmDiretiva();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J59InterpoladoEmDiretiva> viewFactory_J59InterpoladoEmDiretivaHost0() {
  return _ViewJ59InterpoladoEmDiretivaHost0();
}
