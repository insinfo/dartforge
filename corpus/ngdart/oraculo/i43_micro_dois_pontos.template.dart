// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i43_micro_dois_pontos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i43_micro_dois_pontos.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import13;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import15;

final List<Object> styles$I43MicroDoisPontos = const [];

class ViewI43MicroDoisPontos0 extends import0.ComponentView<import1.I43MicroDoisPontos> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import4.ComponentStyles? _componentStyles;
  ViewI43MicroDoisPontos0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i43-micro-dois-pontos'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i43_micro_dois_pontos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I43MicroDoisPontos1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final _anchor_1 = import9.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I43MicroDoisPontos2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', (_ctx.ativo ? _ctx.mostrar : false));
    }
    this._NgIf_0_9.ngIf = (_ctx.ativo ? _ctx.mostrar : false) /* REF:package:corpus_ngdart/src/i43_micro_dois_pontos.html:3:34 */;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', (_ctx.modo == 'a:b'));
    }
    this._NgIf_1_9.ngIf = (_ctx.modo == 'a:b') /* REF:package:corpus_ngdart/src/i43_micro_dois_pontos.html:43:64 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I43MicroDoisPontos, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I43MicroDoisPontosNgFactory = ComponentFactory<import1.I43MicroDoisPontos>('i43-micro-dois-pontos', viewFactory_I43MicroDoisPontosHost0);
ComponentFactory<import1.I43MicroDoisPontos> get I43MicroDoisPontosNgFactory {
  return _I43MicroDoisPontosNgFactory;
}

ComponentFactory<import1.I43MicroDoisPontos> createI43MicroDoisPontosFactory() {
  return ComponentFactory('i43-micro-dois-pontos', viewFactory_I43MicroDoisPontosHost0);
}

class _ViewI43MicroDoisPontos1 extends import13.EmbeddedView<import1.I43MicroDoisPontos> {
  _ViewI43MicroDoisPontos1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_I43MicroDoisPontos1(import14.RenderView parentView, int parentIndex) {
  return _ViewI43MicroDoisPontos1(parentView, parentIndex);
}

class _ViewI43MicroDoisPontos2 extends import13.EmbeddedView<import1.I43MicroDoisPontos> {
  _ViewI43MicroDoisPontos2(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'y');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_I43MicroDoisPontos2(import14.RenderView parentView, int parentIndex) {
  return _ViewI43MicroDoisPontos2(parentView, parentIndex);
}

final List<Object> styles$I43MicroDoisPontosHost = const [];

class _ViewI43MicroDoisPontosHost0 extends import15.HostView<import1.I43MicroDoisPontos> {
  @override
  void build() {
    this.componentView = ViewI43MicroDoisPontos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I43MicroDoisPontos();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.I43MicroDoisPontos> viewFactory_I43MicroDoisPontosHost0() {
  return _ViewI43MicroDoisPontosHost0();
}
