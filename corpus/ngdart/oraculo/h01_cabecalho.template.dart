// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'h01_cabecalho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'h01_cabecalho.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$H01Cabecalho = const [];

class ViewH01Cabecalho0 extends import0.ComponentView<import1.H01Cabecalho> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewH01Cabecalho0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('h01-cabecalho'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/h01_cabecalho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'header');
    this.project(_el_0, 0);
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.titulo)) /* REF:package:corpus_ngdart/src/h01_cabecalho.html:50:60 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$H01Cabecalho, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _H01CabecalhoNgFactory = ComponentFactory<import1.H01Cabecalho>('h01-cabecalho,h01-cab', viewFactory_H01CabecalhoHost0);
ComponentFactory<import1.H01Cabecalho> get H01CabecalhoNgFactory {
  return _H01CabecalhoNgFactory;
}

ComponentFactory<import1.H01Cabecalho> createH01CabecalhoFactory() {
  return ComponentFactory('h01-cabecalho,h01-cab', viewFactory_H01CabecalhoHost0);
}

final List<Object> styles$H01CabecalhoHost = const [];

class _ViewH01CabecalhoHost0 extends import11.HostView<import1.H01Cabecalho> {
  @override
  void build() {
    this.componentView = ViewH01Cabecalho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.H01Cabecalho();
    this.component.itens = [];
    this.component.acoes = [];
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.H01Cabecalho> viewFactory_H01CabecalhoHost0() {
  return _ViewH01CabecalhoHost0();
}
