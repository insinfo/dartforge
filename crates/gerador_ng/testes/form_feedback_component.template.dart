// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'form_feedback_component.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'form_feedback_component.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;

final List<Object> styles$FormFeedbackComponent = const [];

class ViewFormFeedbackComponent0 extends import0.ComponentView<import1.FormFeedbackComponent> {
  static import2.ComponentStyles? _componentStyles;
  ViewFormFeedbackComponent0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('form-feedback-comp'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:new_sali_frontend/lib/src/shared/components/form_feedback/form_feedback_component.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$FormFeedbackComponent, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _FormFeedbackComponentNgFactory = ComponentFactory<import1.FormFeedbackComponent>('form-feedback-comp', viewFactory_FormFeedbackComponentHost0);
ComponentFactory<import1.FormFeedbackComponent> get FormFeedbackComponentNgFactory {
  return _FormFeedbackComponentNgFactory;
}

ComponentFactory<import1.FormFeedbackComponent> createFormFeedbackComponentFactory() {
  return ComponentFactory('form-feedback-comp', viewFactory_FormFeedbackComponentHost0);
}

final List<Object> styles$FormFeedbackComponentHost = const [];

class _ViewFormFeedbackComponentHost0 extends import8.HostView<import1.FormFeedbackComponent> {
  @override
  void build() {
    this.componentView = ViewFormFeedbackComponent0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.FormFeedbackComponent();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.FormFeedbackComponent> viewFactory_FormFeedbackComponentHost0() {
  return _ViewFormFeedbackComponentHost0();
}
