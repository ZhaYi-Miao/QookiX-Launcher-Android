use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    #[serde(alias = "mcVersion")]
    pub mc_version: String,
    pub loader: Loader,
    #[serde(alias = "loaderVersion")]
    pub loader_version: Option<String>,
    pub created: i64,
    #[serde(alias = "lastPlayed")]
    pub last_played: Option<i64>,
    #[serde(alias = "totalPlayTime")]
    pub total_play_time: i64,
    #[serde(alias = "gameDir")]
    pub game_dir: String,
    /// 序列化为前端字段名 `java_path`；读取时兼容旧文件里的 `java_dir`。
    #[serde(rename = "java_path", alias = "java_dir", alias = "javaDir")]
    pub java_dir: String,
    /// 序列化为前端字段名 `jvm_args`；读取时兼容旧文件里的 `java_args`。
    #[serde(rename = "jvm_args", alias = "java_args", alias = "javaArgs")]
    pub java_args: Option<String>,
    #[serde(alias = "gameArgs")]
    pub game_args: Option<String>,
    pub resolution: Option<(i32, i32)>,
    #[serde(alias = "maxMemoryMb")]
    pub max_memory_mb: Option<i32>,
    #[serde(alias = "memoryMode")]
    pub memory_mode: Option<String>,
    #[serde(alias = "accountId")]
    pub account_id: Option<String>,
    pub icon: Option<String>,
    pub mods: Vec<InstalledContent>,
    #[serde(alias = "resourcePacks")]
    pub resource_packs: Vec<InstalledContent>,
    pub shaders: Vec<InstalledContent>,
    pub group: Option<String>,
    #[serde(alias = "isSymlink")]
    pub is_symlink: Option<bool>,
    #[serde(alias = "sourcePath")]
    pub source_path: Option<String>,
    /// 游戏本体是否已下载（前端据此显示「未安装」横幅 / 安装按钮）。
    /// 旧实例文件没有该字段时按 false 处理，等待用户点一次「安装游戏」。
    #[serde(default)]
    pub installed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

/// 已安装内容（mod / 资源包 / 光影）的记录，字段与前端 `InstalledContent` 对齐。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstalledContent {
    pub filename: String,
    /// "modrinth" | "curseforge" | "manual"
    pub source: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    /// Mod 内部 id（fabric 的 sodium / forge 的 modId）
    #[serde(default)]
    pub mod_id: Option<String>,
    #[serde(default)]
    pub authors: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
    pub installed_at: i64,
    pub size: i64,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub enabled: bool,
}

