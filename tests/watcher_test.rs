use sandsifter::ingest::watcher::{
    is_candidate_video, is_file_stable_and_unlocked, WatchFolderDaemon, WatcherConfig,
};
use std::fs::File;
use std::io::Write;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_is_candidate_video_extensions() {
    let extensions = vec![
        "mp4".to_string(),
        "mov".to_string(),
        "mkv".to_string(),
        "webm".to_string(),
    ];

    let dir = tempdir().unwrap();

    // Valid video files
    let valid_mp4 = dir.path().join("render_final.mp4");
    File::create(&valid_mp4).unwrap();
    assert!(is_candidate_video(&valid_mp4, &extensions));

    let valid_mov = dir.path().join("cut_v02.MOV");
    File::create(&valid_mov).unwrap();
    assert!(is_candidate_video(&valid_mov, &extensions));

    // Invalid non-video extensions
    let text_file = dir.path().join("notes.txt");
    File::create(&text_file).unwrap();
    assert!(!is_candidate_video(&text_file, &extensions));

    // Temporary NLE render locks / partial downloads
    let tmp_file = dir.path().join("render_final.mp4.tmp");
    File::create(&tmp_file).unwrap();
    assert!(!is_candidate_video(&tmp_file, &extensions));

    let crdownload = dir.path().join("video.mp4.crdownload");
    File::create(&crdownload).unwrap();
    assert!(!is_candidate_video(&crdownload, &extensions));

    let hidden_file = dir.path().join(".hidden_render.mp4");
    File::create(&hidden_file).unwrap();
    assert!(!is_candidate_video(&hidden_file, &extensions));

    let office_temp = dir.path().join("~$export.mp4");
    File::create(&office_temp).unwrap();
    assert!(!is_candidate_video(&office_temp, &extensions));
}

#[test]
fn test_is_file_stable_and_unlocked() {
    let dir = tempdir().unwrap();
    let empty_file = dir.path().join("empty.mp4");
    File::create(&empty_file).unwrap();

    // 0-byte file is rejected as unstable/empty render
    assert!(!is_file_stable_and_unlocked(&empty_file, Duration::from_millis(10)));

    // File with contents and zero debounce should pass
    let ready_file = dir.path().join("ready.mp4");
    let mut f = File::create(&ready_file).unwrap();
    f.write_all(b"RIFF....WAVEfmt ....data....fake video data").unwrap();
    drop(f);

    assert!(is_file_stable_and_unlocked(&ready_file, Duration::from_millis(50)));

    // Non-existent path returns false
    let missing = dir.path().join("missing.mp4");
    assert!(!is_file_stable_and_unlocked(&missing, Duration::from_millis(10)));
}

#[test]
fn test_watch_folder_daemon_scan_once() {
    let watch_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    // Create a mock video draft inside the watch folder
    let mock_video = watch_dir.path().join("cabin_build_final_v1.mp4");
    let mut f = File::create(&mock_video).unwrap();
    f.write_all(b"MOCK VIDEO CONTENT TO SIMULATE RENDER FILE").unwrap();
    drop(f);

    let config = WatcherConfig {
        watch_dir: watch_dir.path().to_path_buf(),
        output_dir: output_dir.path().to_path_buf(),
        debounce_secs: 0,
        poll_interval_secs: 1,
        supported_extensions: vec!["mp4".to_string(), "mov".to_string()],
    };

    let mut daemon = WatchFolderDaemon::new(config);

    // Initial scan should detect and process the newly exported draft
    let results = daemon.scan_once();
    assert_eq!(results.len(), 1);

    let res = &results[0];
    assert_eq!(res.video_title, "cabin build final v1");
    assert!(res.flight_sheet_path.exists());
    assert!(res.fcpxml_path.exists());
    assert!(res.edl_path.exists());

    // Verify generated FCPXML contains the template header and slug asset
    let fcpxml_content = std::fs::read_to_string(&res.fcpxml_path).unwrap();
    assert!(fcpxml_content.contains("<!DOCTYPE fcpxml>"));
    assert!(fcpxml_content.contains("SandsifterForensicFormat"));

    // Verify generated EDL has standard CMX3600 header
    let edl_content = std::fs::read_to_string(&res.edl_path).unwrap();
    assert!(edl_content.contains("TITLE:"));

    // Verify Flight Sheet has plain English targets with zero LaTeX
    let flight_sheet_content = std::fs::read_to_string(&res.flight_sheet_path).unwrap();
    assert!(flight_sheet_content.contains("Editor's Pre-Publish Draft Sheet"));
    for bad in ["$\\le$", "$\\ge$", "\\text{", "\\%"] {
        assert!(!flight_sheet_content.contains(bad), "LaTeX found: {}", bad);
    }

    // Second scan without file changes should be idempotent and return 0
    let second_scan = daemon.scan_once();
    assert_eq!(second_scan.len(), 0);
}

#[test]
fn test_watch_folder_daemon_detects_modified_render() {
    let watch_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    let mock_video = watch_dir.path().join("episode_04_draft.mov");
    let mut f = File::create(&mock_video).unwrap();
    f.write_all(b"VERSION 1 RENDER").unwrap();
    drop(f);

    let config = WatcherConfig {
        watch_dir: watch_dir.path().to_path_buf(),
        output_dir: output_dir.path().to_path_buf(),
        debounce_secs: 0,
        poll_interval_secs: 1,
        supported_extensions: vec!["mov".to_string()],
    };

    let mut daemon = WatchFolderDaemon::new(config);
    let r1 = daemon.scan_once();
    assert_eq!(r1.len(), 1);

    // Sleep briefly and re-export / overwrite the file with more content
    std::thread::sleep(Duration::from_millis(50));
    let mut f2 = File::create(&mock_video).unwrap();
    f2.write_all(b"VERSION 2 RE-RENDER OVERWRITE WITH LONGER RUNTIME").unwrap();
    drop(f2);

    // Scan should detect that the file was modified and re-evaluate
    let r2 = daemon.scan_once();
    assert_eq!(r2.len(), 1);
}

#[tokio::test]
async fn test_watch_folder_daemon_graceful_shutdown() {
    let watch_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    let config = WatcherConfig {
        watch_dir: watch_dir.path().to_path_buf(),
        output_dir: output_dir.path().to_path_buf(),
        debounce_secs: 0,
        poll_interval_secs: 1,
        supported_extensions: vec!["mp4".to_string()],
    };

    let mut daemon = WatchFolderDaemon::new(config);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let handle = tokio::spawn(async move {
        daemon.run_daemon(Some(shutdown_rx)).await
    });

    // Allow daemon to spin up for 50ms, then trigger shutdown
    tokio::time::sleep(Duration::from_millis(50)).await;
    shutdown_tx.send(()).unwrap();

    let res = tokio::time::timeout(Duration::from_secs(3), handle).await;
    assert!(res.is_ok(), "Daemon should terminate within timeout upon shutdown signal");
    let join_res = res.unwrap();
    assert!(join_res.is_ok());
    assert!(join_res.unwrap().is_ok());
}
