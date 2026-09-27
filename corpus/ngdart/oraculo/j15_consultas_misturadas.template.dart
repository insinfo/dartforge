// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j15_consultas_misturadas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j15_consultas_misturadas.dart' as import1;
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
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/src/runtime/queries.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$J15ConsultasMisturadas = const [];

class ViewJ15ConsultasMisturadas0 extends import0.ComponentView<import1.J15ConsultasMisturadas> {
  bool _viewQuery_dentro_1_isDirty = true;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ15ConsultasMisturadas0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j15-consultas-misturadas'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j15_consultas_misturadas.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_1 = import9.appendText(_el_0, '1');
    final _anchor_2 = import9.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J15ConsultasMisturadas1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    final _el_3 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'u');
    final _text_4 = import9.appendText(_el_3, '3');
    _ctx.fora = _el_0;
    _ctx.depois = _el_3;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.a);
    }
    this._NgIf_2_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j15_consultas_misturadas.html:19:28 */;
    this._appEl_2.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_dentro_1_isDirty) {
        _ctx.dentro = import13.firstOrNull(this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ15ConsultasMisturadas1 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_dentro_1_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J15ConsultasMisturadas, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J15ConsultasMisturadasNgFactory = ComponentFactory<import1.J15ConsultasMisturadas>('j15-consultas-misturadas', viewFactory_J15ConsultasMisturadasHost0);
ComponentFactory<import1.J15ConsultasMisturadas> get J15ConsultasMisturadasNgFactory {
  return _J15ConsultasMisturadasNgFactory;
}

ComponentFactory<import1.J15ConsultasMisturadas> createJ15ConsultasMisturadasFactory() {
  return ComponentFactory('j15-consultas-misturadas', viewFactory_J15ConsultasMisturadasHost0);
}

class _ViewJ15ConsultasMisturadas1 extends import15.EmbeddedView<import1.J15ConsultasMisturadas> {
  late final import8.HtmlElement _el_1;
  _ViewJ15ConsultasMisturadas1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this._el_1 = import9.appendElement<import8.HtmlElement>(doc, _el_0, 'i');
    final _text_2 = import9.appendText(this._el_1, '2');
    this.initRootNode(_el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewJ15ConsultasMisturadas0>((this.parentView!))._viewQuery_dentro_1_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J15ConsultasMisturadas1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ15ConsultasMisturadas1(parentView, parentIndex);
}

final List<Object> styles$J15ConsultasMisturadasHost = const [];

class _ViewJ15ConsultasMisturadasHost0 extends import17.HostView<import1.J15ConsultasMisturadas> {
  @override
  void build() {
    this.componentView = ViewJ15ConsultasMisturadas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J15ConsultasMisturadas();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J15ConsultasMisturadas> viewFactory_J15ConsultasMisturadasHost0() {
  return _ViewJ15ConsultasMisturadasHost0();
}
