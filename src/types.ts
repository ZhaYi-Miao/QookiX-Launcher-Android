import type { Component } from "vue";

export type Loader = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";

/** 右键菜单项；`sep` 为真时渲染为分隔线，其余字段忽略 */
export interface ContextMenuItem {
  key: string;
  label?: string;
  icon?: Component;
  /** 右侧显示的快捷键提示，仅作展示 */
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  sep?: boolean;
  action?: () => void;
}

export interface Settings {
  data_dir: string;
  java_path: string | null;
  max_memory_mb: number;
  min_memory_mb: number;
  memory_mode: string;
  jvm_args: string;
  game_args: string;
  download_threads: number;
  download_chunk_threads: number;
  curseforge_api_key: string | null;
  /** 内容描述/正文翻译服务：default（自建）| custom（OpenAI 兼容）| baidu_web（开浏览器） */
  translate_provider: string;
  translate_api_base: string;
  translate_api_key: string | null;
  translate_api_model: string;
  /** 打开资源详情时自动加载正文译文 */
  body_translate_auto: boolean;
  theme: string;
  theme_color: string;
  close_behavior: string;
  auto_launch: boolean;
  keep_open: boolean;
  ms_client_id: string;
  selected_account: string | null;
  /** 下载代理模式："system"（系统代理）| "direct"（直连）| "custom"（自定义） */
  proxy_mode: string;
  /** 自定义代理地址（proxy_mode === "custom" 时生效） */
  proxy: string | null;
  /** 下载镜像源 id："official" | "bmclapi" | "custom" */
  mirror: string;
  /** 自定义镜像根地址（mirror === "custom" 时生效） */
  mirror_custom: string;
  background_image: string | null;
  background_blur: number;
  background_dim: number;
  glass_blur: number;
  /** 启动器界面缩放（百分比，100 = 原始大小，60–200） */
  ui_scale: number;
  show_sidebar_collapse_btn: boolean;
  /** 新闻页面与侧边栏新闻入口是否显示（默认 true） */
  show_news: boolean;
  /** 首页「正在下载」卡片（有任务时显示，这里是总开关） */
  show_home_downloads: boolean;
  /** 首页「最近游玩」列表 */
  show_home_recent: boolean;
  /** 首页「游玩统计」卡片（累计时长 + 近 30 天曲线） */
  show_home_stats: boolean;
  dismissed_update_version: string | null;
  auto_update: boolean;
  /** 启动器界面方向："landscape"（锁定横屏，默认）| "portrait"（锁定竖屏）| "system"（跟随系统） */
  orientation: string;
  /** 应用自更新源："bucket"（对象存储，默认） | "github"（GitHub Releases 官方源） */
  update_source: "bucket" | "github";
}

/** 下载镜像源预设 */
export interface MirrorPreset {
  id: string;
  label: string;
  /** 镜像站根地址，官方源为空串 */
  base: string;
  desc: string;
}

export interface MirrorTestResult {
  ok: boolean;
  ms: number;
  url: string;
}

export interface JavaInfo {
  path: string;
  version: string;
  major: number;
  vendor: string;
  arch: string;
}

export interface StorageCategory {
  key: string;
  label: string;
  size: number;
  files: number;
}

export interface InstanceStorage {
  id: string;
  name: string;
  size: number;
  files: number;
}

export interface StorageStats {
  categories: StorageCategory[];
  instances: InstanceStorage[];
  servers: InstanceStorage[];
  total: number;
  instance_count: number;
  server_count: number;
  updated_at: number;
  cached: boolean;
}

export interface CacheClearResult {
  freed: number;
}

/** 「游戏目录」的候选项（Android 侧枚举各卷得到）。 */
export interface GameDirOption {
  kind: "internal" | "external";
  label: string;
  /** 游戏数据根；`instances/` 会建在其下 */
  path: string;
  /** 可用空间（字节） */
  free: number;
  total: number;
  /** 是否可插拔（SD 卡） */
  removable: boolean;
}

/** 「游戏目录」当前状态。 */
export interface GameDirState {
  /** 内部私有数据目录（默认值） */
  defaultRoot: string;
  /** 自定义根；null = 用内部目录 */
  customRoot: string | null;
  /** 实际生效的数据根 */
  root: string;
  /** 实际生效的实例目录根 */
  instancesDir: string;
  /** Android 11+ 的「所有文件访问」是否已授权 */
  allFilesAccess: boolean;
}

