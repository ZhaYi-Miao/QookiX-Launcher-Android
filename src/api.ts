import { invoke as rawInvoke } from "@tauri-apps/api/core";
import { trackStart, trackEnd, trackError } from "./loadingBar";
import type { PinItem } from "./stores/pins";
import type {
  Account,
  CacheClearResult,
  ContentItem,
  CrashDiagnosis,
  FsEntry,
  Instance,
  InstanceGroup,
  MirrorPreset,
  MirrorTestResult,
  NewsItem,
  ProjectDependency,
  ProjectHit,
  ProjectVersion,
  ServerConfig,
  ServerEntry,
  ServerStatus,
  Settings,
  StorageStats,
  TerracottaInfo,
  TerracottaLaunch,
  UpdateInfo,
  PlaytimeStats,
  WorldBackupInfo,
} from "./types";

// ---------------------------------------------------------------------------
// Android 兼容层：命令名映射
// Windows 前端调用的命令名与 Android Rust 后端的命令名不同，
// 这里做一层翻译，让上层 Vue 代码无需任何改动。
// ---------------------------------------------------------------------------
const CMD_MAP: Record<string, string> = {
  // settings
  set_settings: "update_settings",
  // instances
  get_instance_info: "get_instance",
  update_instance_settings: "update_instance",
  // launch
  launch_instance: "launch_game",
  stop_game: "kill_game",
  // accounts
  login_offline: "add_account",
  logout_account: "remove_account",
};

// Android 后端未实现的命令列表 —— 调用时返回安全的默认值而不是崩溃
const UNIMPLEMENTED = new Set([
  "change_data_dir",
  "detect_java",
  "download_java",
  "recommend_java",
  "scan_minecraft_import",
  "import_minecraft_folder",
  "export_instance_pack",
  "import_instance_pack",
  "list_hosted_servers",
  "get_hosted_server",
  "create_hosted_server",
  "update_hosted_server",
  "delete_hosted_server",
  "install_hosted_server_core",
  "start_hosted_server",
  "stop_hosted_server",
  "is_hosted_server_running",
  "read_hosted_server_log",
  "open_hosted_server_folder",
  "reveal_hosted_server_path",
  "list_hosted_server_folders",
  "list_hosted_server_files",
  "list_hosted_server_dir",
  "read_hosted_server_file",
  "write_hosted_server_file",
  "list_hosted_server_config_files",
  "terracotta_detect",
  "terracotta_download",
  "terracotta_launch",
  "terracotta_stop",
  "terracotta_status",
  "terracotta_create_room",
  "terracotta_join_room",
  "terracotta_leave",
]);

// 未实现命令的默认返回值（**只有只读命令**才允许给安全默认值）
const UNIMPLEMENTED_DEFAULTS: Record<string, unknown> = {
  detect_java: { candidates: [], selected: null },
  recommend_java: null,
  download_java: null,
  scan_minecraft_import: undefined,
  import_minecraft_folder: [],
  // hosted servers
  get_hosted_server: null,
  create_hosted_server: null,
  update_hosted_server: null,
  delete_hosted_server: undefined,
  // 缺了这两项时 invoke 会 resolve undefined，store 把 undefined 赋给
  // `servers`，随后 `servers.length` 直接抛错 —— 多人游戏页与服务器详情页整页白屏。
  list_hosted_servers: [],
  is_hosted_server_running: false,
  install_hosted_server_core: null,
  start_hosted_server: undefined,
  stop_hosted_server: undefined,
  // 声明的是 string[]（ServerDetailView 会直接 .map()），给 "" 会在打开
  // 「服务器日志」页签时抛 TypeError。
  read_hosted_server_log: [],
  open_hosted_server_folder: undefined,
  reveal_hosted_server_path: undefined,
  list_hosted_server_folders: { folders: [] },
  list_hosted_server_files: { files: [] },
  list_hosted_server_dir: { entries: [] },
  read_hosted_server_file: "",
  write_hosted_server_file: undefined,
  list_hosted_server_config_files: [],
  // terracotta
  terracotta_detect: null,
  terracotta_download: null,
  terracotta_launch: null,
  terracotta_stop: undefined,
  terracotta_status: null,
  terracotta_create_room: null,
  terracotta_join_room: null,
  terracotta_leave: undefined,
};

