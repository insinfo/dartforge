// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i52_ng_for_dinamico.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i52_ng_for_dinamico.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/runtime/text_binding.dart' as import16;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$I52NgForDinamico = const [];

class ViewI52NgForDinamico0 extends import0.ComponentView<import1.I52NgForDinamico> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI52NgForDinamico0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i52-ng-for-dinamico'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i52_ng_for_dinamico.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I52NgForDinamico1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = (_ctx.grupos ?? _ctx.vazio);
    if (import12.checkBinding(this._expr_0, currVal_0, 'grupos ?? vazio', 'package:corpus_ngdart/src/i52_ng_for_dinamico.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i52_ng_for_dinamico.html:5:53 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I52NgForDinamico, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I52NgForDinamicoNgFactory = ComponentFactory<import1.I52NgForDinamico>('i52-ng-for-dinamico', viewFactory_I52NgForDinamicoHost0);
ComponentFactory<import1.I52NgForDinamico> get I52NgForDinamicoNgFactory {
  return _I52NgForDinamicoNgFactory;
}

ComponentFactory<import1.I52NgForDinamico> createI52NgForDinamicoFactory() {
  return ComponentFactory('i52-ng-for-dinamico', viewFactory_I52NgForDinamicoHost0);
}

class _ViewI52NgForDinamico1 extends import14.EmbeddedView<import1.I52NgForDinamico> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewI52NgForDinamico1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I52NgForDinamico2);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i52_ng_for_dinamico.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i52_ng_for_dinamico.html:60:83 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import14.EmbeddedView<void> viewFactory_I52NgForDinamico1(import15.RenderView parentView, int parentIndex) {
  return _ViewI52NgForDinamico1(parentView, parentIndex);
}

class _ViewI52NgForDinamico2 extends import14.EmbeddedView<import1.I52NgForDinamico> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  final import16.TextBinding _textBinding_3 = import16.TextBinding();
  _ViewI52NgForDinamico2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('span'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import9.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    _el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_g = import7.unsafeCast<_ViewI52NgForDinamico1>((this.parentView!)).locals['\$implicit'];
    final local_i = import7.unsafeCast<int>(import7.unsafeCast<_ViewI52NgForDinamico1>((this.parentView!)).locals['index']);
    this._textBinding_1.updateText(import18.interpolate0(local_g.nome)) /* REF:package:corpus_ngdart/src/i52_ng_for_dinamico.html:105:115 */;
    this._textBinding_3.updateTextWithPrimitive(local_i) /* REF:package:corpus_ngdart/src/i52_ng_for_dinamico.html:116:121 */;
  }

  void _handleEvent_0($event) {
    final local_g = import7.unsafeCast<_ViewI52NgForDinamico1>((this.parentView!)).locals['\$implicit'];
    final local_x = import7.unsafeCast<String>(this.locals['\$implicit']);
    final _ctx = this.ctx;
    _ctx.usar(local_g, local_x);
  }
}

import14.EmbeddedView<void> viewFactory_I52NgForDinamico2(import15.RenderView parentView, int parentIndex) {
  return _ViewI52NgForDinamico2(parentView, parentIndex);
}

final List<Object> styles$I52NgForDinamicoHost = const [];

class _ViewI52NgForDinamicoHost0 extends import19.HostView<import1.I52NgForDinamico> {
  @override
  void build() {
    this.componentView = ViewI52NgForDinamico0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I52NgForDinamico();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.I52NgForDinamico> viewFactory_I52NgForDinamicoHost0() {
  return _ViewI52NgForDinamicoHost0();
}