export interface Instance {
  id: string;
  name: string;
  mc_version: string;
  loader: Loader;
  loader_version: string | null;
  created: number;
  last_played: number | null;
  /** 累计游玩时长（秒），由后端在游戏进程退出时累加 */
  total_play_time: number;
  installed: boolean;
  icon: string | null;
  /** 实例别名（qookix://launch/<alias> 协议启动用，全局唯一） */
  alias?: string | null;
  max_memory_mb: number | null;
  memory_mode: string | null;
  jvm_args: string | null;
  game_args: string | null;
  java_path: string | null;
  account_id: string | null;
  resolution: [number, number] | null;
  mods: InstalledContent[];
  resource_packs: InstalledContent[];
  shaders: InstalledContent[];
  is_symlink?: boolean;
  source_path?: string | null;
  /** 所属分组 id，null / undefined 表示未分组 */
  group?: string | null;
  /**
   * 本实例使用的渲染器：
   * - `"auto"` / `null`（缺省）：按 MC 版本自动挑（26.x → MobileGlues，其余 → GL4ES）
   * - `"global"`：跟随「设置 → 游戏内 → 渲染器」
   * - `"opengles2"` | `"mobileglues"` | `"vulkan_zink"`：显式指定
   */
  renderer?: string | null;
  /**
   * 启动前是否检查游戏文件完整性、缺了自动补全再启动。
   * `undefined` / `null`（旧实例）与 `true` 都视为开启，只有 `false` 关闭。
   */
  check_files_on_launch?: boolean | null;
}

/**
 * 「补全游戏文件」诊断结果（后端 `check_instance_files`，只读、不下载）。
 *
 * 用于回答了「实例到底缺什么」这个问题：创建实例时的后台安装一旦失败
 * （网络断流 / 进程被杀 / 磁盘满），实例会停在「只有 instance.json」的状态，
 * 界面看不出原因、也没有补救入口。
 */
export interface InstanceFileReport {
  /** 5 个关键项里完整了几个（0-5） */
  ok: number;
  total: number;
  missing: InstanceMissingItem[];
  /** 缺文件时给用户的一句人话建议 */
  advice: string;
  /** 关键项全齐，可以直接启动 */
  can_launch: boolean;
}

export interface InstanceMissingItem {
  /** version / libraries / assets / loader / runtime */
  kind: string;
  detail: string;
  /** false 表示「补全文件」也修不好（如加载器版本号为空，得换游戏版本） */
  fixable: boolean;
}

/**
 * 渲染器健康检查结论（后端 `check_renderer_health`）。
 * 只在「日志里有渲染器失败证据」**且**「当前渲染器不是该版本推荐的」时才有值。
 */
export interface RendererIssue {
  instance_id: string;
  /** 本次实际使用的渲染器键 */
  used: string;
  used_name: string;
  /** 按 MC 版本推荐的渲染器键 */
  recommended: string;
  recommended_name: string;
  /** 一句人话：日志里发现了什么 */
  reason: string;
  /** 日志证据（最多几条） */
  evidence: string[];
  mc_version: string;
}

/** 实例分组（持久化在 instance_groups.json） */
export interface InstanceGroup {
  id: string;
  name: string;
  color: string | null;
  created: number;
}

export type Account =
  | { type: "offline"; uuid: string; username: string; created: number }
  | {
      type: "microsoft";
      uuid: string;
      username: string;
      created: number;
      msa_expires_at: number;
    };

export interface ProjectHit {
  provider: "modrinth" | "curseforge";
  id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  downloads: number;
  follows: number;
  icon_url: string;
  project_type: string;
  categories: string[];
  latest_version: string;
  game_versions: string[];
  updated: string;
  featured_image: string;
}

export interface ProjectFile {
  url: string;
  filename: string;
  size: number;
  primary: boolean;
  hashes?: Record<string, string>;
}

export interface ProjectVersion {
  id: string;
  name: string;
  version_number: string;
  version_type?: string;
  date_published: string;
  game_versions: string[];
  loaders: string[];
  files: ProjectFile[];
  dependencies?: unknown[];
  download_url?: string;
  filename?: string;
  size?: number;
  release_type?: number;
}

export interface ProjectDependency {
  projectId: string;
  title: string;
  slug: string;
  dependencyType: "required" | "optional" | "incompatible" | "embedded" | string;
}

