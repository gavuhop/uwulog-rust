use super::test_helpers::create_test_app;
use crate::actions::AppAction;
use crate::keymap::{KeyAction, KeyContext, KeymapConfigFile};
use crate::overlay::OverlayLayer;
use eframe::egui::{Key, Modifiers, RawInput};

#[test]
fn test_keymap_integration_actions() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();

    // 1. Kiểm tra ánh xạ mặc định từ KeyAction sang AppAction
    assert!(matches!(
        KeyAction::Dismiss.to_app_action(),
        Some(AppAction::DismissTopLayer)
    ));
    assert!(matches!(
        KeyAction::ToggleProjectPicker.to_app_action(),
        Some(AppAction::ToggleProjectPicker)
    ));
    assert!(matches!(
        KeyAction::PreviousSession.to_app_action(),
        Some(AppAction::CycleSession(false))
    ));
    assert!(matches!(
        KeyAction::NextSession.to_app_action(),
        Some(AppAction::CycleSession(true))
    ));

    // 2. Mở Project Picker modal
    app.push_overlay(OverlayLayer::ProjectPicker);
    assert!(app.is_overlay_open(OverlayLayer::ProjectPicker));
    assert_eq!(app.current_key_context(), KeyContext::Modal);

    // 3. Dispatch action DismissTopLayer đóng modal
    app.dispatch_action(AppAction::DismissTopLayer);
    assert!(!app.is_overlay_open(OverlayLayer::ProjectPicker));
    assert_eq!(app.current_key_context(), KeyContext::Global);
}

#[test]
fn test_keymap_custom_user_override_flow() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();

    // Mặc định, phím ToggleProjectPicker có nhãn là "Alt+P"
    let default_label = app
        .keymap
        .get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
    assert_eq!(default_label.as_deref(), Some("Alt+P"));

    // Giả lập nạp cấu hình người dùng từ keymap.json:
    // Đổi phím sang "ctrl-shift-p" (phong cách VS Code / Zed Palette)
    let user_json = r#"[
        {
            "context": "Global",
            "bindings": {
                "ctrl-shift-p": "workspace::ToggleProjectPicker"
            }
        }
    ]"#;

    let user_config: KeymapConfigFile = serde_json::from_str(user_json).unwrap();
    user_config.apply_to(&mut app.keymap);

    // Nhãn mới phải được cập nhật thành "Ctrl+Shift+P"
    let updated_label = app
        .keymap
        .get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
    assert_eq!(updated_label.as_deref(), Some("Ctrl+Shift+P"));
}

#[test]
fn test_keymap_context_aware_input_processing() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // Giả lập phím bấm: Escape
    let mut raw_input = RawInput::default();
    raw_input.events.push(eframe::egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });

    let mut output = ctx.run_ui(raw_input, |_| {});
    output.textures_delta.clear();

    // Khi ở Global context: Escape trả về KeyAction::Dismiss
    let action = app.keymap.process_input(&ctx, KeyContext::Global);
    assert_eq!(action, Some(KeyAction::Dismiss));
}
