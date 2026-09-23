// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c13_evento_em_ng_for.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c13_evento_em_ng_for.dart' as import1;
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
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$C13EventoEmNgFor = const [];

class ViewC13EventoEmNgFor0 extends import0.ComponentView<import1.C13EventoEmNgFor> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewC13EventoEmNgFor0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('c13-evento-em-ng-for'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/c13_evento_em_ng_for.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_C13EventoEmNgFor1);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/c13_evento_em_ng_for.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/c13_evento_em_ng_for.html:8:49 */;
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

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$C13EventoEmNgFor, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C13EventoEmNgForNgFactory = ComponentFactory<import1.C13EventoEmNgFor>('c13-evento-em-ng-for', viewFactory_C13EventoEmNgForHost0);
ComponentFactory<import1.C13EventoEmNgFor> get C13EventoEmNgForNgFactory {
  return _C13EventoEmNgForNgFactory;
}

ComponentFactory<import1.C13EventoEmNgFor> createC13EventoEmNgForFactory() {
  return ComponentFactory('c13-evento-em-ng-for', viewFactory_C13EventoEmNgForHost0);
}

class _ViewC13EventoEmNgFor1 extends import14.EmbeddedView<import1.C13EventoEmNgFor> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  _ViewC13EventoEmNgFor1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _ctx = this.ctx;
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('li'));
    _el_0.append(this._textBinding_1.element);
    _el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    _el_0.addEventListener('dblclick', this.eventHandler0(_ctx.limpar));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolateString0(local_item)) /* REF:package:corpus_ngdart/src/c13_evento_em_ng_for.html:100:108 */;
  }

  void _handleEvent_0($event) {
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    final local_i = import7.unsafeCast<int>(this.locals['index']);
    final _ctx = this.ctx;
    _ctx.escolher(local_item, local_i);
  }
}

import14.EmbeddedView<void> viewFactory_C13EventoEmNgFor1(import16.RenderView parentView, int parentIndex) {
  return _ViewC13EventoEmNgFor1(parentView, parentIndex);
}

final List<Object> styles$C13EventoEmNgForHost = const [];

class _ViewC13EventoEmNgForHost0 extends import19.HostView<import1.C13EventoEmNgFor> {
  @override
  void build() {
    this.componentView = ViewC13EventoEmNgFor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C13EventoEmNgFor();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.C13EventoEmNgFor> viewFactory_C13EventoEmNgForHost0() {
  return _ViewC13EventoEmNgForHost0();
}
