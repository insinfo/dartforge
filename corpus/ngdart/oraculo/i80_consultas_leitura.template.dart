// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i80_consultas_leitura.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i80_consultas_leitura.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;

final List<Object> styles$I80ConsultasLeitura = const [];

class ViewI80ConsultasLeitura0 extends import0.ComponentView<import1.I80ConsultasLeitura> {
  static import2.ComponentStyles? _componentStyles;
  ViewI80ConsultasLeitura0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i80-consultas-leitura'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i80_consultas_leitura.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I80ConsultasLeitura, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I80ConsultasLeituraNgFactory = ComponentFactory<import1.I80ConsultasLeitura>('i80-consultas-leitura', viewFactory_I80ConsultasLeituraHost0);
ComponentFactory<import1.I80ConsultasLeitura> get I80ConsultasLeituraNgFactory {
  return _I80ConsultasLeituraNgFactory;
}

ComponentFactory<import1.I80ConsultasLeitura> createI80ConsultasLeituraFactory() {
  return ComponentFactory('i80-consultas-leitura', viewFactory_I80ConsultasLeituraHost0);
}

final List<Object> styles$I80ConsultasLeituraHost = const [];

class _ViewI80ConsultasLeituraHost0 extends import8.HostView<import1.I80ConsultasLeitura> {
  @override
  void build() {
    this.componentView = ViewI80ConsultasLeitura0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I80ConsultasLeitura();
    this.component.rotulos = [];
    this.component.todos = [];
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.I80ConsultasLeitura> viewFactory_I80ConsultasLeituraHost0() {
  return _ViewI80ConsultasLeituraHost0();
}
