// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i68_content_child_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i68_content_child_formas.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;

final List<Object> styles$I68ContentChildFormas = const [];

class ViewI68ContentChildFormas0 extends import0.ComponentView<import1.I68ContentChildFormas> {
  static import2.ComponentStyles? _componentStyles;
  ViewI68ContentChildFormas0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i68-content-child-formas'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i68_content_child_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I68ContentChildFormas, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I68ContentChildFormasNgFactory = ComponentFactory<import1.I68ContentChildFormas>('i68-content-child-formas', viewFactory_I68ContentChildFormasHost0);
ComponentFactory<import1.I68ContentChildFormas> get I68ContentChildFormasNgFactory {
  return _I68ContentChildFormasNgFactory;
}

ComponentFactory<import1.I68ContentChildFormas> createI68ContentChildFormasFactory() {
  return ComponentFactory('i68-content-child-formas', viewFactory_I68ContentChildFormasHost0);
}

final List<Object> styles$I68ContentChildFormasHost = const [];

class _ViewI68ContentChildFormasHost0 extends import8.HostView<import1.I68ContentChildFormas> {
  @override
  void build() {
    this.componentView = ViewI68ContentChildFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I68ContentChildFormas();
    this.component.marcas = [];
    this.component.elementos = [];
    this.component.itens = [];
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
    }
    this.componentView.detectChanges();
  }
}

import8.HostView<import1.I68ContentChildFormas> viewFactory_I68ContentChildFormasHost0() {
  return _ViewI68ContentChildFormasHost0();
}
