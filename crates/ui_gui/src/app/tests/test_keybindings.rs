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

    // 6. Test Search History shortcut (Ctrl+H)
    assert!(!app.active_session().view.search.history.is_open);
    let mut input_ctrl_h = RawInput::default();
    input_ctrl_h.events.push(eframe::egui::Event::Key {
        key: Key::H,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut out = ctx.run_ui(input_ctrl_h, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert!(
        app.active_session().view.search.history.is_open,
        "Ctrl+H phải mở dropdown lịch sử tìm kiếm (search history)"
    );

    // Bấm Ctrl+H lần nữa để toggle đóng
    let mut input_ctrl_h_toggle = RawInput::default();
    input_ctrl_h_toggle.events.push(eframe::egui::Event::Key {
        key: Key::H,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut out = ctx.run_ui(input_ctrl_h_toggle, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert!(
        !app.active_session().view.search.history.is_open,
        "Ctrl+H lần thứ hai phải toggle đóng dropdown lịch sử tìm kiếm"
    );

    // 7. Test Stream View Switching shortcuts (Ctrl+Tab, Alt+1, Alt+2)
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );

    // Khi CHƯA có filter -> Bấm Ctrl+Tab KHÔNG được đổi tab
    let mut input_ctrl_tab = RawInput::default();
    input_ctrl_tab.events.push(eframe::egui::Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut out = ctx.run_ui(input_ctrl_tab.clone(), |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered,
        "Khi chưa có filter, Ctrl+Tab không được đổi sang Raw View"
    );

    // Đặt bộ lọc tìm kiếm
    app.active_session_mut().view.search.query = "level:error".to_string();

    // Khi ĐÃ có filter -> Bấm Ctrl+Tab chuyển sang Raw View (Unfiltered)
    let mut out = ctx.run_ui(input_ctrl_tab, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Unfiltered,
        "Ctrl+Tab khi có filter phải toggle sang Raw View (Unfiltered)"
    );

    // Bấm Ctrl+Tab lần nữa -> quay về Main View (Filtered)
    let mut input_ctrl_tab2 = RawInput::default();
    input_ctrl_tab2.events.push(eframe::egui::Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut out = ctx.run_ui(input_ctrl_tab2, |ui| {
        ui.ctx().input_mut(|i| {
            i.modifiers = Modifiers::CTRL;
        });
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered,
        "Ctrl+Tab lần 2 phải toggle về Main View (Filtered)"
    );

    // Bấm Alt+2 -> trực tiếp nhảy sang Raw View
    let mut input_alt_2 = RawInput::default();
    input_alt_2.events.push(eframe::egui::Event::Key {
        key: Key::Num2,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    });
    let mut out = ctx.run_ui(input_alt_2, |ui| {
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
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Unfiltered,
        "Alt+2 phải kích hoạt trực tiếp Raw View (Unfiltered)"
    );

    // Bấm Alt+1 -> trực tiếp nhảy về Main View
    let mut input_alt_1 = RawInput::default();
    input_alt_1.events.push(eframe::egui::Event::Key {
        key: Key::Num1,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    });
    let mut out = ctx.run_ui(input_alt_1, |ui| {
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
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered,
        "Alt+1 phải kích hoạt trực tiếp Main View (Filtered)"
    );

    // 8. Test View Raw Context (Alt+V)
    let mock_event = uwu_core_schema::LogEvent::new(
        "2026-10-04T10:00:00Z",
        uwu_core_schema::LogColor::Default,
        "test log event",
        uwu_core_schema::LogFields::default(),
    )
    .with_id(42);
    app.active_session_mut().view.inspector.selected_log = Some(mock_event);

    // 8.1 Khi chưa có filter: Alt+V không được đổi sang Raw View
    app.active_session_mut().view.search.clear();

    let mut input_alt_v = RawInput::default();
    input_alt_v.events.push(eframe::egui::Event::Key {
        key: Key::V,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    });
    let mut out = ctx.run_ui(input_alt_v.clone(), |ui| {
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
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered,
        "Khi chưa có filter, Alt+V không được đổi sang Raw View"
    );

    // 8.2 Khi đã có filter: Alt+V chuyển sang Raw View và định vị đúng target_id
    app.active_session_mut().view.search.query = "level:error".to_string();

    let mut out = ctx.run_ui(input_alt_v.clone(), |ui| {
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
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Unfiltered,
        "Alt+V khi có filter phải mở Raw View (Unfiltered)"
    );
    assert_eq!(
        app.active_session().view.unfiltered.target_id,
        Some(42),
        "Alt+V phải định vị đúng target_id của dòng log đã chọn trong raw stream"
    );

    // 8.3 Khi đang ở màn Raw mà bấm Alt+V lần nữa: chuyển ngược về Main View (Filtered)
    let mut out = ctx.run_ui(input_alt_v, |ui| {
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
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered,
        "Alt+V khi đang ở màn Raw phải chuyển ngược về Main View (Filtered)"
    );
}

#[test]
fn test_arrow_keys_row_navigation_and_scroll() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // Chuẩn bị 5 dòng log mẫu trong cached_logs
    let events: Vec<uwu_core_schema::LogEvent> = (0..5)
        .map(|i| {
            uwu_core_schema::LogEvent::new(
                "2026-10-04T10:00:00Z",
                uwu_core_schema::LogColor::Default,
                format!("log row {i}"),
                uwu_core_schema::LogFields::default(),
            )
            .with_id(100 + i)
        })
        .collect();
    app.active_session_mut().view.viewport.cached_logs = events.clone();

    // --- Trường hợp 1: Đã chọn 1 dòng log -> Phím lên / xuống đổi dòng được chọn ---
    app.active_session_mut().view.inspector.selected_log = Some(events[1].clone()); // Đang chọn dòng 1

    // Bấm ArrowDown
    let mut input_down = RawInput::default();
    input_down.events.push(eframe::egui::Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_down, |ui| {
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();

    assert_eq!(
        app.active_session()
            .view
            .inspector
            .selected_log
            .as_ref()
            .map(|e| e.id),
        Some(102),
        "ArrowDown phải chuyển selection từ dòng 1 (id 101) sang dòng 2 (id 102)"
    );

    // Bấm ArrowUp
    let mut input_up = RawInput::default();
    input_up.events.push(eframe::egui::Event::Key {
        key: Key::ArrowUp,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_up, |ui| {
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();

    assert_eq!(
        app.active_session()
            .view
            .inspector
            .selected_log
            .as_ref()
            .map(|e| e.id),
        Some(101),
        "ArrowUp phải chuyển selection từ dòng 2 (id 102) quay lại dòng 1 (id 101)"
    );

    // --- Trường hợp 2: Không focus/chọn dòng nào -> Phím lên / xuống cuộn bảng ---
    app.active_session_mut().view.inspector.selected_log = None;
    app.active_session_mut().view.viewport.first_visible_row = Some(1);

    // Bấm ArrowDown khi không có log được chọn
    let mut input_down = RawInput::default();
    input_down.events.push(eframe::egui::Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_down, |ui| {
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();

    assert!(
        app.active_session().view.inspector.selected_log.is_none(),
        "Khi không có dòng nào được chọn, ArrowDown không được tự ý chọn dòng"
    );
    assert_eq!(
        app.active_session().view.viewport.request_scroll_to_row,
        Some((4, Some(eframe::egui::Align::Min))),
        "ArrowDown phải cuộn bảng xuống (1 + 4 = 4 khi max là 4)"
    );

    // --- Trường hợp 3: Khi ô tìm kiếm nhận focus -> Arrow keys không bị Table nuốt ---
    let search_id = eframe::egui::Id::new("search_query_input");
    let input_focus = RawInput::default();
    let mut out = ctx.run_ui(input_focus, |ui| {
        ui.ctx().memory_mut(|m| m.request_focus(search_id));
    });
    out.textures_delta.clear();

    let mut input_down = RawInput::default();
    input_down.events.push(eframe::egui::Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_down, |ui| {
        let active_ctx = app.resolve_active_key_context(ui.ctx());
        assert_eq!(
            active_ctx,
            KeyContext::SearchInput,
            "Khi search input có focus, active context phải là SearchInput"
        );
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        assert!(
            dispatched.is_empty(),
            "Trong SearchInput context (không mở autocomplete), ArrowDown không được dispatch NavigateDown"
        );
    });
    out.textures_delta.clear();
}

#[test]
fn test_excel_like_navigation_and_copy_shortcuts() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = create_test_app();
    let ctx = eframe::egui::Context::default();

    // 1. Tạo 50 dòng log mẫu
    let mut fields = uwu_core_schema::LogFields::default();
    fields.insert("status".to_string(), serde_json::json!(200));
    fields.insert("user".to_string(), serde_json::json!("admin"));

    let events: Vec<uwu_core_schema::LogEvent> = (0..50)
        .map(|i| {
            uwu_core_schema::LogEvent::new(
                "2026-10-05T10:00:00Z",
                uwu_core_schema::LogColor::Default,
                format!("{{\"id\":{i},\"status\":200,\"user\":\"admin\"}}"),
                fields.clone(),
            )
            .with_id(200 + i)
        })
        .collect();
    app.active_session_mut().view.viewport.cached_logs = events.clone();

    // --- Kiểm tra 1: Sao chép Ctrl+C theo trạng thái Beauty / Raw ---
    app.active_session_mut().view.inspector.selected_log = Some(events[0].clone());

    // 1.1 Chế độ Raw: Ctrl+C sao chép raw string
    app.active_session_mut().view.inspector.is_beauty_payload = false;
    let raw_payload = app
        .active_session()
        .view
        .inspector
        .formatted_payload_for_copy()
        .unwrap();
    assert!(
        !raw_payload.contains('\n'),
        "Raw copy không chứa ký tự xuống dòng"
    );

    let mut input_ctrl_c = RawInput::default();
    input_ctrl_c.events.push(eframe::egui::Event::Key {
        key: Key::C,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut out = ctx.run_ui(input_ctrl_c, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::CTRL);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();

    // 1.2 Chế độ Beauty: Ctrl+C sao chép formatted indented JSON
    app.active_session_mut().view.inspector.is_beauty_payload = true;
    let beauty_payload = app
        .active_session()
        .view
        .inspector
        .formatted_payload_for_copy()
        .unwrap();
    assert!(
        beauty_payload.contains('\n') && beauty_payload.contains("  \"status\": 200"),
        "Beauty copy phải là chuỗi JSON có thụt dòng đẹp"
    );

    // 1.3 Kiểm tra Ctrl+C sao chép chính xác dòng đang pick (kể cả qua egui-winit Event::Copy)
    app.active_session_mut().view.inspector.selected_log = Some(events[3].clone());
    let mut input_event_copy = RawInput::default();
    input_event_copy.events.push(eframe::egui::Event::Copy);
    let mut out = ctx.run_ui(input_event_copy, |ui| {
        app.egui_ctx = Some(ui.ctx().clone());
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert!(
        out.platform_output.commands.iter().any(|c| matches!(c, eframe::egui::OutputCommand::CopyText(t) if t == &events[3].beauty_display().into_owned())),
        "Ctrl+C qua Event::Copy phải sao chép toàn bộ dòng log đang pick"
    );

    // 1.4 Khi có text đang bôi đen: Ctrl+C ưu tiên sao chép đúng đoạn bôi đen đó
    let dummy_id = eframe::egui::Id::new("test_cell");
    let mut state = eframe::egui::text_edit::TextEditState::default();
    state
        .cursor
        .set_char_range(Some(eframe::egui::text::CCursorRange::two(
            eframe::egui::text::CCursor::new(2),
            eframe::egui::text::CCursor::new(7),
        )));
    state.store(&ctx, dummy_id);
    ctx.memory_mut(|m| m.request_focus(dummy_id));
    ctx.data_mut(|d| d.insert_temp(dummy_id, "admin".to_string()));

    let mut input_sel_copy = RawInput::default();
    input_sel_copy.events.push(eframe::egui::Event::Copy);
    let mut out = ctx.run_ui(input_sel_copy, |ui| {
        app.egui_ctx = Some(ui.ctx().clone());
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert!(
        out.platform_output
            .commands
            .iter()
            .any(|c| matches!(c, eframe::egui::OutputCommand::CopyText(t) if t == "admin")),
        "Khi có text bôi đen, Ctrl+C phải ưu tiên sao chép đúng đoạn chữ bôi đen"
    );

    // Dọn dẹp focus sau test
    ctx.memory_mut(|m| m.surrender_focus(dummy_id));
    ctx.data_mut(|d| d.remove_temp::<String>(dummy_id));

    // --- Kiểm tra 2: PageUp / PageDown nhảy 20 dòng nhưng vẫn GIỮ NGUYÊN dòng đang chọn ---
    app.active_session_mut().view.inspector.selected_log = Some(events[5].clone()); // Dòng 5
    app.active_session_mut().view.viewport.first_visible_row = Some(5);

    // Bấm PageDown -> cuộn bảng xuống dòng 25 (5 + 20), nhưng dòng pick vẫn là dòng 5 (id 205)
    let mut input_pagedown = RawInput::default();
    input_pagedown.events.push(eframe::egui::Event::Key {
        key: Key::PageDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_pagedown, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::NONE);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session()
            .view
            .inspector
            .selected_log
            .as_ref()
            .map(|e| e.id),
        Some(205),
        "PageDown vẫn phải giữ nguyên dòng 5 (id 205) đang chọn"
    );
    assert_eq!(
        app.active_session().view.viewport.request_scroll_to_row,
        Some((25, Some(eframe::egui::Align::Min))),
        "PageDown phải yêu cầu cuộn bảng xuống dòng 25"
    );

    // Cập nhật first_visible_row lên dòng 25
    app.active_session_mut().view.viewport.first_visible_row = Some(25);

    // Bấm PageUp -> cuộn bảng ngược lại dòng 5 (25 - 20), vẫn giữ nguyên dòng pick 5 (id 205)
    let mut input_pageup = RawInput::default();
    input_pageup.events.push(eframe::egui::Event::Key {
        key: Key::PageUp,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_pageup, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::NONE);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session()
            .view
            .inspector
            .selected_log
            .as_ref()
            .map(|e| e.id),
        Some(205),
        "PageUp vẫn phải giữ nguyên dòng 5 (id 205) đang chọn"
    );
    assert_eq!(
        app.active_session().view.viewport.request_scroll_to_row,
        Some((5, Some(eframe::egui::Align::Min))),
        "PageUp phải yêu cầu cuộn bảng ngược lên dòng 5"
    );

    // --- Kiểm tra 3: Ctrl + ArrowDown / Ctrl + End nhảy về đáy bảng ---
    let mut input_ctrl_down = RawInput::default();
    input_ctrl_down.events.push(eframe::egui::Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut out = ctx.run_ui(input_ctrl_down, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::CTRL);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session()
            .view
            .inspector
            .selected_log
            .as_ref()
            .map(|e| e.id),
        Some(249),
        "Ctrl+ArrowDown phải nhảy tới dòng cuối cùng (dòng 49, id 249)"
    );
    assert!(
        app.active_session().view.viewport.is_auto_scroll,
        "Nhảy tới cuối bảng phải tự động bật follow mode (latch)"
    );

    // Bấm phím End đứng một mình -> nhảy tới dòng cuối cùng và bật Latch
    app.active_session_mut().view.inspector.selected_log = Some(events[20].clone());
    app.active_session_mut().view.viewport.is_auto_scroll = false;
    let mut input_end = RawInput::default();
    input_end.events.push(eframe::egui::Event::Key {
        key: Key::End,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_end, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::NONE);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session()
            .view
            .inspector
            .selected_log
            .as_ref()
            .map(|e| e.id),
        Some(249),
        "Phím End phải nhảy tới dòng cuối cùng (dòng 49, id 249)"
    );
    assert!(
        app.active_session().view.viewport.is_auto_scroll,
        "Phím End phải tự động kích hoạt Latch (Auto-scroll)"
    );

    // --- Kiểm tra 4: Ctrl + ArrowUp / Ctrl + Home / Home nhảy về đỉnh bảng ---
    let mut input_ctrl_up = RawInput::default();
    input_ctrl_up.events.push(eframe::egui::Event::Key {
        key: Key::ArrowUp,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    });
    let mut out = ctx.run_ui(input_ctrl_up, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::CTRL);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session()
            .view
            .inspector
            .selected_log
            .as_ref()
            .map(|e| e.id),
        Some(200),
        "Ctrl+ArrowUp phải nhảy về dòng đầu tiên (dòng 0, id 200)"
    );

    // Bấm phím Home đứng một mình -> nhảy về dòng đầu tiên
    app.active_session_mut().view.inspector.selected_log = Some(events[20].clone());
    let mut input_home = RawInput::default();
    input_home.events.push(eframe::egui::Event::Key {
        key: Key::Home,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_home, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::NONE);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session()
            .view
            .inspector
            .selected_log
            .as_ref()
            .map(|e| e.id),
        Some(200),
        "Phím Home phải nhảy về dòng đầu tiên (dòng 0, id 200)"
    );

    // --- Kiểm tra 5: ArrowLeft / ArrowRight cuộn ngang bảng ---
    let mut input_arrow_right = RawInput::default();
    input_arrow_right.events.push(eframe::egui::Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_arrow_right, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::NONE);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session_mut()
            .consume_horizontal_scroll(crate::state::ActiveTab::Filtered),
        Some(120.0),
        "ArrowRight phải yêu cầu cuộn ngang sang phải 120.0 px"
    );

    let mut input_arrow_left = RawInput::default();
    input_arrow_left.events.push(eframe::egui::Event::Key {
        key: Key::ArrowLeft,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut out = ctx.run_ui(input_arrow_left, |ui| {
        ui.ctx().input_mut(|i| i.modifiers = Modifiers::NONE);
        let mut dispatched = Vec::new();
        app.handle_keybindings(ui.ctx(), &mut |act| dispatched.push(act));
        for a in dispatched {
            app.dispatch_action(a);
        }
    });
    out.textures_delta.clear();
    assert_eq!(
        app.active_session_mut()
            .consume_horizontal_scroll(crate::state::ActiveTab::Filtered),
        Some(-120.0),
        "ArrowLeft phải yêu cầu cuộn ngang sang trái -120.0 px"
    );
}

#[tokio::test]
async fn test_tab_switching_prevented_without_filter() {
    let mut app = create_test_app();
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );

    // 1. Khi chưa có filter: các action chuyển sang Unfiltered đều bị chặn
    app.dispatch_action(AppAction::SwitchTab(crate::state::ActiveTab::Unfiltered));
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );

    app.dispatch_action(AppAction::ToggleStreamView);
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );

    app.dispatch_action(AppAction::ViewRawContext);
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );

    app.dispatch_action(AppAction::OpenUnfilteredStream(None));
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );

    // 2. Khi đã có filter: các action chuyển sang Unfiltered hoạt động bình thường
    app.active_session_mut().view.search.query = "error".to_string();

    app.dispatch_action(AppAction::SwitchTab(crate::state::ActiveTab::Unfiltered));
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Unfiltered
    );

    app.dispatch_action(AppAction::SwitchTab(crate::state::ActiveTab::Filtered));
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );

    app.dispatch_action(AppAction::ToggleStreamView);
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Unfiltered
    );

    // 3. Khi đang ở Unfiltered mà ClearQuery -> tự động đóng Unfiltered và quay về Filtered
    app.dispatch_action(AppAction::ClearQuery);
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );
    assert!(!app.active_session().view.unfiltered.is_open);

    // 4. Khi đang ở Unfiltered mà bấm ViewRawContext (Alt+V) -> chuyển ngược về Filtered (Main)
    app.active_session_mut().view.search.query = "error".to_string();
    app.dispatch_action(AppAction::SwitchTab(crate::state::ActiveTab::Unfiltered));
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Unfiltered
    );
    app.dispatch_action(AppAction::ViewRawContext);
    assert_eq!(
        app.active_session().view.active_tab,
        crate::state::ActiveTab::Filtered
    );
}
