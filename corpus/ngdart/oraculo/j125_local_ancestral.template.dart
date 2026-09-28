// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j125_local_ancestral.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j125_local_ancestral.dart' as import1;
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
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$J125Usa = const [];

class ViewJ125Usa0 extends import0.ComponentView<import1.J125Usa> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewJ125Usa0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j125-usa'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j125_local_ancestral.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J125Usa1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.grupos;
    if (import12.checkBinding(this._expr_0, currVal_0, 'grupos', 'asset:corpus_ngdart/lib/src/j125_local_ancestral.dart')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1026:1069 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J125Usa, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J125UsaNgFactory = ComponentFactory<import1.J125Usa>('j125-usa', viewFactory_J125UsaHost0);
ComponentFactory<import1.J125Usa> get J125UsaNgFactory {
  return _J125UsaNgFactory;
}

ComponentFactory<import1.J125Usa> createJ125UsaFactory() {
  return ComponentFactory('j125-usa', viewFactory_J125UsaHost0);
}

class _ViewJ125Usa1 extends import14.EmbeddedView<import1.J125Usa> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  late final ViewContainer _appEl_2;
  late final import3.NgFor _NgFor_2_9;
  Object? _expr_0;
  _ViewJ125Usa1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J125Usa2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    final _anchor_2 = import9.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J125Usa4);
    this._NgFor_2_9 = import3.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_grupo = import7.unsafeCast<import1.J125Grupo<dynamic>>(this.locals['\$implicit']);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', local_grupo.visivel);
    }
    this._NgIf_1_9.ngIf = local_grupo.visivel /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1078:1099 */;
    final currVal_0 = _ctx.lista;
    if (import12.checkBinding(this._expr_0, currVal_0, 'lista', 'asset:corpus_ngdart/lib/src/j125_local_ancestral.dart')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_0);
      }
      this._NgFor_2_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1235:1252 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_2_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
  }
}

import14.EmbeddedView<void> viewFactory_J125Usa1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ125Usa1(parentView, parentIndex);
}

class _ViewJ125Usa2 extends import14.EmbeddedView<import1.J125Usa> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewJ125Usa2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J125Usa3);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_grupo = import7.unsafeCast<import1.J125Grupo<dynamic>>(import7.unsafeCast<_ViewJ125Usa1>((this.parentView!)).locals['\$implicit']);
    final currVal_0 = local_grupo.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'grupo.itens', 'asset:corpus_ngdart/lib/src/j125_local_ancestral.dart')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1111:1158 */;
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

import14.EmbeddedView<void> viewFactory_J125Usa2(import16.RenderView parentView, int parentIndex) {
  return _ViewJ125Usa2(parentView, parentIndex);
}

class _ViewJ125Usa3 extends import14.EmbeddedView<import1.J125Usa> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  final import17.TextBinding _textBinding_3 = import17.TextBinding();
  final import17.TextBinding _textBinding_5 = import17.TextBinding();
  final import17.TextBinding _textBinding_7 = import17.TextBinding();
  _ViewJ125Usa3(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('span'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import9.appendText(_el_0, '-');
    _el_0.append(this._textBinding_3.element);
    final _text_4 = import9.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_5.element);
    final _text_6 = import9.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_7.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_i = import7.unsafeCast<int>(import7.unsafeCast<_ViewJ125Usa1>(((this.parentView!).parentView!)).locals['index']);
    final local_j = import7.unsafeCast<int>(this.locals['index']);
    final local_item = this.locals['\$implicit'];
    final local_grupo = import7.unsafeCast<import1.J125Grupo<dynamic>>(import7.unsafeCast<_ViewJ125Usa1>(((this.parentView!).parentView!)).locals['\$implicit']);
    this._textBinding_1.updateTextWithPrimitive(local_i) /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1159:1164 */;
    this._textBinding_3.updateTextWithPrimitive(local_j) /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1165:1170 */;
    this._textBinding_5.updateText(import19.interpolate0(local_item)) /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1171:1179 */;
    this._textBinding_7.updateText(import19.interpolateString0(local_grupo.nome)) /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1180:1194 */;
  }
}

import14.EmbeddedView<void> viewFactory_J125Usa3(import16.RenderView parentView, int parentIndex) {
  return _ViewJ125Usa3(parentView, parentIndex);
}

class _ViewJ125Usa4 extends import14.EmbeddedView<import1.J125Usa> {
  final import17.TextBinding _textBinding_0 = import17.TextBinding();
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  final import17.TextBinding _textBinding_2 = import17.TextBinding();
  _ViewJ125Usa4(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(<Object>[this._textBinding_0.element, this._textBinding_1.element, this._textBinding_2.element]), null);
  }

  @override
  void detectChangesInternal() {
    final local_x = import7.unsafeCast<String>(this.locals['\$implicit']);
    final local_k = import7.unsafeCast<int>(this.locals['index']);
    final local_grupo = import7.unsafeCast<import1.J125Grupo<dynamic>>(import7.unsafeCast<_ViewJ125Usa1>((this.parentView!)).locals['\$implicit']);
    this._textBinding_0.updateText(import19.interpolateString0(local_x)) /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1267:1272 */;
    this._textBinding_1.updateTextWithPrimitive(local_k) /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1272:1277 */;
    this._textBinding_2.updateText(import19.interpolateString0(local_grupo.nome)) /* REF:asset:corpus_ngdart/lib/src/j125_local_ancestral.dart:1277:1291 */;
  }
}

import14.EmbeddedView<void> viewFactory_J125Usa4(import16.RenderView parentView, int parentIndex) {
  return _ViewJ125Usa4(parentView, parentIndex);
}

final List<Object> styles$J125UsaHost = const [];

class _ViewJ125UsaHost0 extends import20.HostView<import1.J125Usa> {
  @override
  void build() {
    this.componentView = ViewJ125Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J125Usa();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.J125Usa> viewFactory_J125UsaHost0() {
  return _ViewJ125UsaHost0();
}
