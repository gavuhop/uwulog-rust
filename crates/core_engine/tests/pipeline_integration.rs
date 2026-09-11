#![cfg(test)]

use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use uwu_core_engine::SystemEngine;
use uwu_core_schema::LogColor;
use uwu_driver_sources::{FileSource, ProcessSource};

#[tokio::test]
async fn test_full_pipeline_multi_source_and_parallel_filter() {
    let engine = Arc::new(SystemEngine::new(500));

    // 1. Tạo file nguồn log tạm
    let temp_file_path =
        std::env::temp_dir().join(format!("uwu_pipeline_{}.log", uuid::Uuid::new_v4()));
    {
        let mut f = std::fs::File::create(&temp_file_path).unwrap();
        writeln!(
            f,
            "{{\"level\":\"ERROR\",\"message\":\"Database failed\",\"db_port\":5432}}"
        )
        .unwrap();
        writeln!(
            f,
            "{{\"level\":\"INFO\",\"message\":\"Cache warm\",\"cache_keys\":1000}}"
        )
        .unwrap();
        writeln!(f, "[WARN] High memory: 82%").unwrap();
        f.flush().unwrap();
    }

    let file_source = Box::new(FileSource::new(&temp_file_path));
    engine.add_source(file_source).await.unwrap();

    // 2. Thêm Process Source
    #[cfg(target_os = "windows")]
    let proc_source = Box::new(ProcessSource::new(
        "cmd",
        vec![
            "/c".to_string(),
            "echo [INFO] Process worker ready".to_string(),
        ],
    ));

    #[cfg(not(target_os = "windows"))]
    let proc_source = Box::new(ProcessSource::new(
        "echo",
        vec!["[INFO] Process worker ready".to_string()],
    ));

    engine.add_source(proc_source).await.unwrap();

    // Đợi pipeline nạp dữ liệu từ các source
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 3. Kiểm tra tổng số log đã nạp
    assert!(engine.total_logs() >= 4);

    // 4. Test parallel filtering
    let (err_count, err_logs) = engine.search_with_count("level:error", 10);
    assert_eq!(err_count, 1);
    assert_eq!(err_logs[0].color, LogColor::Red);
    assert_eq!(err_logs[0].get_field_cow("level").as_deref(), Some("ERROR"));
    assert_eq!(
        err_logs[0].fields.get("db_port").unwrap(),
        &serde_json::json!(5432)
    );

    // 5. Test numeric range on dynamically inferred field
    let (cache_count, cache_logs) = engine.search_with_count("cache_keys:>=500", 10);
    assert_eq!(cache_count, 1);
    assert_eq!(
        cache_logs[0].fields.get("cache_keys").unwrap(),
        &serde_json::json!(1000)
    );

    // 6. Test append log mới vào file (Live streaming tailing)
    let last_processed = engine.total_processed();
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&temp_file_path)
            .unwrap();
        writeln!(
            f,
            "{{\"level\":\"ERROR\",\"message\":\"Deadlock detected\",\"tx_id\":999}}"
        )
        .unwrap();
        f.flush().unwrap();
    }

    tokio::time::sleep(Duration::from_millis(150)).await;

    // Lọc tăng tiến (Incremental)
    let (new_matched, new_logs) = engine.filter_incremental("level:error", last_processed);
    assert_eq!(new_matched, 1);
    assert_eq!(new_logs.len(), 1);
    assert!(new_logs[0].message.contains("Deadlock detected"));

    // Dọn dẹp file tạm
    let _ = std::fs::remove_file(&temp_file_path);
}
