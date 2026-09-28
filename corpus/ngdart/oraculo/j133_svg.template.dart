// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j133_svg.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j133_svg.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'dart:html' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$J133Usa = ['.a._ngcontent-%ID%{color:red}'];

class ViewJ133Usa0 extends import0.ComponentView<import1.J133Usa> {
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  Object? _expr_0;
  late final import4.Element _el_2;
  static import5.ComponentStyles? _componentStyles;
  ViewJ133Usa0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import4.document.createElement('j133-usa'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j133_svg.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import4.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'a');
    this.addShimC(_el_0);
    final _el_1 = doc.createElementNS('http://www.w3.org/2000/svg', 'svg');
    _el_0.append(_el_1);
    this.updateChildClassNonHtml(_el_1, 'icone');
    import9.setAttribute(_el_1, 'viewBox', '0 0 24 24');
    import9.setAttribute(_el_1, 'width', '24');
    import9.setAttribute(_el_1, 'xmlns', 'http://www.w3.org/2000/svg');
    this.addShimE(_el_1);
    this._el_2 = doc.createElementNS('http://www.w3.org/2000/svg', 'path');
    _el_1.append(this._el_2);
    import9.setAttribute(this._el_2, 'd', 'M12 2z');
    this.addShimE(this._el_2);
    final _anchor_3 = import9.appendAnchor(_el_1);
    this._appEl_3 = ViewContainer(3, 1, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J133Usa1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.x);
    }
    this._NgIf_3_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j133_svg.dart:509:518 */;
    this._appEl_3.detectChangesInNestedViews();
    final currVal_0 = _ctx.cor;
    if (import12.checkBinding(this._expr_0, currVal_0, 'cor', 'asset:corpus_ngdart/lib/src/j133_svg.dart')) {
      import9.updateAttribute(this._el_2, 'fill', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j133_svg.dart:482:499 */;
      this._expr_0 = currVal_0;
    }
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.scoped(styles$J133Usa, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J133UsaNgFactory = ComponentFactory<import1.J133Usa>('j133-usa', viewFactory_J133UsaHost0);
ComponentFactory<import1.J133Usa> get J133UsaNgFactory {
  return _J133UsaNgFactory;
}

ComponentFactory<import1.J133Usa> createJ133UsaFactory() {
  return ComponentFactory('j133-usa', viewFactory_J133UsaHost0);
}

class _ViewJ133Usa1 extends import14.EmbeddedView<import1.J133Usa> {
  Object? _expr_0;
  late final import4.Element _el_1;
  _ViewJ133Usa1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import4.document;
    final _el_0 = doc.createElementNS('http://www.w3.org/2000/svg', 'g');
    this.addShimE(_el_0);
    this._el_1 = doc.createElementNS('http://www.w3.org/2000/svg', 'circle');
    _el_0.append(this._el_1);
    import9.setAttribute(this._el_1, 'r', '3');
    this.addShimE(this._el_1);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.x;
    if (import12.checkBinding(this._expr_0, currVal_0, 'x', 'asset:corpus_ngdart/lib/src/j133_svg.dart')) {
      import9.updateClassBindingNonHtml(this._el_1, 'ativo', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j133_svg.dart:533:550 */;
      this._expr_0 = currVal_0;
    }
  }
}

import14.EmbeddedView<void> viewFactory_J133Usa1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ133Usa1(parentView, parentIndex);
}

final List<Object> styles$J133UsaHost = const [];

class _ViewJ133UsaHost0 extends import16.HostView<import1.J133Usa> {
  @override
  void build() {
    this.componentView = ViewJ133Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J133Usa();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.J133Usa> viewFactory_J133UsaHost0() {
  return _ViewJ133UsaHost0();
}
