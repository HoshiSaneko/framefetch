mod browser_cookies;
mod webview_url;
mod media_tools;
mod xiaohongshu;
mod xiaohongshu_download;
mod bilibili;
mod bilibili_download;
mod removal;
mod download;
mod work_files;
mod storage_layout;
mod douyin;
mod douyin_download;
mod douyin_batch;
mod commands;
mod engine;
mod links;
mod models;
mod providers;
mod qr_login;
mod telegram;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(douyin::LoginGate::default())
        .manage(bilibili::LoginGate::default())
        .manage(xiaohongshu::LoginGate::default())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            bilibili::clean_stale_exports(app.handle()).map_err(std::io::Error::other)?;
            let root = app.path().app_data_dir()?;
            #[cfg(target_os = "macos")]
            let downloads = app.path().download_dir()?.join("FrameFetch");
            #[cfg(not(target_os = "macos"))]
            let executable = std::env::current_exe()?;
            #[cfg(not(target_os = "macos"))]
            let downloads = executable
                .parent()
                .ok_or_else(|| std::io::Error::other("无法确定程序所在目录"))?
                .join("downloads");
            let engine = engine::Engine::load(root, downloads).map_err(std::io::Error::other)?;
            app.manage(engine::Shared(engine.clone()));
            engine.start_scheduler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            xiaohongshu::xiaohongshu_qr_start,
            xiaohongshu::xiaohongshu_qr_poll,
            xiaohongshu::xiaohongshu_qr_cancel,
            xiaohongshu::xiaohongshu_login_open,
            xiaohongshu::xiaohongshu_login_status,
            xiaohongshu::xiaohongshu_login_finish,
            xiaohongshu::xiaohongshu_logout,
            xiaohongshu_download::xiaohongshu_preview,
            xiaohongshu_download::enqueue_xiaohongshu,
            bilibili::bilibili_login_open,
            bilibili::bilibili_profile,
            bilibili::bilibili_qr_start,
            bilibili::bilibili_qr_poll,
            bilibili::bilibili_qr_cancel,
            bilibili::bilibili_login_status,
            bilibili::bilibili_login_finish,
            bilibili::bilibili_logout,
            bilibili_download::bilibili_preview,
            bilibili_download::enqueue_bilibili,
            douyin::douyin_login_open,
            douyin::douyin_profile,
            douyin_download::douyin_library,
            douyin_batch::enqueue_douyin_batch,
            douyin::douyin_qr_start,
            douyin::douyin_qr_poll,
            douyin::douyin_qr_cancel,
            douyin::douyin_login_status,
            douyin::douyin_login_finish,
            douyin::douyin_logout,
            commands::snapshot,
            commands::telegram_avatar,
            commands::list_platforms,
            commands::probe_telegram,
            commands::remove_download,
            commands::open_download,
            commands::playback_path,
            commands::download_path_exists,
            commands::open_external_link,
            commands::file_thumbnail,
            commands::restore_session,
            commands::save_settings,
            commands::send_code,
            commands::sign_in,
            commands::check_password,
            qr_login::start_qr_login,
            qr_login::poll_qr_login,
            qr_login::cancel_qr_login,
            commands::logout,
            commands::preview_link,
            commands::enqueue_download,
            commands::control_download,
            commands::reveal_download,
            commands::open_download_folder
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start FrameFetch");
}

