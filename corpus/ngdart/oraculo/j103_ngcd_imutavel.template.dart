// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j103_ngcd_imutavel.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j103_ngcd_imutavel.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import11;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;

final List<Object> styles$J103Usa = const [];

class ViewJ103Usa0 extends import0.ComponentView<import1.J103Usa> {
  late final J103ItemNgCd _J103Item_0_5;
  late final J103IdNgCd _J103Id_1_5;
  late final import2.HtmlElement _el_0;
  late final import2.HtmlElement _el_1;
  static import3.ComponentStyles? _componentStyles;
  ViewJ103Usa0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j103-usa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j103_ngcd_imutavel.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'p');
    import7.setAttribute(this._el_0, 'j103Item', '');
    import7.setAttribute(this._el_0, 'role', 'x');
    this._J103Item_0_5 = J103ItemNgCd(import1.J103Item('x'));
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(this._el_0, this._J103Item_0_5.instance);
    }
    this._el_1 = import7.appendSpan(doc, parentRenderNode);
    import7.setAttribute(this._el_1, 'j103Id', '');
    this._J103Id_1_5 = J103IdNgCd(import1.J103Id());
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(this._el_1, this._J103Id_1_5.instance);
    }
  }

  @override
  void detectChangesInternal() {
    this._J103Item_0_5.detectHostChanges(this, this._el_0);
    this._J103Id_1_5.detectHostChanges(this, this._el_1);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J103Usa, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J103UsaNgFactory = ComponentFactory<import1.J103Usa>('j103-usa', viewFactory_J103UsaHost0);
ComponentFactory<import1.J103Usa> get J103UsaNgFactory {
  return _J103UsaNgFactory;
}

ComponentFactory<import1.J103Usa> createJ103UsaFactory() {
  return ComponentFactory('j103-usa', viewFactory_J103UsaHost0);
}

final List<Object> styles$J103UsaHost = const [];

class _ViewJ103UsaHost0 extends import10.HostView<import1.J103Usa> {
  @override
  void build() {
    this.componentView = ViewJ103Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J103Usa();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J103Usa> viewFactory_J103UsaHost0() {
  return _ViewJ103UsaHost0();
}

class J103ItemNgCd extends import11.DirectiveChangeDetector {
  final import1.J103Item instance;
  Object? _expr_1;
  J103ItemNgCd(this.instance);
  void detectHostChanges(import12.RenderView view, import2.Element el) {
    bool firstCheck = view.firstCheck;
    if (firstCheck) {
      if ((this.instance.role != null)) {
        import7.updateAttribute(el, 'role', this.instance.role);
      }
      if ((this.instance.fixo != null)) {
        import7.updateClassBindingNonHtml(el, 'fixo', this.instance.fixo);
      }
    }
    final currVal_1 = this.instance.tabIndex;
    if (import13.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.updateAttribute(el, 'tabindex', currVal_1);
      this._expr_1 = currVal_1;
    }
  }
}

class J103IdNgCd extends import11.DirectiveChangeDetector {
  final import1.J103Id instance;
  Object? _expr_1;
  J103IdNgCd(this.instance);
  void detectHostChanges(import12.RenderView view, import2.Element el) {
    bool firstCheck = view.firstCheck;
    if (firstCheck) {
      if ((this.instance.id != null)) {
        import7.updateAttribute(el, 'id', this.instance.id);
      }
    }
    final currVal_1 = this.instance.titulo;
    if (import13.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.updateAttribute(el, 'title', currVal_1);
      this._expr_1 = currVal_1;
    }
  }
}
