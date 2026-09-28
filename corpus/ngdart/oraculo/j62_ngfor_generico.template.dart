// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j62_ngfor_generico.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j62_ngfor_generico.dart' as import1;
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
import 'dart:core';
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$J62NgforGenerico = const [];

class ViewJ62NgforGenerico0 extends import0.ComponentView<import1.J62NgforGenerico> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  late final ViewContainer _appEl_2;
  late final import3.NgFor _NgFor_2_9;
  Object? _expr_0;
  Object? _expr_1;
  static import4.ComponentStyles? _componentStyles;
  ViewJ62NgforGenerico0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j62-ngfor-generico'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j62_ngfor_generico.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.TableElement>(doc, parentRenderNode, 'table');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J62NgforGenerico1);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    final _anchor_2 = import9.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J62NgforGenerico3);
    this._NgFor_2_9 = import3.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.semanas;
    if (import12.checkBinding(this._expr_0, currVal_0, 'semanas', 'package:corpus_ngdart/src/j62_ngfor_generico.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j62_ngfor_generico.html:14:44 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    final currVal_1 = _ctx.pares;
    if (import12.checkBinding(this._expr_1, currVal_1, 'pares', 'package:corpus_ngdart/src/j62_ngfor_generico.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_1);
      }
      this._NgFor_2_9.ngForOf = currVal_1 /* REF:package:corpus_ngdart/src/j62_ngfor_generico.html:127:152 */;
      this._expr_1 = currVal_1;
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

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J62NgforGenerico, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J62NgforGenericoNgFactory = ComponentFactory<import1.J62NgforGenerico>('j62-ngfor-generico', viewFactory_J62NgforGenericoHost0);
ComponentFactory<import1.J62NgforGenerico> get J62NgforGenericoNgFactory {
  return _J62NgforGenericoNgFactory;
}

ComponentFactory<import1.J62NgforGenerico> createJ62NgforGenericoFactory() {
  return ComponentFactory('j62-ngfor-generico', viewFactory_J62NgforGenericoHost0);
}

class _ViewJ62NgforGenerico1 extends import14.EmbeddedView<import1.J62NgforGenerico> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewJ62NgforGenerico1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('tr'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J62NgforGenerico2);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_semana = import7.unsafeCast<List<import1.J62Celula>>(this.locals['\$implicit']);
    final currVal_0 = local_semana;
    if (import12.checkBinding(this._expr_0, currVal_0, 'semana', 'package:corpus_ngdart/src/j62_ngfor_generico.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j62_ngfor_generico.html:54:83 */;
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

import14.EmbeddedView<void> viewFactory_J62NgforGenerico1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ62NgforGenerico1(parentView, parentIndex);
}

class _ViewJ62NgforGenerico2 extends import14.EmbeddedView<import1.J62NgforGenerico> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewJ62NgforGenerico2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('td'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_celula = import7.unsafeCast<import1.J62Celula>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolateString0(local_celula.rotulo)) /* REF:package:corpus_ngdart/src/j62_ngfor_generico.html:84:101 */;
  }
}

import14.EmbeddedView<void> viewFactory_J62NgforGenerico2(import15.RenderView parentView, int parentIndex) {
  return _ViewJ62NgforGenerico2(parentView, parentIndex);
}

class _ViewJ62NgforGenerico3 extends import14.EmbeddedView<import1.J62NgforGenerico> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewJ62NgforGenerico3(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_par = import7.unsafeCast<Map<String, int>>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolate0(local_par['a'])) /* REF:package:corpus_ngdart/src/j62_ngfor_generico.html:153:165 */;
  }
}

import14.EmbeddedView<void> viewFactory_J62NgforGenerico3(import15.RenderView parentView, int parentIndex) {
  return _ViewJ62NgforGenerico3(parentView, parentIndex);
}

final List<Object> styles$J62NgforGenericoHost = const [];

class _ViewJ62NgforGenericoHost0 extends import19.HostView<import1.J62NgforGenerico> {
  @override
  void build() {
    this.componentView = ViewJ62NgforGenerico0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J62NgforGenerico();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.J62NgforGenerico> viewFactory_J62NgforGenericoHost0() {
  return _ViewJ62NgforGenericoHost0();
}
