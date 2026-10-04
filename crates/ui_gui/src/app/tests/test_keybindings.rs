use super::test_helpers::create_test_app;
use crate::actions::AppAction;
use crate::keymap::{KeyAction, KeyActionExt, KeyContext, KeymapConfigFile};
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

#[test]
fn test_keymap_remote_servers_and_modal_context() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // 1. Mở RemoteServersModal
    app.push_overlay(OverlayLayer::RemoteServersModal);
    assert_eq!(app.current_key_context(), KeyContext::RemoteServers);

    // 2. Escape trong RemoteServers phải map sang KeyAction::Back (thay vì Dismiss)
    let mut esc_input = RawInput::default();
    esc_input.events.push(eframe::egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(esc_input, |_| {});
    out.textures_delta.clear();
    let action = app.keymap.process_input(&ctx, KeyContext::RemoteServers);
    assert_eq!(action, Some(KeyAction::Back));

    // 3. Down trong RemoteServers phải map sang KeyAction::SelectNext
    let mut down_input = RawInput::default();
    down_input.events.push(eframe::egui::Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out2 = ctx.run_ui(down_input, |_| {});
    out2.textures_delta.clear();
    let action = app.keymap.process_input(&ctx, KeyContext::RemoteServers);
    assert_eq!(action, Some(KeyAction::SelectNext));

    // 4. Tab trong RemoteServers phải map sang KeyAction::TabComplete
    let mut tab_input = RawInput::default();
    tab_input.events.push(eframe::egui::Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out3 = ctx.run_ui(tab_input, |_| {});
    out3.textures_delta.clear();
    let action = app.keymap.process_input(&ctx, KeyContext::RemoteServers);
    assert_eq!(action, Some(KeyAction::TabComplete));

    // 5. Enter trong Modal context (Launch/About) map sang ConfirmSelection
    let mut enter_input = RawInput::default();
    enter_input.events.push(eframe::egui::Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out4 = ctx.run_ui(enter_input, |_| {});
    out4.textures_delta.clear();
    let action = app.keymap.process_input(&ctx, KeyContext::Modal);
    assert_eq!(action, Some(KeyAction::ConfirmSelection));
}

#[test]
fn test_keymap_modal_open_close_and_apply() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();

    // 1. Mở Keymap Modal qua AppAction
    app.dispatch_action(AppAction::OpenKeymapModal);
    assert!(app.is_overlay_open(OverlayLayer::KeymapModal));
    assert_eq!(app.current_key_context(), KeyContext::Modal);
    assert!(app.overlays.keymap_modal_state.is_some());

    // 2. Thay đổi phím trong draft
    if let Some(ref mut state) = app.overlays.keymap_modal_state {
        state
            .draft
            .bind("ctrl-shift-z", KeyAction::ResetZoom, KeyContext::Global);
    }

    // 3. Đóng bằng CloseKeymapModal (Cancel) -> keymap thực tế chưa bị đổi
    app.dispatch_action(AppAction::CloseKeymapModal);
    assert!(!app.is_overlay_open(OverlayLayer::KeymapModal));
    assert!(app.overlays.keymap_modal_state.is_none());
    assert_ne!(
        app.keymap
            .get_label_for_action(&KeyAction::ResetZoom, KeyContext::Global)
            .as_deref(),
        Some("Ctrl+Shift+Z")
    );

    // 4. Mở lại và ApplyKeymapModal
    app.dispatch_action(AppAction::OpenKeymapModal);
    let mut draft = app
        .overlays
        .keymap_modal_state
        .as_ref()
        .unwrap()
        .draft
        .clone();
    draft.bind("ctrl-shift-z", KeyAction::ResetZoom, KeyContext::Global);
    app.dispatch_action(AppAction::ApplyKeymapModal(Box::new(draft)));

    // 5. Kiểm tra phím mới đã có hiệu lực trên app.keymap
    assert!(!app.is_overlay_open(OverlayLayer::KeymapModal));
    assert_eq!(
        app.keymap
            .get_label_for_action(&KeyAction::ResetZoom, KeyContext::Global)
            .as_deref(),
        Some("Ctrl+Shift+Z")
    );

    // 6. Mở lại và test DismissTopLayer
    app.dispatch_action(AppAction::OpenKeymapModal);
    assert!(app.is_overlay_open(OverlayLayer::KeymapModal));
    app.dispatch_action(AppAction::DismissTopLayer);
    assert!(!app.is_overlay_open(OverlayLayer::KeymapModal));
}

#[test]
fn test_keymap_modal_record_key_search() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // 1. Mở Keymap Modal
    app.dispatch_action(AppAction::OpenKeymapModal);
    assert!(app.is_overlay_open(OverlayLayer::KeymapModal));

    // 2. Kích hoạt search_recording (tương tự như khi bấm nút ⌨)
    app.overlays
        .keymap_modal_state
        .as_mut()
        .unwrap()
        .search_recording = true;

    // 3. Giả lập bấm phím Ctrl+F trên bàn phím
    let mut raw_input = RawInput::default();
    raw_input.events.push(eframe::egui::Event::Key {
        key: Key::F,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });

    let mut out = ctx.run_ui(raw_input, |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out.textures_delta.clear();

    // 4. Modal vẫn mở, search_recording được tắt, và search_query được gán giá trị "Ctrl-F"
    assert!(app.is_overlay_open(OverlayLayer::KeymapModal));
    let state = app.overlays.keymap_modal_state.as_ref().unwrap();
    assert!(!state.search_recording);
    assert_eq!(state.search_query, "Ctrl-F");
}

#[test]
fn test_keymap_modal_record_key_search_escape_cancels() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    app.dispatch_action(AppAction::OpenKeymapModal);
    app.overlays
        .keymap_modal_state
        .as_mut()
        .unwrap()
        .search_recording = true;

    let mut raw_input = RawInput::default();
    raw_input.events.push(eframe::egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });

    let mut out = ctx.run_ui(raw_input, |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out.textures_delta.clear();

    assert!(app.is_overlay_open(OverlayLayer::KeymapModal));
    let state = app.overlays.keymap_modal_state.as_ref().unwrap();
    assert!(!state.search_recording);
}

#[test]
fn test_key_customizer_popup_recording_and_escape() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // 1. Mở Keymap modal
    app.dispatch_action(AppAction::OpenKeymapModal);
    assert!(app.is_overlay_open(OverlayLayer::KeymapModal));

    // 2. Kích hoạt chỉnh sửa một action trong popup
    let target_action = uwu_core_keymap::KeyAction::ToggleProjectPicker;
    let target_context = uwu_core_keymap::KeyContext::Global;
    {
        let state = app.overlays.keymap_modal_state.as_mut().unwrap();
        state.recording = Some((target_action.clone(), target_context.clone()));
        state.pending_context = target_context;
        state.is_recording_keystroke = false;
    }

    // 3. Nhấn phím Enter để bắt đầu Record Keystroke
    let mut raw_input = RawInput::default();
    raw_input.events.push(eframe::egui::Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });

    let mut out = ctx.run_ui(raw_input, |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out.textures_delta.clear();

    // Kiểm tra đã vào chế độ record keystroke
    {
        let state = app.overlays.keymap_modal_state.as_ref().unwrap();
        assert!(state.recording.is_some());
        assert!(state.is_recording_keystroke);
    }

    // 4. Nhấn phím Escape: Chỉ hủy mode recording, không làm đóng dialog
    let mut raw_input2 = RawInput::default();
    raw_input2.events.push(eframe::egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });

    let mut out2 = ctx.run_ui(raw_input2, |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out2.textures_delta.clear();

    {
        let state = app.overlays.keymap_modal_state.as_ref().unwrap();
        assert!(
            state.recording.is_some(),
            "Dialog popup vẫn phải mở sau khi hủy record"
        );
        assert!(!state.is_recording_keystroke, "Phải tắt chế độ record");
    }

    // 5. Nhấn Escape lần nữa: Đóng dialog popup
    let mut raw_input3 = RawInput::default();
    raw_input3.events.push(eframe::egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });

    let mut out3 = ctx.run_ui(raw_input3, |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out3.textures_delta.clear();

    {
        let state = app.overlays.keymap_modal_state.as_ref().unwrap();
        assert!(
            state.recording.is_none(),
            "Dialog popup phải đóng khi nhấn Escape lúc không record"
        );
    }
}

