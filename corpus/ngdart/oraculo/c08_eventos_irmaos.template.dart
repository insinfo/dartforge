// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c08_eventos_irmaos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c08_eventos_irmaos.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$C08EventosIrmaos = const [];

class ViewC08EventosIrmaos0 extends import0.ComponentView<import1.C08EventosIrmaos> {
  static import2.ComponentStyles? _componentStyles;
  ViewC08EventosIrmaos0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('c08-eventos-irmaos'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/c08_eventos_irmaos.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    final _el_1 = import7.appendElement<import6.HtmlElement>(doc, _el_0, 'i');
    final _el_2 = import7.appendDiv(doc, parentRenderNode);
    final _el_3 = import7.appendSpan(doc, _el_2);
    final _text_4 = import7.appendText(_el_3, 'x');
    _el_0.addEventListener('click', this.eventHandler0(_ctx.a));
    _el_2.addEventListener('mouseover', this.eventHandler1(_ctx.b));
    _el_2.addEventListener('click', this.eventHandler0(_ctx.c));
    _el_3.addEventListener('click', this.eventHandler0(_ctx.d));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$C08EventosIrmaos, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C08EventosIrmaosNgFactory = ComponentFactory<import1.C08EventosIrmaos>('c08-eventos-irmaos', viewFactory_C08EventosIrmaosHost0);
ComponentFactory<import1.C08EventosIrmaos> get C08EventosIrmaosNgFactory {
  return _C08EventosIrmaosNgFactory;
}

ComponentFactory<import1.C08EventosIrmaos> createC08EventosIrmaosFactory() {
  return ComponentFactory('c08-eventos-irmaos', viewFactory_C08EventosIrmaosHost0);
}

final List<Object> styles$C08EventosIrmaosHost = const [];

class _ViewC08EventosIrmaosHost0 extends import9.HostView<import1.C08EventosIrmaos> {
  @override
  void build() {
    this.componentView = ViewC08EventosIrmaos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C08EventosIrmaos();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.C08EventosIrmaos> viewFactory_C08EventosIrmaosHost0() {
  return _ViewC08EventosIrmaosHost0();
}
