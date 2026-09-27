// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j07_estilos_na_anotacao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j07_estilos_na_anotacao.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$J07EstilosNaAnotacao = ['._nghost-%ID%{display:block}.a._ngcontent-%ID%{color:red}'];

class ViewJ07EstilosNaAnotacao0 extends import0.ComponentView<import1.J07EstilosNaAnotacao> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ07EstilosNaAnotacao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j07-estilos-na-anotacao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j07_estilos_na_anotacao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'a');
    this.addShimC(_el_0);
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.scoped(styles$J07EstilosNaAnotacao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J07EstilosNaAnotacaoNgFactory = ComponentFactory<import1.J07EstilosNaAnotacao>('j07-estilos-na-anotacao', viewFactory_J07EstilosNaAnotacaoHost0);
ComponentFactory<import1.J07EstilosNaAnotacao> get J07EstilosNaAnotacaoNgFactory {
  return _J07EstilosNaAnotacaoNgFactory;
}

ComponentFactory<import1.J07EstilosNaAnotacao> createJ07EstilosNaAnotacaoFactory() {
  return ComponentFactory('j07-estilos-na-anotacao', viewFactory_J07EstilosNaAnotacaoHost0);
}

final List<Object> styles$J07EstilosNaAnotacaoHost = const [];

class _ViewJ07EstilosNaAnotacaoHost0 extends import9.HostView<import1.J07EstilosNaAnotacao> {
  @override
  void build() {
    this.componentView = ViewJ07EstilosNaAnotacao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J07EstilosNaAnotacao();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J07EstilosNaAnotacao> viewFactory_J07EstilosNaAnotacaoHost0() {
  return _ViewJ07EstilosNaAnotacaoHost0();
}
