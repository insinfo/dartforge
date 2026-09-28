// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j70_evento_e_host_listener.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j70_evento_e_host_listener.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J70EventoEHostListener = const [];

class ViewJ70EventoEHostListener0 extends import0.ComponentView<import1.J70EventoEHostListener> {
  late final import1.J70Dica _J70Dica_0_5;
  late final import1.J70Dica _J70Dica_3_5;
  late final import1.J70Rastro _J70Rastro_3_6;
  late final import1.J70Rastro _J70Rastro_6_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ70EventoEHostListener0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j70-evento-e-host-listener'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j70_evento_e_host_listener.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    import7.setAttribute(_el_0, 'j70-dica', '');
    this._J70Dica_0_5 = import1.J70Dica();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_0, this._J70Dica_0_5);
    }
    final _text_1 = import7.appendText(_el_0, 'a');
    final _text_2 = import7.appendText(parentRenderNode, '\n');
    final _el_3 = import7.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    import7.setAttribute(_el_3, 'j70-dica', '');
    import7.setAttribute(_el_3, 'j70-rastro', '');
    this._J70Dica_3_5 = import1.J70Dica();
    this._J70Rastro_3_6 = import1.J70Rastro();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_3, this._J70Dica_3_5);
      import8.Inspector.instance.registerDirective(_el_3, this._J70Rastro_3_6);
    }
    final _text_4 = import7.appendText(_el_3, 'b');
    final _text_5 = import7.appendText(parentRenderNode, '\n');
    final _el_6 = import7.appendSpan(doc, parentRenderNode);
    import7.setAttribute(_el_6, 'j70-rastro', '');
    this._J70Rastro_6_5 = import1.J70Rastro();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_6, this._J70Rastro_6_5);
    }
    _el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    _el_0.addEventListener('mouseenter', this.eventHandler0(this._J70Dica_0_5.entrou));
    _el_0.addEventListener('focus', this.eventHandler1(this._handleEvent_1));
    _el_3.addEventListener('click', this.eventHandler1(this._handleEvent_2));
    _el_3.addEventListener('mouseenter', this.eventHandler1(this._handleEvent_3));
    _el_3.addEventListener('focus', this.eventHandler1(this._handleEvent_4));
    _el_6.addEventListener('focus', this.eventHandler0(_ctx.alternar));
    _el_6.addEventListener('click', this.eventHandler0(this._J70Rastro_6_5.marcar));
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.alternar();
    this._J70Dica_0_5.clicou($event);
  }

  void _handleEvent_1($event) {
    this._J70Dica_0_5.focou(1);
  }

  void _handleEvent_2($event) {
    final _ctx = this.ctx;
    _ctx.n = (_ctx.n + 1);
    this._J70Dica_3_5.clicou($event);
    this._J70Rastro_3_6.marcar();
  }

  void _handleEvent_3($event) {
    final _ctx = this.ctx;
    _ctx.alternar();
    this._J70Dica_3_5.entrou();
  }

  void _handleEvent_4($event) {
    this._J70Dica_3_5.focou(1);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J70EventoEHostListener, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J70EventoEHostListenerNgFactory = ComponentFactory<import1.J70EventoEHostListener>('j70-evento-e-host-listener', viewFactory_J70EventoEHostListenerHost0);
ComponentFactory<import1.J70EventoEHostListener> get J70EventoEHostListenerNgFactory {
  return _J70EventoEHostListenerNgFactory;
}

ComponentFactory<import1.J70EventoEHostListener> createJ70EventoEHostListenerFactory() {
  return ComponentFactory('j70-evento-e-host-listener', viewFactory_J70EventoEHostListenerHost0);
}

final List<Object> styles$J70EventoEHostListenerHost = const [];

class _ViewJ70EventoEHostListenerHost0 extends import10.HostView<import1.J70EventoEHostListener> {
  @override
  void build() {
    this.componentView = ViewJ70EventoEHostListener0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J70EventoEHostListener();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J70EventoEHostListener> viewFactory_J70EventoEHostListenerHost0() {
  return _ViewJ70EventoEHostListenerHost0();
}
