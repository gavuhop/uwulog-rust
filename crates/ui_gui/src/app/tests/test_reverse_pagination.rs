use super::test_helpers::create_test_app;
use std::time::Duration;
use uwu_core_schema::{RawLogEntry, RawPayload};

#[tokio::test]
async fn test_reverse_pagination_loads_older_logs() {
    let mut app = create_test_app();
    let tx = app.active_session().session.engine.get_channel();

    // 1. Gửi 25 bản ghi
    for i in 0..25 {
        let entry = RawLogEntry {
            payload: RawPayload::Text(format!("[INFO] Message index {:02}", i)),
        };
        tx.send(entry).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Giả lập display_limit nhỏ (10 bản ghi mới nhất ban đầu)
    app.active_session_mut().session.display_limit = 10;
    app.active_session_mut().trigger_full_search();

    assert_eq!(app.active_session().viewport.cached_logs.len(), 10);
    assert_eq!(
        app.active_session().viewport.cached_logs[0].message,
        "[INFO] Message index 15"
    );
    assert_eq!(
        app.active_session().viewport.cached_logs[9].message,
        "[INFO] Message index 24"
    );
    assert!(!app.active_session().viewport.reached_oldest);

    // 2. Cuộn lên đỉnh -> gọi load_more_older_logs(10)
    app.active_session_mut().load_more_older_logs(10);

    // Bây giờ cached_logs phải có 20 bản ghi (index 05..24)
    assert_eq!(app.active_session().viewport.cached_logs.len(), 20);
    assert_eq!(
        app.active_session().viewport.cached_logs[0].message,
        "[INFO] Message index 05"
    );
    assert_eq!(
        app.active_session().viewport.cached_logs[9].message,
        "[INFO] Message index 14"
    );
    assert_eq!(
        app.active_session().viewport.cached_logs[19].message,
        "[INFO] Message index 24"
    );
    // Vị trí scroll duy trì phải bằng số log vừa nạp thêm (10)
    assert_eq!(
        app.active_session().viewport.request_maintain_scroll_offset,
        Some(10)
    );
    assert!(!app.active_session().viewport.reached_oldest);

    // 3. Cuộn tiếp lên đỉnh -> nạp nốt 10 log cũ nhất (chỉ còn 5 bản ghi 00..04)
    app.active_session_mut().load_more_older_logs(10);
    assert_eq!(app.active_session().viewport.cached_logs.len(), 25);
    assert_eq!(
        app.active_session().viewport.cached_logs[0].message,
        "[INFO] Message index 00"
    );
    assert_eq!(
        app.active_session().viewport.cached_logs[4].message,
        "[INFO] Message index 04"
    );
    assert_eq!(
        app.active_session().viewport.request_maintain_scroll_offset,
        Some(5)
    );
    // Vì nạp được 5 < 10 (hết sạch log trong engine), reached_oldest phải bật thành true
    assert!(app.active_session().viewport.reached_oldest);

    // 4. Lần gọi tiếp theo khi reached_oldest = true không làm thay đổi cached_logs
    app.active_session_mut().load_more_older_logs(10);
    assert_eq!(app.active_session().viewport.cached_logs.len(), 25);

    // 5. Bật lại Latch -> trigger_full_search -> reset reached_oldest
    app.active_session_mut().latch();
    assert!(!app.active_session().viewport.reached_oldest);
    assert_eq!(app.active_session().viewport.cached_logs.len(), 10);
    assert_eq!(
        app.active_session().viewport.cached_logs[0].message,
        "[INFO] Message index 15"
    );
}

#[tokio::test]
async fn test_reverse_pagination_with_filter() {
    let mut app = create_test_app();
    let tx = app.active_session().session.engine.get_channel();

    for i in 0..20 {
        let level = if i % 2 == 0 { "ERROR" } else { "INFO" };
        let entry = RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "level": level,
                "message": format!("[{}] Index {:02}", level, i)
            })),
        };
        tx.send(entry).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Filter ERROR (chỉ có các chỉ số chẵn: 0, 2, 4, 6, 8, 10, 12, 14, 16, 18 = 10 logs)
    app.active_session_mut().view.search.query = "level:error".to_string();
    app.active_session_mut().session.display_limit = 5;
    app.active_session_mut().trigger_full_search();

    assert_eq!(app.active_session().viewport.cached_logs.len(), 5);
    // 5 log mới nhất là 10, 12, 14, 16, 18
    assert_eq!(
        app.active_session().viewport.cached_logs[0].message,
        "[ERROR] Index 10"
    );
    assert_eq!(
        app.active_session().viewport.cached_logs[4].message,
        "[ERROR] Index 18"
    );

    // Nạp thêm 5 logs cũ hơn
    app.active_session_mut().load_more_older_logs(5);
    assert_eq!(app.active_session().viewport.cached_logs.len(), 10);
    assert_eq!(
        app.active_session().viewport.cached_logs[0].message,
        "[ERROR] Index 00"
    );
    assert_eq!(
        app.active_session().viewport.cached_logs[4].message,
        "[ERROR] Index 08"
    );
    assert_eq!(
        app.active_session().viewport.cached_logs[9].message,
        "[ERROR] Index 18"
    );
    assert_eq!(
        app.active_session().viewport.request_maintain_scroll_offset,
        Some(5)
    );
}

#[tokio::test]
async fn test_reverse_pagination_unfiltered_stream() {
    let mut app = create_test_app();
    let tx = app.active_session().session.engine.get_channel();

    for i in 0..15 {
        let entry = RawLogEntry {
            payload: RawPayload::Text(format!("[INFO] Raw line {:02}", i)),
        };
        tx.send(entry).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Mở unfiltered stream tại target log index 10
    app.active_session_mut().open_unfiltered_stream(None);
    let all_logs = app.active_session().session.engine.search("");
    let target_id = all_logs[10].id;
    app.active_session_mut()
        .open_unfiltered_stream(Some(target_id));

    // Thu nhỏ cached_unfiltered xuống 5 logs để test load_more
    app.active_session_mut().view.unfiltered.cached_unfiltered = all_logs[10..15].to_vec();
    assert_eq!(app.active_session().unfiltered.cached_unfiltered.len(), 5);

    // Tải thêm 5 logs cũ hơn
    app.active_session_mut().load_more_older_unfiltered(5);
    assert_eq!(app.active_session().unfiltered.cached_unfiltered.len(), 10);
    assert_eq!(
        app.active_session().unfiltered.cached_unfiltered[0].message,
        "[INFO] Raw line 05"
    );
    assert_eq!(
        app.active_session()
            .unfiltered
            .request_maintain_scroll_offset,
        Some(5)
    );
}
