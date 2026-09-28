// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j82_consulta_sem_resultado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j82_consulta_sem_resultado.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$J82ConsultaSemResultado = const [];

class ViewJ82ConsultaSemResultado0 extends import0.ComponentView<import1.J82ConsultaSemResultado> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ82ConsultaSemResultado0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j82-consulta-sem-resultado'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j82_consulta_sem_resultado.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    _ctx.ausentes = [];
    _ctx.outro = _el_0;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J82ConsultaSemResultado, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J82ConsultaSemResultadoNgFactory = ComponentFactory<import1.J82ConsultaSemResultado>('j82-consulta-sem-resultado', viewFactory_J82ConsultaSemResultadoHost0);
ComponentFactory<import1.J82ConsultaSemResultado> get J82ConsultaSemResultadoNgFactory {
  return _J82ConsultaSemResultadoNgFactory;
}

ComponentFactory<import1.J82ConsultaSemResultado> createJ82ConsultaSemResultadoFactory() {
  return ComponentFactory('j82-consulta-sem-resultado', viewFactory_J82ConsultaSemResultadoHost0);
}

final List<Object> styles$J82ConsultaSemResultadoHost = const [];

class _ViewJ82ConsultaSemResultadoHost0 extends import9.HostView<import1.J82ConsultaSemResultado> {
  @override
  void build() {
    this.componentView = ViewJ82ConsultaSemResultado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J82ConsultaSemResultado();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J82ConsultaSemResultado> viewFactory_J82ConsultaSemResultadoHost0() {
  return _ViewJ82ConsultaSemResultadoHost0();
}
