// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b14_ciclo_completo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b14_ciclo_completo.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;

final List<Object> styles$B14CicloCompleto = const [];

class ViewB14CicloCompleto0 extends import0.ComponentView<import1.B14CicloCompleto> {
  static import2.ComponentStyles? _componentStyles;
  ViewB14CicloCompleto0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b14-ciclo-completo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b14_ciclo_completo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B14CicloCompleto, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B14CicloCompletoNgFactory = ComponentFactory<import1.B14CicloCompleto>('b14-ciclo-completo', viewFactory_B14CicloCompletoHost0);
ComponentFactory<import1.B14CicloCompleto> get B14CicloCompletoNgFactory {
  return _B14CicloCompletoNgFactory;
}

ComponentFactory<import1.B14CicloCompleto> createB14CicloCompletoFactory() {
  return ComponentFactory('b14-ciclo-completo', viewFactory_B14CicloCompletoHost0);
}

final List<Object> styles$B14CicloCompletoHost = const [];

class _ViewB14CicloCompletoHost0 extends import9.HostView<import1.B14CicloCompleto> {
  @override
  void build() {
    this.componentView = ViewB14CicloCompleto0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B14CicloCompleto();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import10.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    if ((!import10.debugThrowIfChanged)) {
      this.component.ngDoCheck();
    }
    if ((!import10.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
      this.component.ngAfterContentChecked();
    }
    this.componentView.detectChanges();
    if ((!import10.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterViewInit();
      }
      this.component.ngAfterViewChecked();
    }
  }

  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
}

import9.HostView<import1.B14CicloCompleto> viewFactory_B14CicloCompletoHost0() {
  return _ViewB14CicloCompletoHost0();
}