impl InstalledContent {
    /// 构造一个"手动导入"的空记录，后续可被识别/填充。
    pub fn manual(filename: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64;
        InstalledContent {
            filename: filename.into(),
            source: "manual".to_string(),
            project_id: None,
            slug: None,
            version_id: None,
            name: None,
            version: None,
            mod_id: None,
            authors: None,
            description: None,
            installed_at: now,
            size: 0,
            icon: None,
            enabled: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Account {
    pub uuid: String,
    pub username: String,
    pub created: i64,
    pub account_type: AccountType,
    pub msa_refresh_token: Option<String>,
    pub xuid: Option<String>,
    pub expires_at: Option<i64>,
    pub skin_face_base64: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AccountType {
    Offline,
    Microsoft,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountType::Offline => "offline",
            AccountType::Microsoft => "microsoft",
        }
    }

    /// 兼容旧数据：`Offline` / `offline` / `microsoft` / `Microsoft` 均可解析。
    fn parse(raw: &str) -> AccountType {
        if raw.eq_ignore_ascii_case("microsoft") {
            AccountType::Microsoft
        } else {
            AccountType::Offline
        }
    }
}

/// 线上格式（与前端 `Account` 判别联合一致）：
/// `{"type":"offline"|"microsoft","uuid":..,"username":..,"created":..,"msa_expires_at":..}`
/// 同时兼容旧文件里的 `account_type: "Offline"` 写法。
impl Serialize for Account {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", self.account_type.as_str())?;
        map.serialize_entry("uuid", &self.uuid)?;
        map.serialize_entry("username", &self.username)?;
        map.serialize_entry("created", &self.created)?;
        map.serialize_entry("msa_refresh_token", &self.msa_refresh_token)?;
        map.serialize_entry("xuid", &self.xuid)?;
        map.serialize_entry("expires_at", &self.expires_at)?;
        map.serialize_entry("skin_face_base64", &self.skin_face_base64)?;
        if let Some(exp) = self.expires_at {
            map.serialize_entry("msa_expires_at", &exp)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Account {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            uuid: String,
            #[serde(default)]
            username: String,
            #[serde(default)]
            created: i64,
            #[serde(default, rename = "type")]
            kind: Option<String>,
            #[serde(default)]
            account_type: Option<String>,
            #[serde(default)]
            msa_refresh_token: Option<String>,
            #[serde(default)]
            xuid: Option<String>,
            #[serde(default)]
            expires_at: Option<i64>,
            #[serde(default)]
            msa_expires_at: Option<i64>,
            #[serde(default)]
            skin_face_base64: Option<String>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let account_type = AccountType::parse(
            raw.kind
                .as_deref()
                .or(raw.account_type.as_deref())
                .unwrap_or("offline"),
        );
        Ok(Account {
            uuid: raw.uuid,
            username: raw.username,
            created: raw.created,
            account_type,
            msa_refresh_token: raw.msa_refresh_token,
            xuid: raw.xuid,
            expires_at: raw.expires_at.or(raw.msa_expires_at),
            skin_face_base64: raw.skin_face_base64,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Runtime {
    pub name: String,
    pub version_string: String,
    pub java_version: i32,
    pub arch: String,
    pub path: String,
    pub is_internal: bool,
    #[serde(default)]
    pub is_ported: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionList {
    pub latest: std::collections::HashMap<String, String>,
    pub versions: Vec<VersionInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionInfo {
    pub id: String,
    pub r#type: String,
    pub url: String,
    pub sha1: Option<String>,
    pub release_time: String,
    pub inherits_from: Option<String>,
    pub java_version: Option<JavaVersionInfo>,
    pub libraries: Vec<DependentLibrary>,
    pub downloads: std::collections::HashMap<String, ClientDownload>,
    pub asset_index: Option<AssetIndex>,
    pub main_class: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JavaVersionInfo {
    pub component: String,
    pub major_version: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DependentLibrary {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<LibraryRule>>,
    pub natives: Option<std::collections::HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LibraryDownloads {
    pub artifact: Option<LibraryArtifact>,
    pub classifiers: Option<std::collections::HashMap<String, LibraryArtifact>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LibraryArtifact {
    pub path: String,
    pub sha1: String,
    pub size: i32,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LibraryRule {
    pub action: String,
    pub os: Option<OsRule>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OsRule {
    pub name: String,
    pub version: Option<String>,
    pub arch: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientDownload {
    pub path: String,
    pub sha1: String,
    pub size: i32,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: i32,
    pub total_size: i32,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub data_dir: String,
    pub java_path: Option<String>,
    pub max_memory_mb: i32,
    pub min_memory_mb: i32,
    pub memory_mode: String,
    pub jvm_args: String,
    pub game_args: String,
    pub download_threads: i32,
    pub download_chunk_threads: i32,
    pub curseforge_api_key: Option<String>,
    pub theme: String,
    pub theme_color: String,
    pub close_behavior: String,
    pub auto_launch: bool,
    pub keep_open: bool,
    pub ms_client_id: String,
    pub selected_account: Option<String>,
    pub proxy_mode: String,
    pub proxy: Option<String>,
    pub mirror: String,
    pub mirror_custom: Option<String>,
    pub background_image: Option<String>,
    pub background_blur: i32,
    pub background_dim: i32,
    pub glass_blur: i32,
    /// 启动器界面缩放（百分比，100 = 原始大小）。
    /// 不同机型 CSS 视口差别很大（实测 853×384 与 792×360），
    /// 与其为每台机器写死尺寸，不如让用户自己按舒服的密度调。
    /// `serde(default)`：旧 settings.json 没有这个字段也能正常读。
    #[serde(default = "default_ui_scale")]
    pub ui_scale: i32,
    pub show_home_hero: bool,
    pub show_sidebar_collapse_btn: bool,
    pub show_news: bool,
    pub dismissed_update_version: Option<String>,
    pub auto_update: bool,
    pub update_source: String,
    /// 启动器界面方向："system"（跟随系统）| "portrait"（锁定竖屏）| "landscape"（锁定横屏）。
    /// 启动器默认锁定横屏：竖屏下桌面式侧边栏会把内容挤到只剩一条缝。
    #[serde(default = "default_orientation")]
    pub orientation: String,
    /// 手机导航栏（首页/实例/内容/…/下载/账号）的摆放位置：
    /// "bottom"（默认，屏幕底部横条）| "left"（屏幕左侧竖栏）。
    /// 横屏手机可用高度只有 384px 左右，底栏会吃掉约 70px；
    /// 挪到左侧能把这段高度还给内容区，而横向空间本来就用不完。
    #[serde(default = "default_nav_position")]
    pub nav_position: String,
    /// 左侧导航栏相对屏幕左缘的避让宽度（px，0 = 紧贴边缘）。
    /// 每台机器的挖孔/刘海位置与宽度都不一样，自动读取的安全区不一定符合
    /// 用户审美（有人宁可让图标被挡住一点也要贴边），所以交给用户拖滑块自己对齐。
    #[serde(default = "default_nav_offset")]
    pub nav_offset: i32,
    /// 左侧导航栏里「挖孔空档」的起始位置（px，相对栏顶。0 = 不启用）。
    /// 栏体贴边后图标分两段：挖孔区上下各一段，中空的一块给摄像头让位。
    /// 位置由用户在栏右侧的把手上**竖向拖动**决定。
    #[serde(default = "default_nav_gap_top")]
    pub nav_gap_top: i32,
    /// 挖孔空档的高度（px，0 = 不挖）。
    #[serde(default = "default_nav_gap_h")]
    pub nav_gap_h: i32,
    /// 挖孔 / 刘海的类型（用户按**竖屏**方向描述，横竖屏换算由前端做）：
    ///   "auto"    默认：完全跟随系统 env() 安全区（大多数机型够用）；
    ///   "none"    强制 0（系统误报时用）；
    ///   "center"  中置挖孔；"topleft" 左上角打孔；"topright" 右上角打孔；
    ///   "notch"   刘海 —— 长短不一，避让宽度用 `nav_offset`（用户自定）。
    /// 打孔的尺寸彼此都差不多，统一让开 44px（约一个图标的位置）即可，
    /// 不必让用户抠像素；只有刘海需要自定义宽度。
    #[serde(default = "default_nav_cutout")]
    pub nav_cutout: String,
    /// 触控目标大小档位：
    ///   "compact"（默认）保持项目原有的紧凑尺寸，不做任何干预；
    ///   "standard" 交互元素最小 40px；"large" 48px。
    /// 手机上手点不准时让用户自己放大 —— 规则见前端 `styles.css` 的
    /// `:root[data-touch=...]` 一段，值由 App.vue 写到 <html> 上。
    #[serde(default = "default_touch_target")]
    pub touch_target: String,
}

fn default_ui_scale() -> i32 {
    100
}

fn default_orientation() -> String {
    "landscape".to_string()
}

fn default_nav_position() -> String {
    "bottom".to_string()
}

fn default_nav_offset() -> i32 {
    0
}

fn default_nav_cutout() -> String {
    "auto".to_string()
}

fn default_touch_target() -> String {
    "compact".to_string()
}

fn default_nav_gap_top() -> i32 {
    0
}

fn default_nav_gap_h() -> i32 {
    0
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DownloadProgress {
    pub task_id: String,
    pub progress: f32,
    pub message: String,
    pub total_bytes: i64,
    pub downloaded_bytes: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameStatus {
    pub is_running: bool,
    pub instance_id: Option<String>,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrashReport {
    pub timestamp: i64,
    pub instance_id: String,
    pub error_message: String,
    pub stack_trace: String,
}

// ---------------------------------------------------------------------------
// 固定项（首页 / 侧边栏）
// ---------------------------------------------------------------------------

/// 固定位置：首页快捷卡片 / 侧边栏图标
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PinItem {
    /// 唯一标识：`instanceId:type:key:target`
    pub id: String,
    /// "server" | "world" | "instance"
    #[serde(rename = "type")]
    pub pin_type: String,
    /// "home" | "sidebar"
    pub target: String,
    #[serde(default)]
    pub instance_id: String,
    #[serde(default)]
    pub instance_name: String,
    #[serde(default)]
    pub instance_icon: Option<String>,
    #[serde(default)]
    pub mc_version: String,
    #[serde(default)]
    pub loader: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

// ---------------------------------------------------------------------------
// 实例分组
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstanceGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub created: u64,
}

// ---------------------------------------------------------------------------
// 存储统计
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StorageCategory {
    pub key: String,
    pub label: String,
    pub size: u64,
    pub files: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstanceStorage {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub files: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StorageStats {
    pub categories: Vec<StorageCategory>,
    pub instances: Vec<InstanceStorage>,
    pub servers: Vec<InstanceStorage>,
    pub total: u64,
    pub instance_count: u64,
    pub server_count: u64,
    pub updated_at: u64,
    pub cached: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheClearResult {
    pub freed: u64,
}

// ---------------------------------------------------------------------------
// 世界存档备份
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupInfo {
    pub filename: String,
    pub size: u64,
    pub modified: u64,
}

// ---------------------------------------------------------------------------
// 游玩时长统计
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaytimeStats {
    pub total_seconds: i64,
    pub by_instance: Vec<PlaytimeByInstance>,
    pub by_day: Vec<PlaytimeByDay>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaytimeByInstance {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub seconds: i64,
    pub last_played: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlaytimeByDay {
    pub day: i64,
    pub seconds: i64,
}

// ---------------------------------------------------------------------------
// 多人游戏服务器
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerEntry {
    pub name: String,
    pub address: String,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerStatus {
    pub online: bool,
    pub address: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub players_online: Option<i64>,
    pub players_max: Option<i64>,
    pub motd: Option<String>,
    pub favicon: Option<String>,
    pub latency_ms: Option<i64>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// 崩溃日志
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrashLogEntry {
    pub filename: String,
    pub modified: u64,
    pub size: u64,
    pub kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrashCause {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub reason: String,
    pub advice: String,
    pub evidence: String,
    pub confidence: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrashDetail {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrashDiagnosis {
    pub severity: String,
    pub title: String,
    pub reason: String,
    pub advice: String,
    pub excerpt: String,
    pub exit_code: Option<i32>,
    pub crash_report: Option<String>,
    pub affected_mods: Vec<String>,
    pub causes: Vec<CrashCause>,
    pub stacktrace: Vec<String>,
    pub details: Vec<CrashDetail>,
    pub confidence: u32,
}
