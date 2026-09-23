// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd09_projecao_select.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd09_projecao_select.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$D09ProjecaoSelect = const [];

class ViewD09ProjecaoSelect0 extends import0.ComponentView<import1.D09ProjecaoSelect> {
  static import2.ComponentStyles? _componentStyles;
  ViewD09ProjecaoSelect0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('d09-projecao-select'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/d09_projecao_select.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'header');
    this.project(_el_0, 0);
    final _el_1 = import7.appendDiv(doc, parentRenderNode);
    this.project(_el_1, 1);
    final _el_2 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'footer');
    this.project(_el_2, 2);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$D09ProjecaoSelect, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D09ProjecaoSelectNgFactory = ComponentFactory<import1.D09ProjecaoSelect>('d09-projecao-select', viewFactory_D09ProjecaoSelectHost0);
ComponentFactory<import1.D09ProjecaoSelect> get D09ProjecaoSelectNgFactory {
  return _D09ProjecaoSelectNgFactory;
}

ComponentFactory<import1.D09ProjecaoSelect> createD09ProjecaoSelectFactory() {
  return ComponentFactory('d09-projecao-select', viewFactory_D09ProjecaoSelectHost0);
}

final List<Object> styles$D09ProjecaoSelectHost = const [];

class _ViewD09ProjecaoSelectHost0 extends import9.HostView<import1.D09ProjecaoSelect> {
  @override
  void build() {
    this.componentView = ViewD09ProjecaoSelect0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D09ProjecaoSelect();
    this.component.marcas = [];
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.D09ProjecaoSelect> viewFactory_D09ProjecaoSelectHost0() {
  return _ViewD09ProjecaoSelectHost0();
}
