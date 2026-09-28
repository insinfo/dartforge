// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j98_estatico_herdado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j98_estatico_herdado.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$J98EstaticoHerdado = const [];

class ViewJ98EstaticoHerdado0 extends import0.ComponentView<import1.J98EstaticoHerdado> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import3.ComponentStyles? _componentStyles;
  ViewJ98EstaticoHerdado0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j98-estatico-herdado'));
    this.updateChildClassNonHtml(this.rootElement, import1.J98EstaticoHerdado.hostClass);
    import8.updateAttribute(this.rootElement, 'aria-modal', import1.J98EstaticoHerdado.modal);
    this.rootElement.tabIndex = import1.J98EstaticoHerdado.tab;
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j98_estatico_herdado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendSpan(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.rotulo)) /* REF:asset:corpus_ngdart/lib/src/j98_estatico_herdado.dart:585:597 */;
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.ativo;
    if (import10.checkBinding(this._expr_0, currVal_0, null, null)) {
      import8.updateClassBindingNonHtml(this.rootElement, 'ativo', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.rotulo;
    if (import10.checkBinding(this._expr_1, currVal_1, null, null)) {
      import8.updateAttribute(this.rootElement, 'aria-label', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.grande;
    if (import10.checkBinding(this._expr_2, currVal_2, null, null)) {
      import8.updateClassBindingNonHtml(this.rootElement, 'grande', currVal_2);
      this._expr_2 = currVal_2;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J98EstaticoHerdado, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J98EstaticoHerdadoNgFactory = ComponentFactory<import1.J98EstaticoHerdado>('j98-estatico-herdado', viewFactory_J98EstaticoHerdadoHost0);
ComponentFactory<import1.J98EstaticoHerdado> get J98EstaticoHerdadoNgFactory {
  return _J98EstaticoHerdadoNgFactory;
}

ComponentFactory<import1.J98EstaticoHerdado> createJ98EstaticoHerdadoFactory() {
  return ComponentFactory('j98-estatico-herdado', viewFactory_J98EstaticoHerdadoHost0);
}

final List<Object> styles$J98EstaticoHerdadoHost = const [];

class _ViewJ98EstaticoHerdadoHost0 extends import12.HostView<import1.J98EstaticoHerdado> {
  @override
  void build() {
    this.componentView = ViewJ98EstaticoHerdado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J98EstaticoHerdado();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import12.HostView<import1.J98EstaticoHerdado> viewFactory_J98EstaticoHerdadoHost0() {
  return _ViewJ98EstaticoHerdadoHost0();
}

final List<Object> styles$J98Usa = const [];

class ViewJ98Usa0 extends import0.ComponentView<import1.J98Usa> {
  late final ViewJ98EstaticoHerdado0 _compView_0;
  late final import1.J98EstaticoHerdado _J98EstaticoHerdado_0_5;
  static import3.ComponentStyles? _componentStyles;
  ViewJ98Usa0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j98-usa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j98_estatico_herdado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ98EstaticoHerdado0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J98EstaticoHerdado_0_5 = import1.J98EstaticoHerdado();
    this._compView_0.create(this._J98EstaticoHerdado_0_5);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this._compView_0.detectHostChanges(firstCheck);
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J98Usa, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J98UsaNgFactory = ComponentFactory<import1.J98Usa>('j98-usa', viewFactory_J98UsaHost0);
ComponentFactory<import1.J98Usa> get J98UsaNgFactory {
  return _J98UsaNgFactory;
}

ComponentFactory<import1.J98Usa> createJ98UsaFactory() {
  return ComponentFactory('j98-usa', viewFactory_J98UsaHost0);
}

final List<Object> styles$J98UsaHost = const [];

class _ViewJ98UsaHost0 extends import12.HostView<import1.J98Usa> {
  @override
  void build() {
    this.componentView = ViewJ98Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J98Usa();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J98Usa> viewFactory_J98UsaHost0() {
  return _ViewJ98UsaHost0();
}