export interface ContentItem {
  record: InstalledContent;
  exists: boolean;
}

export interface InstalledContent {
  filename: string;
  source: string;
  project_id: string | null;
  /** Modrinth/CurseForge 项目 slug，用于内容中心检索与中文名映射 */
  slug?: string | null;
  /** 后端按 WikiEntries 映射出的中文名（未命中为 null） */
  cn_name?: string | null;
  version_id: string | null;
  name: string | null;
  version: string | null;
  /** Mod 内部 id（fabric 的 sodium / forge 的 modId） */
  mod_id?: string | null;
  /** 作者列表 */
  authors?: string[] | null;
  /** Mod 描述 */
  description?: string | null;
  installed_at: number;
  size: number;
  icon: string | null;
  enabled: boolean;
}

/** 世界存档备份快照元信息 */
export interface WorldBackupInfo {
  filename: string;
  size: number;
  /** unix 秒 */
  modified: number;
}

/** 游玩时长统计（后端聚合） */
export interface PlaytimeStats {
  totalSeconds: number;
  byInstance: {
    id: string;
    name: string;
    icon: string | null;
    seconds: number;
    lastPlayed: number | null;
  }[];
  /** 最近 30 天，day 为 (unix+8h)/86400 的天数索引 */
  byDay: { day: number; seconds: number }[];
}

export interface UpdateInfo {
  filename: string;
  projectId: string;
  currentVersion: string | null;
  latestVersion: string;
  latestVersionId: string;
  projectTitle: string | null;
  kind: string;
  provider: string;
}

export interface InstallProgressEvent {
  taskId: number;
  stage: string;
  message: string;
  done: number;
  total: number;
  instanceId?: string;
  instanceName?: string;
  source?: string;
  ok?: boolean;
}

export interface DownloadProgressEvent {
  taskId: number;
  phase: string;
  done: number;
  total: number;
  current: string;
  ok: boolean;
  bytesDone?: number;
  bytesTotal?: number;
  ts?: number;
  activeFiles?: ActiveFile[];
}

/** 正在下载的文件，由后端 `download.rs` 每 400ms 上报一次实时字节数 */
export interface ActiveFile {
  name: string;
  bytesDone: number;
  bytesTotal: number;
}

/**
 * 插件（组件 / 渲染器 / 驱动）的展示信息。
 *
 * 后端把「远程清单」与「本机安装状态」合并后给出：`version` 是清单里的可用版本，
 * `installed_version` 是本机装的版本，两者不等即 `update_available`。
 */
export interface PluginInfo {
  id: string;
  name: string;
  summary: string;
  kind: string;
  version: string | null;
  installed_version: string | null;
  enabled: boolean;
  update_available: boolean;
  abi_supported: boolean;
  device_abi: string;
  size: number | null;
  installed_size: number;
  source: string | null;
  error: string | null;
  /** 渲染器插件提供的渲染后端键（`opengles2` / `mobileglues` / `vulkan_zink`）。 */
  renderers: string[];
  /** 属于「首启要装」的类别（渲染器 / 驱动 / 组件） */
  recommended: boolean;
}

/** 首启准备状态：还缺哪些渲染器/驱动/组件、一共要下多少。 */
export interface PluginSetupStatus {
  needed: boolean;
  /** 用户点过「稍后」 */
  dismissed: boolean;
  missing: PluginInfo[];
  total_size: number;
  /** 清单拿不到时的说明（首启离线） */
  error: string | null;
}

/** 一次「补齐首启插件」的结果。 */
export interface PluginSetupResult {
  plugins: PluginInfo[];
  installed: number;
  /** 形如「MobileGlues：下载失败：…」 */
  failed: string[];
}

/** 控制布局里的一个按钮（与 Rust `controls::ControlButtonInfo` 对应）。 */
/** 一份触控控制布局（`<files>/controlmap/<名字>.json`，全局共享，不区分实例） */
export interface ControlLayoutInfo {
  name: string;
  buttons: number;
  joysticks: number;
  drawers: number;
  current: boolean;
  size: number;
  /** 最后修改时间（unix 秒，0 = 取不到） */
  modified: number;
}

export interface ControlButtonInfo {
  index: number;
  name: string;
  /** 按住这个键时拖动是否也能转视角 */
  passThru: boolean;
}

