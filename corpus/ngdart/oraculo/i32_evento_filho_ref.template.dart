// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i32_evento_filho_ref.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i32_evento_filho_ref.dart' as import1;
import 'a02_texto_estatico.template.dart' as import2;
import 'a02_texto_estatico.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$I32EventoFilhoRef = const [];

class ViewI32EventoFilhoRef0 extends import0.ComponentView<import1.I32EventoFilhoRef> {
  late final import2.ViewA02TextoEstatico0 _compView_0;
  late final import3.A02TextoEstatico _A02TextoEstatico_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewI32EventoFilhoRef0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i32-evento-filho-ref'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i32_evento_filho_ref.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA02TextoEstatico0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A02TextoEstatico_0_5 = import3.A02TextoEstatico();
    this._compView_0.create(this._A02TextoEstatico_0_5);
    final doc = import8.document;
    final _el_1 = import9.appendElement<import8.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_2 = import9.appendText(_el_1, 'x');
    _el_1.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final local_t = this._A02TextoEstatico_0_5;
    final _ctx = this.ctx;
    _ctx.usar(local_t);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I32EventoFilhoRef, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I32EventoFilhoRefNgFactory = ComponentFactory<import1.I32EventoFilhoRef>('i32-evento-filho-ref', viewFactory_I32EventoFilhoRefHost0);
ComponentFactory<import1.I32EventoFilhoRef> get I32EventoFilhoRefNgFactory {
  return _I32EventoFilhoRefNgFactory;
}

ComponentFactory<import1.I32EventoFilhoRef> createI32EventoFilhoRefFactory() {
  return ComponentFactory('i32-evento-filho-ref', viewFactory_I32EventoFilhoRefHost0);
}

final List<Object> styles$I32EventoFilhoRefHost = const [];

class _ViewI32EventoFilhoRefHost0 extends import11.HostView<import1.I32EventoFilhoRef> {
  @override
  void build() {
    this.componentView = ViewI32EventoFilhoRef0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I32EventoFilhoRef();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.I32EventoFilhoRef> viewFactory_I32EventoFilhoRefHost0() {
  return _ViewI32EventoFilhoRefHost0();
}
