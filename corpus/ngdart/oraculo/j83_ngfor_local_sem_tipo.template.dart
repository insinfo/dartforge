// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j83_ngfor_local_sem_tipo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j83_ngfor_local_sem_tipo.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/common/directives/ng_template_outlet.dart' as import4;
import 'package:ngdart/src/common/directives/ng_for.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$J83NgforLocalSemTipo = const [];

class ViewJ83NgforLocalSemTipo0 extends import0.ComponentView<import1.J83NgforLocalSemTipo> {
  late final ViewContainer _appEl_0;
  late final TemplateRef _TemplateRef_0_7;
  late final ViewContainer _appEl_1;
  late final import4.NgTemplateOutlet _NgTemplateOutlet_1_9;
  late final ViewContainer _appEl_2;
  late final import5.NgFor _NgFor_2_9;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import6.ComponentStyles? _componentStyles;
  ViewJ83NgforLocalSemTipo0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('j83-ngfor-local-sem-tipo'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j83_ngfor_local_sem_tipo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import11.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    this._TemplateRef_0_7 = TemplateRef(this._appEl_0, viewFactory_J83NgforLocalSemTipo1);
    final _anchor_1 = import11.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J83NgforLocalSemTipo3);
    this._NgTemplateOutlet_1_9 = import4.NgTemplateOutlet(this._appEl_1);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_1, this._NgTemplateOutlet_1_9);
    }
    final _anchor_2 = import11.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J83NgforLocalSemTipo4);
    this._NgFor_2_9 = import5.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_arvore = this._TemplateRef_0_7;
    final currVal_0 = local_arvore;
    if (import13.checkBinding(this._expr_0, currVal_0, 'arvore', 'package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgTemplateOutlet_1_9, 'ngTemplateOutlet', currVal_0);
      }
      this._NgTemplateOutlet_1_9.ngTemplateOutlet = currVal_0 /* REF:package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html:105:132 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.contexto;
    if (import13.checkBinding(this._expr_1, currVal_1, 'contexto', 'package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgTemplateOutlet_1_9, 'ngTemplateOutletContext', currVal_1);
      }
      this._NgTemplateOutlet_1_9.ngTemplateOutletContext = currVal_1 /* REF:package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html:133:169 */;
      this._expr_1 = currVal_1;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgTemplateOutlet_1_9.ngDoCheck();
    }
    final currVal_2 = _ctx.soltos;
    if (import13.checkBinding(this._expr_2, currVal_2, 'soltos', 'package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_2);
      }
      this._NgFor_2_9.ngForOf = currVal_2 /* REF:package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html:187:211 */;
      this._expr_2 = currVal_2;
    }
    if ((!import13.debugThrowIfChanged)) {
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
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J83NgforLocalSemTipo, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J83NgforLocalSemTipoNgFactory = ComponentFactory<import1.J83NgforLocalSemTipo>('j83-ngfor-local-sem-tipo', viewFactory_J83NgforLocalSemTipoHost0);
ComponentFactory<import1.J83NgforLocalSemTipo> get J83NgforLocalSemTipoNgFactory {
  return _J83NgforLocalSemTipoNgFactory;
}

ComponentFactory<import1.J83NgforLocalSemTipo> createJ83NgforLocalSemTipoFactory() {
  return ComponentFactory('j83-ngfor-local-sem-tipo', viewFactory_J83NgforLocalSemTipoHost0);
}

class _ViewJ83NgforLocalSemTipo1 extends import15.EmbeddedView<import1.J83NgforLocalSemTipo> {
  late final ViewContainer _appEl_0;
  late final import5.NgFor _NgFor_0_9;
  Object? _expr_0;
  _ViewJ83NgforLocalSemTipo1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _anchor_0 = import11.createAnchor();
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J83NgforLocalSemTipo2);
    this._NgFor_0_9 = import5.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
    this.initRootNode(this._appEl_0);
  }

  @override
  void detectChangesInternal() {
    final local_lista = this.locals['\$implicit'];
    final currVal_0 = local_lista;
    if (import13.checkBinding(this._expr_0, currVal_0, 'lista', 'package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html:34:60 */;
      this._expr_0 = currVal_0;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }
}

import15.EmbeddedView<void> viewFactory_J83NgforLocalSemTipo1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ83NgforLocalSemTipo1(parentView, parentIndex);
}

class _ViewJ83NgforLocalSemTipo2 extends import15.EmbeddedView<import1.J83NgforLocalSemTipo> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewJ83NgforLocalSemTipo2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_item = import9.unsafeCast<import1.J83No>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolateString0(local_item.rotulo)) /* REF:package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html:61:78 */;
  }
}

import15.EmbeddedView<void> viewFactory_J83NgforLocalSemTipo2(import16.RenderView parentView, int parentIndex) {
  return _ViewJ83NgforLocalSemTipo2(parentView, parentIndex);
}

class _ViewJ83NgforLocalSemTipo3 extends import15.EmbeddedView<import1.J83NgforLocalSemTipo> {
  _ViewJ83NgforLocalSemTipo3(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import9.unsafeCast(const <Object>[]), null);
  }
}

import15.EmbeddedView<void> viewFactory_J83NgforLocalSemTipo3(import16.RenderView parentView, int parentIndex) {
  return _ViewJ83NgforLocalSemTipo3(parentView, parentIndex);
}

class _ViewJ83NgforLocalSemTipo4 extends import15.EmbeddedView<import1.J83NgforLocalSemTipo> {
  late final ViewContainer _appEl_1;
  late final import5.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewJ83NgforLocalSemTipo4(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import11.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J83NgforLocalSemTipo5);
    this._NgFor_1_9 = import5.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_s = this.locals['\$implicit'];
    final currVal_0 = local_s.itens;
    if (import13.checkBinding(this._expr_0, currVal_0, 's.itens', 'package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html:221:246 */;
      this._expr_0 = currVal_0;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import15.EmbeddedView<void> viewFactory_J83NgforLocalSemTipo4(import16.RenderView parentView, int parentIndex) {
  return _ViewJ83NgforLocalSemTipo4(parentView, parentIndex);
}

class _ViewJ83NgforLocalSemTipo5 extends import15.EmbeddedView<import1.J83NgforLocalSemTipo> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewJ83NgforLocalSemTipo5(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('span'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_t = this.locals['\$implicit'];
    this._textBinding_1.updateText(import18.interpolate0(local_t)) /* REF:package:corpus_ngdart/src/j83_ngfor_local_sem_tipo.html:247:254 */;
  }
}

import15.EmbeddedView<void> viewFactory_J83NgforLocalSemTipo5(import16.RenderView parentView, int parentIndex) {
  return _ViewJ83NgforLocalSemTipo5(parentView, parentIndex);
}

final List<Object> styles$J83NgforLocalSemTipoHost = const [];

class _ViewJ83NgforLocalSemTipoHost0 extends import19.HostView<import1.J83NgforLocalSemTipo> {
  @override
  void build() {
    this.componentView = ViewJ83NgforLocalSemTipo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J83NgforLocalSemTipo();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.J83NgforLocalSemTipo> viewFactory_J83NgforLocalSemTipoHost0() {
  return _ViewJ83NgforLocalSemTipoHost0();
}
