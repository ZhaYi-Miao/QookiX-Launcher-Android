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
mod server_process;
mod server_files;
mod instance_files;
mod rcon;
mod crash;
mod renderer_health;
mod storage;
mod natives;
mod jvm_launcher;
mod terracotta;
mod render_bridge;
mod input_bridge;
mod browse;
mod translate;
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
/// 插件系统（v1 组件插件；渲染器/驱动沿用同一套机制）。
mod plugin;
/// 启动器自更新（查 GitHub Release → 下 APK → 交给系统安装器）。
mod updater;
/// 控制布局里「按键透传」的读取与修改。
mod controls;

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
    servers::list_hosted_servers,
    servers::get_hosted_server,
    servers::create_hosted_server,
    servers::update_hosted_server,
    servers::delete_hosted_server,
    servers::suggest_server_memory,
    instance_files::check_instance_files,
    instance_files::repair_instance_files,
    servers::install_hosted_server_core,
    server_files::list_hosted_server_dir,
    server_files::read_hosted_server_file,
    server_files::write_hosted_server_file,
    server_files::list_hosted_server_config_files,
    server_files::open_hosted_server_folder,
    servers::start_hosted_server,
    servers::is_hosted_server_running,
    servers::read_hosted_server_log,
    server_process::inspect_server_core_manifest,
    servers::start_hosted_server,
    servers::stop_hosted_server,
    servers::server_console_command,
    servers::hosted_server_runtime,
    servers::hosted_server_log,
    servers::hosted_server_address,
    servers::hosted_server_addresses,
    servers::hosted_server_core_installed,
    terracotta::terracotta_ping,
    terracotta::terracotta_request,
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

            // Plugin commands（组件/渲染器插件：下载、校验、启停）
            commands::get_plugins,
            commands::refresh_plugin_manifest,
            commands::install_plugin,
            commands::install_plugin_from_file,
            commands::uninstall_plugin,
            commands::set_plugin_enabled,
            commands::get_plugin_manifest_url,
            commands::set_plugin_manifest_url,
            // 首启准备：自动补齐渲染器/驱动/组件
            commands::get_plugin_setup_status,
            commands::install_recommended_plugins,
            commands::dismiss_plugin_setup,
            
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

            // 内容中心翻译
            commands::translate_mod_descriptions,
            commands::translate_project_body,
            commands::report_translation_stale,
            commands::report_translation_quality,
            commands::clear_translation_cache,
            commands::test_translate_api,

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
        commands::check_renderer_health,
        commands::check_renderer_health_latest,
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

            // 启动器自更新
            updater::check_for_update,
            updater::download_update,
            updater::install_update,

            // 控制布局的按键透传
            controls::get_control_buttons,
            controls::set_control_button_passthru,

            // 控制布局管理（列出 / 切换当前 / 复制 / 重命名 / 删除）
            controls::list_control_layouts,
            controls::set_current_control_layout,
            controls::duplicate_control_layout,
            controls::rename_control_layout,
            controls::delete_control_layout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Qookix application");
}

#[cfg(test)]
pub fn run() {}
