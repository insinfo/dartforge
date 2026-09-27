// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j05_usa_host_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j05_usa_host_formas.dart' as import1;
import 'j05_diretiva_host_formas.template.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'j05_diretiva_host_formas.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$J05UsaHostFormas = const [];

class ViewJ05UsaHostFormas0 extends import0.ComponentView<import1.J05UsaHostFormas> {
  late final import2.J05DiretivaHostFormasNgCd _J05DiretivaHostFormas_0_5;
  late final import3.DivElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewJ05UsaHostFormas0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j05-usa-host-formas'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j05_usa_host_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendDiv(doc, parentRenderNode);
    import8.setAttribute(this._el_0, 'j05-host', '');
    this._J05DiretivaHostFormas_0_5 = import2.J05DiretivaHostFormasNgCd(import9.J05DiretivaHostFormas());
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(this._el_0, this._J05DiretivaHostFormas_0_5.instance);
    }
    final _text_1 = import8.appendText(this._el_0, 'x');
  }

  @override
  void detectChangesInternal() {
    this._J05DiretivaHostFormas_0_5.detectHostChanges(this, this._el_0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J05UsaHostFormas, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J05UsaHostFormasNgFactory = ComponentFactory<import1.J05UsaHostFormas>('j05-usa-host-formas', viewFactory_J05UsaHostFormasHost0);
ComponentFactory<import1.J05UsaHostFormas> get J05UsaHostFormasNgFactory {
  return _J05UsaHostFormasNgFactory;
}

ComponentFactory<import1.J05UsaHostFormas> createJ05UsaHostFormasFactory() {
  return ComponentFactory('j05-usa-host-formas', viewFactory_J05UsaHostFormasHost0);
}

final List<Object> styles$J05UsaHostFormasHost = const [];

class _ViewJ05UsaHostFormasHost0 extends import12.HostView<import1.J05UsaHostFormas> {
  @override
  void build() {
    this.componentView = ViewJ05UsaHostFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J05UsaHostFormas();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J05UsaHostFormas> viewFactory_J05UsaHostFormasHost0() {
  return _ViewJ05UsaHostFormasHost0();
}
