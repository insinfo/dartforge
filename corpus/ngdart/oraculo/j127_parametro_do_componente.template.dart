// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j127_parametro_do_componente.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j127_parametro_do_componente.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/runtime/text_binding.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'package:ngdart/src/runtime/interpolate.dart' as import18;

final List<Object> styles$J127Folha = const [];

class ViewJ127Folha0 extends import0.ComponentView<import1.J127Folha> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ127Folha0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j127-folha'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J127Folha, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J127FolhaNgFactory = ComponentFactory<import1.J127Folha>('j127-folha', viewFactory_J127FolhaHost0);
ComponentFactory<import1.J127Folha> get J127FolhaNgFactory {
  return _J127FolhaNgFactory;
}

ComponentFactory<import1.J127Folha> createJ127FolhaFactory() {
  return ComponentFactory('j127-folha', viewFactory_J127FolhaHost0);
}

final List<Object> styles$J127FolhaHost = const [];

class _ViewJ127FolhaHost0 extends import8.HostView<import1.J127Folha> {
  @override
  void build() {
    this.componentView = ViewJ127Folha0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J127Folha();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J127Folha> viewFactory_J127FolhaHost0() {
  return _ViewJ127FolhaHost0();
}

final List<Object> styles$J127Arvore = const [];

class ViewJ127Arvore0<T> extends import0.ComponentView<import1.J127Arvore<T>> {
  late final ViewContainer _appEl_0;
  late final import10.NgFor _NgFor_0_9;
  late final ViewContainer _appEl_1;
  late final import10.NgFor _NgFor_1_9;
  late final ViewContainer _appEl_2;
  late final import10.NgFor _NgFor_2_9;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import2.ComponentStyles? _componentStyles;
  ViewJ127Arvore0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j127-arvore'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import11.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, (parentView, parentIndex) {
      return viewFactory_J127Arvore1<T>(parentView, parentIndex);
    });
    this._NgFor_0_9 = import10.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
    final _anchor_1 = import11.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, (parentView, parentIndex) {
      return viewFactory_J127Arvore3<T>(parentView, parentIndex);
    });
    this._NgFor_1_9 = import10.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    final _anchor_2 = import11.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, (parentView, parentIndex) {
      return viewFactory_J127Arvore4<T>(parentView, parentIndex);
    });
    this._NgFor_2_9 = import10.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.opcoes.grupos;
    if (import14.checkBinding(this._expr_0, currVal_0, 'opcoes.grupos', 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:768:799 */;
      this._expr_0 = currVal_0;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    final currVal_1 = _ctx.filhos(1);
    if (import14.checkBinding(this._expr_1, currVal_1, 'filhos(1)', 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_1);
      }
      this._NgFor_1_9.ngForOf = currVal_1 /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:889:916 */;
      this._expr_1 = currVal_1;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    final currVal_2 = _ctx.brutos;
    if (import14.checkBinding(this._expr_2, currVal_2, 'brutos', 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_2);
      }
      this._NgFor_2_9.ngForOf = currVal_2 /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:937:961 */;
      this._expr_2 = currVal_2;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_2_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J127Arvore, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J127ArvoreNgFactory = ComponentFactory<import1.J127Arvore>('j127-arvore', viewFactory_J127ArvoreHost0);
ComponentFactory<import1.J127Arvore> get J127ArvoreNgFactory {
  return _J127ArvoreNgFactory;
}

ComponentFactory<import1.J127Arvore<T>> createJ127ArvoreFactory<T>() {
  return ComponentFactory('j127-arvore', viewFactory_J127ArvoreHost0);
}

class _ViewJ127Arvore1<T> extends import15.EmbeddedView<import1.J127Arvore<T>> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  late final ViewContainer _appEl_2;
  late final import10.NgFor _NgFor_2_9;
  Object? _expr_0;
  _ViewJ127Arvore1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    _el_0.append(this._textBinding_1.element);
    final _anchor_2 = import11.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, (parentView, parentIndex) {
      return viewFactory_J127Arvore2<T>(parentView, parentIndex);
    });
    this._NgFor_2_9 = import10.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_g = import5.unsafeCast<import1.J127Grupo<T>>(this.locals['\$implicit']);
    final currVal_0 = _ctx.filhos(local_g);
    if (import14.checkBinding(this._expr_0, currVal_0, 'filhos(g)', 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_0);
      }
      this._NgFor_2_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:825:852 */;
      this._expr_0 = currVal_0;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_2_9.ngDoCheck();
    }
    this._appEl_2.detectChangesInNestedViews();
    this._textBinding_1.updateText(import18.interpolateString0(local_g.nome)) /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:800:810 */;
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
  }
}