/**
 * 未实现命令中属于「写操作」的那些。
 *
 * 这些**绝不能**静默返回默认值 —— 后端什么都没做，UI 却会弹「配置已保存 /
 * 服务器已启动 / 已删除」，用户完全看不出来（问题清单 P0-10 的假成功族）。
 * 读命令仍走上面的默认值，避免整页崩。
 */
const UNIMPLEMENTED_WRITE = new Set([
  "change_data_dir",
  "download_java",
  "export_instance_pack",
  "import_instance_pack",
  "import_minecraft_folder",
  "create_hosted_server",
  "update_hosted_server",
  "delete_hosted_server",
  "install_hosted_server_core",
  "start_hosted_server",
  "stop_hosted_server",
  "write_hosted_server_file",
  "terracotta_download",
  "terracotta_launch",
  "terracotta_stop",
  "terracotta_create_room",
  "terracotta_join_room",
  "terracotta_leave",
]);

/**
 * 包装 tauri invoke：
 * 1. 命令名映射（Windows → Android）
 * 2. 未实现命令的安全降级
 * 3. 顶部加载条（trackStart/trackEnd）
 */
function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  opts?: { silent?: boolean }
): Promise<T> {
  const silent = opts?.silent ?? false;

  // 映射命令名
  const mappedCmd = CMD_MAP[cmd] ?? cmd;

  // 未实现的命令（不触发加载条）
  if (UNIMPLEMENTED.has(mappedCmd)) {
    console.warn(`[api] 命令 "${cmd}" (→ "${mappedCmd}") 在 Android 上未实现`);
    if (UNIMPLEMENTED_WRITE.has(mappedCmd)) {
      // 写操作必须显式失败：否则 UI 会「假成功」（问题清单 P0-4 / P0-10）
      return Promise.reject(
        new Error(`该功能在当前平台（Android）暂未实现：${cmd}`)
      );
    }
    return Promise.resolve(UNIMPLEMENTED_DEFAULTS[mappedCmd] as T);
  }

  if (!silent) trackStart();
  return rawInvoke<T>(mappedCmd, args).then(
    (res) => {
      if (!silent) trackEnd();
      return res;
    },
    (err) => {
      if (!silent) {
        trackError();
        trackEnd();
      }
      console.error(`[api] 命令 "${cmd}" (→ "${mappedCmd}") 调用失败:`, err);
      throw err;
    },
  );
}

