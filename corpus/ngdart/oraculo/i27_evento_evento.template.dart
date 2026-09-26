// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i27_evento_evento.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i27_evento_evento.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/core/linker/app_view_utils.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I27EventoEvento = const [];

class ViewI27EventoEvento0 extends import0.ComponentView<import1.I27EventoEvento> {
  static import2.ComponentStyles? _componentStyles;
  ViewI27EventoEvento0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i27-evento-evento'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i27_evento_evento.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.InputElement>(doc, parentRenderNode, 'input');
    _el_0.addEventListener('input', this.eventHandler1(this._handleEvent_0));
    _el_0.addEventListener('blur', this.eventHandler0(_ctx.sair));
    import8.appViewUtils.eventManager.addEventListener(_el_0, 'keydown.enter', this.eventHandler1(_ctx.enviar));
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.valor = $event.target.value;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I27EventoEvento, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I27EventoEventoNgFactory = ComponentFactory<import1.I27EventoEvento>('i27-evento-evento', viewFactory_I27EventoEventoHost0);
ComponentFactory<import1.I27EventoEvento> get I27EventoEventoNgFactory {
  return _I27EventoEventoNgFactory;
}

ComponentFactory<import1.I27EventoEvento> createI27EventoEventoFactory() {
  return ComponentFactory('i27-evento-evento', viewFactory_I27EventoEventoHost0);
}

final List<Object> styles$I27EventoEventoHost = const [];

class _ViewI27EventoEventoHost0 extends import10.HostView<import1.I27EventoEvento> {
  @override
  void build() {
    this.componentView = ViewI27EventoEvento0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I27EventoEvento();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I27EventoEvento> viewFactory_I27EventoEventoHost0() {
  return _ViewI27EventoEventoHost0();
}
