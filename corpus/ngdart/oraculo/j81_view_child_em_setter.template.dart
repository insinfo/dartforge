// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j81_view_child_em_setter.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j81_view_child_em_setter.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/src/runtime/queries.dart' as import15;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;

final List<Object> styles$J81Editor = const [];

class ViewJ81Editor0 extends import0.ComponentView<import1.J81Editor> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ81Editor0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j81-editor'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j81_view_child_em_setter.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J81Editor, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J81EditorNgFactory = ComponentFactory<import1.J81Editor>('j81-editor', viewFactory_J81EditorHost0);
ComponentFactory<import1.J81Editor> get J81EditorNgFactory {
  return _J81EditorNgFactory;
}

ComponentFactory<import1.J81Editor> createJ81EditorFactory() {
  return ComponentFactory('j81-editor', viewFactory_J81EditorHost0);
}

final List<Object> styles$J81EditorHost = const [];

class _ViewJ81EditorHost0 extends import9.HostView<import1.J81Editor> {
  @override
  void build() {
    this.componentView = ViewJ81Editor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J81Editor();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J81Editor> viewFactory_J81EditorHost0() {
  return _ViewJ81EditorHost0();
}

final List<Object> styles$J81ViewChildEmSetter = const [];

class ViewJ81ViewChildEmSetter0 extends import0.ComponentView<import1.J81ViewChildEmSetter> {
  bool _viewQuery_dentro_1_isDirty = true;
  late final ViewJ81Editor0 _compView_1;
  late final import1.J81Editor _J81Editor_1_5;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ81ViewChildEmSetter0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j81-view-child-em-setter'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j81_view_child_em_setter.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this._compView_1 = ViewJ81Editor0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._J81Editor_1_5 = import1.J81Editor();
    this._compView_1.create(this._J81Editor_1_5);
    final _anchor_2 = import7.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J81ViewChildEmSetter1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    _ctx.editorRef = this._J81Editor_1_5;
    _ctx.caixa = _el_0;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_2_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j81_view_child_em_setter.html:59:74 */;
    this._appEl_2.detectChangesInNestedViews();
    if ((!import14.debugThrowIfChanged)) {
      if (this._viewQuery_dentro_1_isDirty) {
        _ctx.dentroRef = import15.firstOrNull(this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ81ViewChildEmSetter1 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_dentro_1_isDirty = false;
      }
    }
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J81ViewChildEmSetter, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J81ViewChildEmSetterNgFactory = ComponentFactory<import1.J81ViewChildEmSetter>('j81-view-child-em-setter', viewFactory_J81ViewChildEmSetterHost0);
ComponentFactory<import1.J81ViewChildEmSetter> get J81ViewChildEmSetterNgFactory {
  return _J81ViewChildEmSetterNgFactory;
}

ComponentFactory<import1.J81ViewChildEmSetter> createJ81ViewChildEmSetterFactory() {
  return ComponentFactory('j81-view-child-em-setter', viewFactory_J81ViewChildEmSetterHost0);
}

class _ViewJ81ViewChildEmSetter1 extends import16.EmbeddedView<import1.J81ViewChildEmSetter> {
  late final import6.HtmlElement _el_1;
  _ViewJ81ViewChildEmSetter1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('p'));
    this._el_1 = import7.appendSpan(doc, _el_0);
    this.initRootNode(_el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ81ViewChildEmSetter0>((this.parentView!))._viewQuery_dentro_1_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J81ViewChildEmSetter1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ81ViewChildEmSetter1(parentView, parentIndex);
}

final List<Object> styles$J81ViewChildEmSetterHost = const [];

class _ViewJ81ViewChildEmSetterHost0 extends import9.HostView<import1.J81ViewChildEmSetter> {
  @override
  void build() {
    this.componentView = ViewJ81ViewChildEmSetter0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J81ViewChildEmSetter();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J81ViewChildEmSetter> viewFactory_J81ViewChildEmSetterHost0() {
  return _ViewJ81ViewChildEmSetterHost0();
}