export const api = {
  // settings & java
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (patch: Record<string, unknown>) => invoke<Settings>("set_settings", { patch }),

  // pinned items (首页 / 侧边栏)
  getPins: () => invoke<PinItem[]>("get_pins"),
  setPins: (items: PinItem[]) => invoke<void>("set_pins", { items }),
  listMirrors: () => invoke<MirrorPreset[]>("list_mirrors", undefined, { silent: true }),
  testMirror: (base: string) => invoke<MirrorTestResult>("test_mirror", { base }, { silent: true }),
  testProxy: (proxyMode: string, proxy: string | null) =>
    invoke<MirrorTestResult>("test_proxy", { proxyMode, proxy }),
  autoDetectMemory: () =>
    invoke<{ total_mb: number; used_mb: number; available_mb: number; max_mb: number; min_mb: number }>("auto_detect_memory"),

  // versions
  getVersionManifest: () =>
    invoke<{
      versions: { id: string; type: string; releaseTime: string }[];
      latest: { release: string; snapshot: string };
    }>("get_version_manifest"),
  getLoaderVersions: (loader: string, mc_version: string) =>
    invoke<string[]>("get_loader_versions", { loader, mcVersion: mc_version }),

  // instances
  listInstances: () => invoke<Instance[]>("list_instances"),
  getInstance: (id: string) => invoke<Instance>("get_instance_info", { id }),
  createInstance: (name: string, mc_version: string, loader: string, loader_version: string | null) =>
    invoke<Instance>("create_instance", { name, mcVersion: mc_version, loader, loaderVersion: loader_version }),
  updateInstance: (patch: Record<string, unknown>) => invoke<Instance>("update_instance_settings", { patch }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),

  // instance groups
  listGroups: () => invoke<InstanceGroup[]>("list_instance_groups"),
  createGroup: (name: string, color?: string | null) =>
    invoke<InstanceGroup>("create_instance_group", { name, color: color ?? null }),
  renameGroup: (id: string, name: string, color?: string | null) =>
    invoke<InstanceGroup>("rename_instance_group", { id, name, color: color ?? null }),
  deleteGroup: (id: string) => invoke<void>("delete_instance_group", { id }),
  reorderGroups: (ids: string[]) => invoke<InstanceGroup[]>("reorder_instance_groups", { ids }),
  installGame: (instanceId: string) =>
    invoke<{ instance_id: string; total_bytes: number; file_count: number }>("install_game", { instanceId }, { silent: true }),

  /**
   * 取消进行中的安装/下载任务。
   *
   * `taskId` 就是「下载中心」里那张卡片的 id（后端 `TaskCtx.task_id`）。
   * 以前 `cancel_install` 被列在 UNIMPLEMENTED 里，前端点了不会有任何后端动作。
   */
  cancelInstall: (taskId: number) => invoke<void>("cancel_install", { taskId }),

  /**
   * 读取系统剪贴板（代码编辑器「粘贴」用）。
   *
   * 安卓 WebView 拿不到 `clipboard-read` 权限，`execCommand("paste")` 也必然失败，
   * 只能由原生 ClipboardManager 读。
   */
  readClipboard: () => invoke<string>("read_clipboard"),

  /** 写入系统剪贴板（同上，WebView 没有 clipboard-write 权限）。 */
  writeClipboard: (text: string) => invoke<void>("write_clipboard", { text }),
  launchInstance: (instanceId: string, world?: string, server?: string) =>
    invoke<{ pid: number; command: string[] }>(
      "launch_instance",
      {
        instanceId,
        world: world ?? null,
        server: server ?? null,
      },
      { silent: true }
    ),
  stopGame: () => invoke<void>("stop_game"),
  isGameRunning: () => invoke<boolean>("is_game_running"),
  listInstanceFolders: (instanceId: string) =>
    invoke<{ folders: { name: string; exists: boolean }[] }>("list_instance_folders", { instanceId }),
  listInstanceFiles: (instanceId: string, sub: string) =>
    invoke<{
      files: {
        name: string;
        size: number;
        modified: number;
        isDir: boolean;
        path: string;
        icon: string | null;
      }[];
    }>("list_instance_files", { instanceId, sub }),
  importModpack: (filePath: string) => invoke<Instance>("import_modpack", { filePath }),
  importInstanceImage: (sourcePath: string) => invoke<string>("import_instance_image", { sourcePath }),
  importBackgroundImage: (sourcePath: string) => invoke<string>("import_background_image", { sourcePath }),
  scanMinecraftImport: (source: string) => invoke<void>("scan_minecraft_import", { source }),
  importInstancePack: (filePath: string) =>
    invoke<{ instance: Instance; pendingDownloads: number }>("import_instance_pack", { filePath }),
  playtimeStats: () => invoke<PlaytimeStats>("playtime_stats"),
  // 读磁盘上的实例日志（内存里的实时日志会随崩溃一起消失，日志页需要这个回退）
  readInstanceLog: (instanceId: string) => invoke<string>("read_instance_log", { instanceId }),
  // 游戏内设置（Pojav 控制层）：存在安卓 SharedPreferences 里，经原生桥读写
  getPojavPrefs: () => invoke<Record<string, unknown>>("get_pojav_prefs"),
  setPojavPrefs: (patch: Record<string, unknown>) => invoke<void>("set_pojav_prefs", { patch }),
  logDebug: (msg: string) => invoke<void>("log_debug", { msg }, { silent: true }),
  listWorldBackups: (instanceId: string, world: string) =>
    invoke<WorldBackupInfo[]>("list_world_backups", { instanceId, world }),
  createWorldBackup: (instanceId: string, world: string) =>
    invoke<WorldBackupInfo>("create_world_backup", { instanceId, world }),
  restoreWorldBackup: (instanceId: string, world: string, filename: string) =>
    invoke<void>("restore_world_backup", { instanceId, world, filename }),
  deleteWorldBackup: (instanceId: string, world: string, filename: string) =>
    invoke<void>("delete_world_backup", { instanceId, world, filename }),
  estimateDownload: (mcVersion: string) =>
    invoke<{
      download_files: number;
      download_bytes: number;
      assets_known: boolean;
    }>("estimate_download", { mcVersion }),
  estimateImport: (source: string, rawIds: string[]) =>
    invoke<{
      import_files: number;
      import_bytes: number;
    }>("estimate_import", { source, rawIds }),
  importMinecraftFolder: (
    source: string,
    name: string,
    rawIds: string[],
    mcVersions: string[],
    loaders: string[],
    loaderVersions: (string | null)[],
    mode: "copy" | "symlink"
  ) =>
    invoke<{
      instance_id: string;
      total_bytes: number;
      file_count: number;
      symlink_fallback?: boolean;
    }[]>("import_minecraft_folder", {
      source,
      name,
      rawIds,
      mcVersions,
      loaders,
      loaderVersions,
      mode,
    }),

  // accounts
  listAccounts: () => invoke<Account[]>("list_accounts"),
  loginOffline: (username: string) => invoke<Account>("login_offline", { username }),
  loginMsStart: () => invoke<{ userCode: string; verificationUri: string; expiresIn: number }>("login_ms_start"),
  loginMsPoll: () => invoke<Account>("login_ms_poll"),
  logoutAccount: (uuid: string) => invoke<void>("logout_account", { uuid }),

  // browse & content
  browse: (
    provider: string,
    query: string,
    projectType: string,
    category: string,
    page: number,
    gameVersion?: string,
    loader?: string,
    sort?: string,
    pageSize?: number
  ) =>
    invoke<{ hits: ProjectHit[]; total: number; cf_error?: string | null; cf_count?: number }>("browse", {
      provider,
      query,
      projectType,
      category,
      page,
      gameVersion: gameVersion ?? "",
      loader: loader ?? "",
      sort: sort ?? "downloads",
      pageSize: pageSize ?? 20,
    }),
  projectVersions: (provider: string, projectId: string, mcVersion: string, loader: string) =>
    invoke<{ provider: string; versions: ProjectVersion[] }>("project_versions", {
      provider,
      projectId,
      mcVersion,
      loader,
    }),
  projectDependencies: (provider: string, projectId: string, versionId: string) =>
    invoke<ProjectDependency[]>(
      "project_dependencies",
      {
        provider,
        projectId,
        versionId,
      },
      { silent: true }
    ),
  mcWikiUrl: (name: string, slug?: string, provider?: string) =>
    invoke<string>("mc_wiki_url", { name, slug, provider }, { silent: true }),
  curseforgeCategories: (projectType: string) =>
    invoke<{ categories: { id: number; name: string }[] }>("curseforge_categories", { projectType }),
  projectInfo: (provider: string, projectId: string) =>
    invoke<ProjectHit>("project_info", { provider, projectId }),
  installContent: (
    instanceId: string,
    provider: string,
    projectId: string,
    versionId: string,
    kind: string
  ) =>
    invoke<{ ok: boolean; filename?: string; mods?: number }>(
      "install_content",
      {
        instanceId,
        provider,
        projectId,
        versionId,
        kind,
      },
      { silent: true }
    ),
  checkUpdates: (instanceId: string, kind: string) =>
    invoke<UpdateInfo[]>("check_updates", { instanceId, kind }),
  applyUpdate: (
    instanceId: string,
    kind: string,
    oldFilename: string,
    provider: string,
    projectId: string,
    newVersionId: string
  ) =>
    invoke<{ ok: boolean; filename?: string }>(
      "apply_update",
      {
        instanceId,
        kind,
        oldFilename,
        provider,
        projectId,
        newVersionId,
      },
      { silent: true }
    ),
  uninstallContent: (instanceId: string, kind: string, filename: string) =>
    invoke<void>("uninstall_content", { instanceId, kind, filename }),
  listContent: (instanceId: string, kind: string) =>
    invoke<{ items: ContentItem[]; onDisk: string[] }>("list_content", { instanceId, kind }),
  identifyContent: (instanceId: string, kind: string) =>
    invoke<void>("identify_content", { instanceId, kind }, { silent: true }),
  toggleContentEnabled: (instanceId: string, kind: string, filename: string, enabled: boolean) =>
    invoke<void>("toggle_content_enabled", { instanceId, kind, filename, enabled }),
  importLocalFile: (instanceId: string, kind: string, sourcePath: string) =>
    invoke<{ ok: boolean }>("import_local_file", { instanceId, kind, sourcePath }),
  saveTextFile: (path: string, content: string) => invoke<void>("save_text_file", { path, content }),

  // instance file manager
  listInstanceDir: (instanceId: string, rel: string) =>
    invoke<{ rel: string; entries: FsEntry[] }>("list_instance_dir", { instanceId, rel }, { silent: true }),
  readInstanceFile: (instanceId: string, rel: string) =>
    invoke<{ rel: string; content: string; size: number; modified: number }>(
      "read_instance_file",
      { instanceId, rel },
      { silent: true }
    ),
  writeInstanceFile: (instanceId: string, rel: string, content: string) =>
    invoke<{ rel: string; size: number; modified: number }>(
      "write_instance_file",
      { instanceId, rel, content },
      { silent: true }
    ),
  createInstanceEntry: (instanceId: string, rel: string, isDir: boolean) =>
    invoke<{ rel: string; is_dir: boolean }>("create_instance_entry", { instanceId, rel, isDir }),
  deleteInstancePath: (instanceId: string, rel: string) =>
    invoke<void>("delete_instance_path", { instanceId, rel }),
  renameInstancePath: (instanceId: string, rel: string, newName: string) =>
    invoke<{ rel: string; name: string }>("rename_instance_path", { instanceId, rel, newName }),
  extractGameIcons: (instanceId?: string) =>
    invoke<{ name: string; label: string; path: string }[]>("extract_game_icons", {
      instanceId: instanceId ?? null,
    }),

  // skins
  listSkins: () =>
    invoke<{ name: string; filename: string; path: string; size: number; modified: number }[]>("list_skins"),
  readSkinDataUrl: (filename: string) => invoke<string>("read_skin_data_url", { filename }),
  saveSkinFromData: (name: string, data: string) =>
    invoke<{ name: string; filename: string; path: string; size: number; modified: number }>("save_skin_from_data", {
      name,
      data,
    }),
  downloadSkinFromUrl: (name: string, url: string) =>
    invoke<{ name: string; filename: string; path: string; size: number; modified: number }>(
      "download_skin_from_url",
      { name, url },
    ),
  deleteSkin: (filename: string) => invoke<void>("delete_skin", { filename }),
  fetchPlayerSkin: (username: string) =>
    invoke<{ data_url: string; model: string; cape_data_url: string | null }>("fetch_player_skin", { username }),
  fetchImageDataURL: (url: string) => invoke<string>("fetch_image_data_url", { url }),
  fetchPlayerCapes: (accountUuid: string) =>
    invoke<{ id: string; name: string; data_url: string; active: boolean }[]>("fetch_player_capes", {
      accountUuid,
    }),
  applySkinToAccount: (accountUuid: string, skinData: string, variant: string) =>
    invoke<void>("apply_skin_to_account", { accountUuid, skinData, variant }),
  applyCapeToAccount: (accountUuid: string, capeId: string | null) =>
    invoke<void>("apply_cape_to_account", { accountUuid, capeId }),
  applySkinOffline: (skinData: string, variant: string, uuid: string) =>
    invoke<void>("apply_skin_offline", { skinData, variant, uuid }),
  getOfflineSkin: (uuid: string) =>
    invoke<{ src: string; variant: "slim" | "classic" | null } | null>("get_offline_skin", {
      uuid,
    }),

  // multiplayer servers
  listServers: (instanceId: string) =>
    invoke<{ servers: ServerEntry[] }>("list_servers", { instanceId }).then((r) => r.servers),
  pingServer: (address: string) => invoke<ServerStatus>("ping_mc_server", { address }),

  // hosted game servers
  listHostedServers: () => invoke<ServerConfig[]>("list_hosted_servers"),
  getHostedServer: (id: string) => invoke<ServerConfig>("get_hosted_server", { id }),
  createHostedServer: (name: string, core: string, mcVersion: string) =>
    invoke<ServerConfig>("create_hosted_server", { name, core, mcVersion }),
  updateHostedServer: (patch: Record<string, unknown>) =>
    invoke<ServerConfig>("update_hosted_server", { patch }),
  deleteHostedServer: (id: string) => invoke<void>("delete_hosted_server", { id }),
  installHostedServerCore: (id: string) =>
    invoke<void>("install_hosted_server_core", { id }, { silent: true }),
  startHostedServer: (id: string) =>
    invoke<{ pid: number }>("start_hosted_server", { id }, { silent: true }),
  stopHostedServer: (id: string) => invoke<void>("stop_hosted_server", { id }),
  isHostedServerRunning: (id: string) => invoke<boolean>("is_hosted_server_running", { id }),
  readHostedServerLog: (id: string) => invoke<string[]>("read_hosted_server_log", { id }),
  openHostedServerFolder: (id: string, sub?: string) =>
    invoke<void>("open_hosted_server_folder", { id, sub: sub ?? null }),
  listHostedServerFolders: (id: string) =>
    invoke<{ folders: { name: string; exists: boolean }[] }>("list_hosted_server_folders", { id }),
  listHostedServerFiles: (id: string, sub: string) =>
    invoke<{
      files: { name: string; path: string; size: number; modified: number; isDir: boolean; icon: string | null }[];
    }>("list_hosted_server_files", { id, sub }),
  listHostedServerDir: (id: string, rel: string) =>
    invoke<{ rel: string; entries: FsEntry[] }>("list_hosted_server_dir", { id, rel }),
  readHostedServerFile: (id: string, rel: string) =>
    invoke<{ rel: string; content: string; size: number; modified: number }>(
      "read_hosted_server_file",
      { id, rel },
      { silent: true }
    ),
  writeHostedServerFile: (id: string, rel: string, content: string) =>
    invoke<{ rel: string; size: number; modified: number }>(
      "write_hosted_server_file",
      { id, rel, content },
      { silent: true }
    ),
  listHostedServerConfigFiles: (id: string) =>
    invoke<{ name: string; rel: string; size: number; modified: number }[]>(
      "list_hosted_server_config_files",
      { id },
    ),

  // terracotta (陶瓦联机)
  terracottaDetect: () => invoke<TerracottaInfo>("terracotta_detect", undefined, { silent: true }),
  terracottaDownload: () => invoke<string>("terracotta_download", undefined, { silent: true }),
  terracottaLaunch: () => invoke<TerracottaLaunch>("terracotta_launch", undefined, { silent: true }),
  terracottaStop: () => invoke<void>("terracotta_stop", undefined, { silent: true }),
  terracottaStatus: () => invoke<Record<string, unknown>>("terracotta_status", undefined, { silent: true }),
  terracottaCreateRoom: (player?: string) =>
    invoke<Record<string, unknown>>("terracotta_create_room", { player: player ?? null }, { silent: true }),
  terracottaJoinRoom: (room: string, player?: string) =>
    invoke<Record<string, unknown>>("terracotta_join_room", { room, player: player ?? null }, { silent: true }),
  terracottaLeave: () => invoke<Record<string, unknown>>("terracotta_leave", undefined, { silent: true }),

  // storage
  getStorageStats: () => invoke<StorageStats>("get_storage_stats"),
  refreshStorageStats: () => invoke<StorageStats>("refresh_storage_stats"),
  clearCache: () => invoke<CacheClearResult>("clear_cache"),

  // crash analysis
  crashAnalysis: (instanceId: string) =>
    invoke<{ filename: string; modified: number; size: number; kind: string }[]>("list_crash_logs", { id: instanceId }),
  analyzeCrash: (instanceId: string, filename: string) =>
    invoke<CrashDiagnosis>("analyze_crash_log", { id: instanceId, filename }),
  getCrashReportContent: (instanceId: string, filename: string) =>
    invoke<string>("get_crash_report_content", { id: instanceId, filename }),

  // news
  fetchNews: () => invoke<NewsItem[]>("fetch_news"),

  // native (Android 屏幕方向 / 文件选择结果落地)
  /** 设置启动器界面方向：system | portrait | landscape */
  setOrientation: (mode: string) =>
    invoke<void>("set_orientation", { mode }, { silent: true }),
  /**
   * 安卓文件选择器返回的是 `content://` URI，后端只认普通路径。
   * 这里先把它落地到应用私有目录再返回真实路径；桌面端原样返回。
   */
  resolvePickedPath: (path: string) =>
    invoke<string>("resolve_picked_path", { path }, { silent: true }),
  /** 检测系统/VPN 下发的 HTTP 代理（无代理返回 null） */
  detectSystemProxy: () =>
    invoke<string | null>("detect_system_proxy", undefined, { silent: true }),
};