#[test]
fn test_key_customizer_context_selection_and_save() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // 1. Mở Keymap modal
    app.dispatch_action(AppAction::OpenKeymapModal);
    assert!(app.is_overlay_open(OverlayLayer::KeymapModal));

    // 2. Kích hoạt chỉnh sửa: gán pending keystroke và đổi context từ Global sang Table
    let target_action = uwu_core_keymap::KeyAction::ToggleProjectPicker;
    let original_context = uwu_core_keymap::KeyContext::Global;
    let new_context = uwu_core_keymap::KeyContext::Table;
    let custom_keystroke = uwu_core_keymap::Keystroke {
        key: uwu_core_keymap::Key::K,
        ctrl: true,
        alt: false,
        shift: false,
        mac_cmd: false,
    };

    {
        let state = app.overlays.keymap_modal_state.as_mut().unwrap();
        state.recording = Some((target_action.clone(), original_context));
        state.pending_keystroke = Some(custom_keystroke);
        state.pending_context = new_context.clone();
        state.context_text = new_context.display_path().to_string();
        state.is_recording_keystroke = false;
    }

    // 3. Render UI để đảm bảo không panic và ComboBox hiển thị chính xác
    let mut out = ctx.run_ui(RawInput::default(), |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out.textures_delta.clear();

    {
        let state = app.overlays.keymap_modal_state.as_ref().unwrap();
        assert_eq!(state.pending_context, new_context);
        assert!(state.recording.is_some());
    }

    // Kiểm tra rằng khi không có phím trùng (Ctrl+K là duy nhất), không có text conflict nào được render
    let shapes_contain_conflict = out.shapes.iter().any(|clipped| match &clipped.shape {
        eframe::egui::epaint::Shape::Text(text_shape) => {
            text_shape
                .galley
                .job
                .text
                .contains("bindings with the same keystrokes")
                || text_shape.galley.job.text.contains("conflicting")
        }
        _ => false,
    });
    assert!(
        !shapes_contain_conflict,
        "Không được hiển thị thông tin conflict khi không trùng phím"
    );

    // 4. Kiểm tra khi CÓ phím trùng: gán pending_keystroke trùng với một action khác
    {
        let state = app.overlays.keymap_modal_state.as_mut().unwrap();
        state.draft.bind(
            "alt-p",
            uwu_core_keymap::KeyAction::OpenLaunchModal,
            uwu_core_keymap::KeyContext::Global,
        );
        state.pending_keystroke = Some(uwu_core_keymap::Keystroke::alt(uwu_core_keymap::Key::P));
    }

    let mut out_conflict = ctx.run_ui(RawInput::default(), |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out_conflict.textures_delta.clear();

    let shapes_contain_conflict_when_duplicated = out_conflict
        .shapes
        .iter()
        .any(|clipped| shape_contains_text(&clipped.shape, "with the same keystrokes"));
    assert!(
        shapes_contain_conflict_when_duplicated,
        "Phải hiển thị thông tin conflict khi có phím trùng"
    );
}

fn shape_contains_text(shape: &eframe::egui::epaint::Shape, target: &str) -> bool {
    match shape {
        eframe::egui::epaint::Shape::Text(t) => t.galley.job.text.contains(target),
        eframe::egui::epaint::Shape::Vec(children) => {
            children.iter().any(|c| shape_contains_text(c, target))
        }
        _ => false,
    }
}

#[test]
fn test_key_customizer_context_typing_and_ctrl_space_autocomplete() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // 1. Mở Keymap modal
    app.dispatch_action(AppAction::OpenKeymapModal);
    assert!(app.is_overlay_open(OverlayLayer::KeymapModal));

    // 2. Kích hoạt chỉnh sửa một action
    let target_action = uwu_core_keymap::KeyAction::ToggleProjectPicker;
    let original_context = uwu_core_keymap::KeyContext::Global;
    let custom_keystroke = uwu_core_keymap::Keystroke {
        key: uwu_core_keymap::Key::P,
        ctrl: true,
        alt: true,
        shift: false,
        mac_cmd: false,
    };

    {
        let state = app.overlays.keymap_modal_state.as_mut().unwrap();
        state.recording = Some((target_action.clone(), original_context.clone()));
        state.pending_keystroke = Some(custom_keystroke);
        state.pending_context = original_context;
        state.context_text = "search".to_string();
        state.context_autocomplete_open = true;
        state.is_recording_keystroke = false;
    }

    // 3. Render 2 frames để egui Area hoàn tất đo đạc và vẽ popover
    let mut first_out = ctx.run_ui(RawInput::default(), |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    first_out.textures_delta.clear();
    let mut out = ctx.run_ui(RawInput::default(), |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out.textures_delta.clear();

    // Kiểm tra state vẫn mở
    {
        let state = app.overlays.keymap_modal_state.as_ref().unwrap();
        assert!(state.recording.is_some());
        assert!(
            state.context_autocomplete_open,
            "context_autocomplete_open phải là true"
        );
    }

    fn collect_texts(shape: &eframe::egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            eframe::egui::epaint::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            eframe::egui::epaint::Shape::Vec(children) => {
                for c in children {
                    collect_texts(c, out);
                }
            }
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        collect_texts(&clipped.shape, &mut texts);
    }

    // Kiểm tra popover hiển thị mục khớp "SearchBar"
    let has_search_suggestion = texts.iter().any(|t| t.contains("SearchBar"));
    assert!(
        has_search_suggestion,
        "Autocomplete popover phải hiển thị mục chứa SearchBar khi tìm kiếm 'search'"
    );

    // 4. Kiểm tra gõ ngữ cảnh tùy biến hoàn toàn (Custom Context) và lưu
    {
        let state = app.overlays.keymap_modal_state.as_mut().unwrap();
        state.context_text = "Plugin > MyConsole".to_string();
        state.context_autocomplete_open = false;
    }

    // Giả lập lưu với custom context
    {
        let state = app.overlays.keymap_modal_state.as_mut().unwrap();
        let target_ctx = uwu_core_keymap::KeyContext::parse(&state.context_text).unwrap();
        state
            .draft
            .bind_keystroke(custom_keystroke, target_action.clone(), target_ctx.clone());
        assert_eq!(target_ctx.display_path(), "Plugin > MyConsole");
        assert_eq!(
            target_ctx.parent(),
            Some(uwu_core_keymap::KeyContext::new("Plugin"))
        );
    }
}

#[test]
fn test_key_customizer_context_syntax_validation_and_feedback() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // 1. Mở Keymap modal
    app.dispatch_action(AppAction::OpenKeymapModal);
    let target_action = uwu_core_keymap::KeyAction::ToggleProjectPicker;
    let target_context = uwu_core_keymap::KeyContext::Global;
    {
        let state = app.overlays.keymap_modal_state.as_mut().unwrap();
        state.recording = Some((target_action.clone(), target_context.clone()));
        state.pending_context = target_context;
        state.is_recording_keystroke = false;
        // Nhập cú pháp sai: phân đoạn rỗng
        state.context_text = "Workspace > > Table".to_string();
    }

    fn collect_texts(shape: &eframe::egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            eframe::egui::epaint::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            eframe::egui::epaint::Shape::Vec(children) => {
                for c in children {
                    collect_texts(c, out);
                }
            }
            _ => {}
        }
    }

    // Pass 1 & 2 để egui tính toán layout
    let mut out1 = ctx.run_ui(RawInput::default(), |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out1.textures_delta.clear();
    let mut out2 = ctx.run_ui(RawInput::default(), |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out2.textures_delta.clear();

    let mut texts = Vec::new();
    for clipped in &out2.shapes {
        collect_texts(&clipped.shape, &mut texts);
    }

    let has_syntax_error = texts.iter().any(|t| t.contains("Syntax error"));
    assert!(
        has_syntax_error,
        "Phải hiển thị cảnh báo lỗi cú pháp khi gõ 'Workspace > > Table'"
    );

    // 2. Nhập cú pháp hợp lệ nhưng là Context chưa có view nào sử dụng (Unknown Custom Context)
    {
        let state = app.overlays.keymap_modal_state.as_mut().unwrap();
        state.context_text = "Workspace > NewExtensionTag".to_string();
    }
    let mut out3 = ctx.run_ui(RawInput::default(), |ui| {
        crate::views::render_ui(ui, &mut app);
    });
    out3.textures_delta.clear();

    texts.clear();
    for clipped in &out3.shapes {
        collect_texts(&clipped.shape, &mut texts);
    }
    let has_custom_hint = texts
        .iter()
        .any(|t| t.contains("Custom context: won't trigger until a view registers this tag"));
    assert!(
        has_custom_hint,
        "Phải hiển thị gợi ý thông tin cho context tùy biến chưa kích hoạt"
    );
}

#[test]
fn test_default_shortcuts_run_command_filter_columns_and_rerun() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // 1. Test Filter shortcut (Ctrl+F)
    assert!(!app.active_session().view.search.focus_requested);
    let mut input_ctrl_f = RawInput::default();
    input_ctrl_f.events.push(eframe::egui::Event::Key {
        key: Key::F,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut out = ctx.run_ui(input_ctrl_f, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
        crate::views::render_ui(ui, &mut app);
    });
    out.textures_delta.clear();
    assert!(
        ctx.memory(|m| m.has_focus(egui::Id::new("search_query_input"))),
        "Ctrl+F phải kích hoạt focus vào ô tìm kiếm/lọc"
    );

    // 2. Test Column Settings shortcut (Alt+C) - không có phím phụ
    assert!(!app.is_overlay_open(OverlayLayer::ColumnsModal));
    let mut input_alt_c = RawInput::default();
    input_alt_c.events.push(eframe::egui::Event::Key {
        key: Key::C,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    });
    let mut out = ctx.run_ui(input_alt_c, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::ALT;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert!(
        app.is_overlay_open(OverlayLayer::ColumnsModal),
        "Alt+C phải mở ColumnsModal"
    );
    app.close_overlay(OverlayLayer::ColumnsModal);

    // Kiểm tra phím phụ cũ (Ctrl+Shift+C) đã được bỏ
    let mut input_ctrl_shift_c = RawInput::default();
    input_ctrl_shift_c.events.push(eframe::egui::Event::Key {
        key: Key::C,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL | Modifiers::SHIFT,
    });
    let mut out = ctx.run_ui(input_ctrl_shift_c, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL | Modifiers::SHIFT;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert!(
        !app.is_overlay_open(OverlayLayer::ColumnsModal),
        "Ctrl+Shift+C không còn được gắn cho ColumnsModal"
    );

    // 3. Test Run Command Settings shortcut (Alt+R) - không có phím phụ
    assert!(!app.is_overlay_open(OverlayLayer::LaunchModal));
    let mut input_alt_r = RawInput::default();
    input_alt_r.events.push(eframe::egui::Event::Key {
        key: Key::R,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    });
    let mut out = ctx.run_ui(input_alt_r, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::ALT;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert!(
        app.is_overlay_open(OverlayLayer::LaunchModal),
        "Alt+R phải mở LaunchModal (Run command settings)"
    );
    app.close_overlay(OverlayLayer::LaunchModal);

    // Kiểm tra phím phụ cũ (Ctrl+Shift+R) đã được bỏ
    let mut input_ctrl_shift_r = RawInput::default();
    input_ctrl_shift_r.events.push(eframe::egui::Event::Key {
        key: Key::R,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL | Modifiers::SHIFT,
    });
    let mut out = ctx.run_ui(input_ctrl_shift_r, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL | Modifiers::SHIFT;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert!(
        !app.is_overlay_open(OverlayLayer::LaunchModal),
        "Ctrl+Shift+R không còn được gắn cho LaunchModal"
    );

    // 4. Test Rerun Command shortcut chính (Ctrl+R)
    let mut input_ctrl_r = RawInput::default();
    input_ctrl_r.events.push(eframe::egui::Event::Key {
        key: Key::R,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut restart_dispatched = false;
    let mut out = ctx.run_ui(input_ctrl_r, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            if matches!(a, AppAction::RestartSource) {
                restart_dispatched = true;
            }
        }
    });
    out.textures_delta.clear();
    assert!(
        restart_dispatched,
        "Ctrl+R phải dispatch AppAction::RestartSource"
    );

    // 5. Test Rerun Command shortcut phụ (F5)
    let mut input_f5 = RawInput::default();
    input_f5.events.push(eframe::egui::Event::Key {
        key: Key::F5,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut f5_restart_dispatched = false;
    let mut out = ctx.run_ui(input_f5, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::NONE;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            if matches!(a, AppAction::RestartSource) {
                f5_restart_dispatched = true;
            }
        }
    });
    out.textures_delta.clear();
    assert!(
        f5_restart_dispatched,
        "F5 phải dispatch AppAction::RestartSource"
    );
}