/** 启动器自更新：GitHub Release 检查结果（与 Rust `updater::UpdateInfo` 对应）。 */
export interface AppUpdateInfo {
  available: boolean;
  version: string | null;
  currentVersion: string;
  notes: string | null;
  size: number | null;
  /** 这个版本已经下载好的安装包路径 */
  downloadedPath: string | null;
}

/** 插件安装进度（`plugin://progress`）。phase: download / verify / extract / done */
export interface PluginProgressEvent {
  id: string;
  phase: string;
  done: number;
  total: number;
  message: string;
}

export interface NewsItem {
  title: string;
  description?: string;
  content?: string;
  author?: string;
  time: number;
  image?: string;
  image_alt?: string;
  url?: string;
  important?: boolean;
}

export interface LaunchLogEvent {
  instanceId: string;
  stream: "out" | "err";
  line: string;
}

export interface LaunchStateEvent {
  instanceId: string;
  state: "running" | "exited";
  pid: number;
  code: number | null;
}

// 实例的多人游戏服务器条目（来自游戏内 servers.json / servers.dat）
export interface ServerEntry {
  name: string;
  address: string;
  icon: string | null; // 原始 base64（无 data: 前缀）
}

/** 实例文件管理器中的一条目录项 */
export interface FsEntry {
  name: string;
  /** 相对实例根目录的路径（用 / 分隔） */
  rel: string;
  size: number;
  modified: number;
  is_dir: boolean;
  /** 小写扩展名，目录为空字符串 */
  ext: string;
}

// 经 Server List Ping 获取的实时状态
export interface ServerStatus {
  online: boolean;
  address: string;
  name: string | null;
  version: string | null;
  players_online: number | null;
  players_max: number | null;
  motd: string | null;
  favicon: string | null; // 完整 data:image/png;base64,...
  latency_ms: number | null;
  error: string | null;
}

// 本地托管的游戏服务器核心类型
export type ServerCore =
  | "vanilla"
  | "paper"
  | "spigot"
  | "purpur"
  | "forge"
  | "fabric";

// 陶瓦联机（Terracotta）
export interface TerracottaInfo {
  found: boolean;
  path: string | null;
  running: boolean;
  port: number | null;
  download_url: string;
  icon: string | null;
}

export interface TerracottaLaunch {
  port: number;
  ui_url: string;
  path: string;
}

// 陶瓦联机下载进度
export interface TerracottaDownloadProgress {
  downloaded: number;
  total: number;
  percent: number;
  extracting?: boolean;
  done?: boolean;
}

export type TerracottaRoomState =
  | "waiting"
  | "scanning"
  | "host-starting"
  | "host-ok"
  | "guest-connecting"
  | "guest-starting"
  | "guest-ok"
  | "exception";

// 用户在"多人游戏 → 服务器"中创建的本地服务器配置
export interface ServerConfig {
  id: string;
  name: string;
  core: ServerCore;
  mc_version: string;
  port: number;
  max_memory_mb: number;
  min_memory_mb: number;
  motd: string;
  eula: boolean;
  created: number;
  last_started: number | null;
  java_path: string | null;
  jvm_args: string | null;
  stop_command: string | null;
}

// 崩溃分析结果
// 主因字段（severity/title/reason/advice）取自置信度最高的一条，兼容旧展示逻辑；
// causes 里是全量命中原因，stacktrace/details 是报告结构化解析结果。
export type CrashSeverity =
  | "oom"
  | "jvm"
  | "lwjgl"
  | "java_ver"
  | "gl"
  | "mod"
  | "unknown";

export interface CrashCause {
  id: string;
  severity: CrashSeverity;
  title: string;
  reason: string;
  advice: string;
  /** 命中该原因的证据（崩溃报告原文片段） */
  evidence: string;
  /** 置信度 0-100 */
  confidence: number;
}

export interface CrashDetail {
  key: string;
  value: string;
}

export interface CrashDiagnosis {
  severity: CrashSeverity;
  title: string;
  reason: string;
  advice: string;
  excerpt: string;
  exit_code: number | null;
  crash_report: string | null;
  affected_mods: string[];
  /** 全部命中原因，按置信度降序 */
  causes: CrashCause[];
  /** 关键堆栈帧 */
  stacktrace: string[];
  /** 环境信息（Minecraft / Java / 内存 / 显卡 / 系统） */
  details: CrashDetail[];
  /** 主因置信度 0-100，0 表示未能定位 */
  confidence: number;
}