import15.EmbeddedView<void> viewFactory_J127Arvore1<T>(import17.RenderView parentView, int parentIndex) {
  return _ViewJ127Arvore1<T>(parentView, parentIndex);
}

class _ViewJ127Arvore2<T> extends import15.EmbeddedView<import1.J127Arvore<T>> {
  late final ViewJ127Folha0 _compView_0;
  late final import1.J127Folha _J127Folha_0_5;
  Object? _expr_0;
  _ViewJ127Arvore2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ127Folha0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J127Folha_0_5 = import1.J127Folha();
    this._compView_0.create(this._J127Folha_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_f = import5.unsafeCast<import1.J127Grupo<T?>>(this.locals['\$implicit']);
    final currVal_0 = local_f;
    if (import14.checkBinding(this._expr_0, currVal_0, 'f', 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J127Folha_0_5, 'valor', currVal_0);
      }
      this._J127Folha_0_5.valor = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:853:864 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import15.EmbeddedView<void> viewFactory_J127Arvore2<T>(import17.RenderView parentView, int parentIndex) {
  return _ViewJ127Arvore2<T>(parentView, parentIndex);
}

class _ViewJ127Arvore3<T> extends import15.EmbeddedView<import1.J127Arvore<T>> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  _ViewJ127Arvore3(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_s = import5.unsafeCast<import1.J127Grupo<T?>>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolateString0(local_s.nome)) /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:917:927 */;
  }
}

import15.EmbeddedView<void> viewFactory_J127Arvore3<T>(import17.RenderView parentView, int parentIndex) {
  return _ViewJ127Arvore3<T>(parentView, parentIndex);
}

class _ViewJ127Arvore4<T> extends import15.EmbeddedView<import1.J127Arvore<T>> {
  late final ViewContainer _appEl_1;
  late final import10.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewJ127Arvore4(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import11.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, (parentView, parentIndex) {
      return viewFactory_J127Arvore5<T>(parentView, parentIndex);
    });
    this._NgFor_1_9 = import10.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_b = this.locals['\$implicit'];
    final currVal_0 = _ctx.filhos(local_b);
    if (import14.checkBinding(this._expr_0, currVal_0, 'filhos(b)', 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:977:1004 */;
      this._expr_0 = currVal_0;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import15.EmbeddedView<void> viewFactory_J127Arvore4<T>(import17.RenderView parentView, int parentIndex) {
  return _ViewJ127Arvore4<T>(parentView, parentIndex);
}

class _ViewJ127Arvore5<T> extends import15.EmbeddedView<import1.J127Arvore<T>> {
  late final ViewJ127Folha0 _compView_0;
  late final import1.J127Folha _J127Folha_0_5;
  Object? _expr_0;
  _ViewJ127Arvore5(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ127Folha0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J127Folha_0_5 = import1.J127Folha();
    this._compView_0.create(this._J127Folha_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_y = import5.unsafeCast<import1.J127Grupo<T?>>(this.locals['\$implicit']);
    final currVal_0 = local_y;
    if (import14.checkBinding(this._expr_0, currVal_0, 'y', 'asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J127Folha_0_5, 'valor', currVal_0);
      }
      this._J127Folha_0_5.valor = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j127_parametro_do_componente.dart:1005:1016 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import15.EmbeddedView<void> viewFactory_J127Arvore5<T>(import17.RenderView parentView, int parentIndex) {
  return _ViewJ127Arvore5<T>(parentView, parentIndex);
}

final List<Object> styles$J127ArvoreHost = const [];

class _ViewJ127ArvoreHost0<T> extends import8.HostView<import1.J127Arvore<T>> {
  @override
  void build() {
    this.componentView = ViewJ127Arvore0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J127Arvore();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J127Arvore<T>> viewFactory_J127ArvoreHost0<T>() {
  return _ViewJ127ArvoreHost0();
}
