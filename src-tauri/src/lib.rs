mod commands;
mod models;
mod settings;
mod instances;
mod version;
mod launch;
/// GL_VENDOR 品牌串覆盖（F3 里那行 `Display: … (厂商串)`）。
mod gl_brand;
mod download;
mod java;
mod accounts;
mod modpack;
mod servers;
mod crash;
mod storage;
mod natives;
mod jvm_launcher;
mod render_bridge;
mod input_bridge;
mod browse;
mod pins;
mod groups;
mod mirror;
mod mcping;
mod util;
mod skins;
mod world_backup;
mod playtime;
mod fsutil;
mod progress;

#[cfg(target_os = "android")]
mod jni_bridge;
mod android_bridge;
mod android_env;

use tauri::Builder;

#[cfg(not(test))]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // 保存 AppHandle：下载/安装/启动的进度事件都靠它广播给前端
        .setup(|app| {
            progress::set_app_handle(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Version commands
            commands::get_version_list,
            commands::get_version_manifest,
            commands::get_version_info,
            commands::install_version,
            
            // Instance commands
            commands::create_instance,
            commands::delete_instance,
            commands::list_instances,
            commands::get_instance,
            commands::update_instance,
            
            // Account commands
            commands::list_accounts,
            commands::add_account,
            commands::remove_account,
            commands::select_account,
            
            // Launch commands
            commands::launch_game,
            commands::kill_game,
            commands::get_game_status,
            
            // Download commands
            commands::download_file,
            commands::cancel_download,
            commands::get_download_progress,
            commands::cancel_install,
            commands::read_clipboard,
            commands::write_clipboard,
            
            // Settings commands
            commands::get_settings,
            commands::update_settings,
            
            // Modpack commands
            commands::import_modpack,
            commands::install_mod,
            commands::uninstall_mod,
            commands::get_installed_mods,

            // Browse commands
            commands::browse,
            commands::curseforge_categories,
            commands::project_info,
            commands::project_versions,
            commands::project_dependencies,
            commands::get_loader_versions,
            commands::install_game,
            commands::install_content,
            commands::list_content,
            commands::mc_wiki_url,
            commands::fetch_news,
            commands::estimate_download,
            commands::estimate_import,

            // Pins
            commands::get_pins,
            commands::set_pins,

            // Instance groups
            commands::list_instance_groups,
            commands::create_instance_group,
            commands::rename_instance_group,
            commands::delete_instance_group,
            commands::reorder_instance_groups,

            // Mirror
            commands::list_mirrors,
            commands::test_mirror,
            commands::test_proxy,

            // Memory & log
            commands::auto_detect_memory,
            commands::log_debug,
            commands::is_game_running,

            // Storage
            commands::get_storage_stats,
            commands::refresh_storage_stats,
            commands::clear_cache,

            // Crash analysis
            commands::list_crash_logs,
            commands::analyze_crash_log,
            commands::get_crash_report_content,

            // Skins
            commands::list_skins,
            commands::read_skin_data_url,
            commands::save_skin_from_data,
            commands::download_skin_from_url,
            commands::delete_skin,
            commands::fetch_image_data_url,
            commands::fetch_player_skin,
            commands::fetch_player_capes,
            commands::apply_skin_to_account,
            commands::apply_cape_to_account,
            commands::apply_skin_offline,
            commands::get_offline_skin,

            // Multiplayer servers
            commands::list_servers,
            commands::ping_mc_server,

            // World backup
            commands::list_world_backups,
            commands::create_world_backup,
            commands::restore_world_backup,
            commands::delete_world_backup,

            // Playtime
            commands::playtime_stats,
            commands::read_instance_log,
            commands::get_pojav_prefs,
            commands::set_pojav_prefs,

            // Microsoft login (device code)
            commands::login_ms_start,
            commands::login_ms_poll,

            // Content management
            commands::check_updates,
            commands::apply_update,
            commands::uninstall_content,
            commands::identify_content,
            commands::import_local_file,
            commands::toggle_content_enabled,
            commands::save_text_file,
            commands::extract_game_icons,

            // Instance file manager
            commands::list_instance_folders,
            commands::list_instance_files,
            commands::list_instance_dir,
            commands::read_instance_file,
            commands::write_instance_file,
            commands::create_instance_entry,
            commands::delete_instance_path,
            commands::rename_instance_path,

            // Image import
            commands::import_instance_image,
            commands::import_background_image,

            // Native (orientation / file picker)
            commands::set_orientation,
            commands::resolve_picked_path,
            commands::detect_system_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Qookix application");
}

#[cfg(test)]
pub fn run() {}
