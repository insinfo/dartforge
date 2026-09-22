// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a08_evento.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a08_evento.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$A08Evento = const [];

class ViewA08Evento0 extends import0.ComponentView<import1.A08Evento> {
  static import2.ComponentStyles? _componentStyles;
  ViewA08Evento0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('a08-evento'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/a08_evento.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_1 = import7.appendText(_el_0, 'ok');
    _el_0.addEventListener('click', this.eventHandler0(_ctx.clicou));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$A08Evento, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A08EventoNgFactory = ComponentFactory<import1.A08Evento>('a08-evento', viewFactory_A08EventoHost0);
ComponentFactory<import1.A08Evento> get A08EventoNgFactory {
  return _A08EventoNgFactory;
}

ComponentFactory<import1.A08Evento> createA08EventoFactory() {
  return ComponentFactory('a08-evento', viewFactory_A08EventoHost0);
}

final List<Object> styles$A08EventoHost = const [];

class _ViewA08EventoHost0 extends import9.HostView<import1.A08Evento> {
  @override
  void build() {
    this.componentView = ViewA08Evento0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A08Evento();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.A08Evento> viewFactory_A08EventoHost0() {
  return _ViewA08EventoHost0();
}
