<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onBeforeUnmount, onMounted, ref, watch, nextTick } from "vue";
import { fmtSize } from "../utils/format";
import { useRoute, useRouter } from "vue-router";
import { NInput, NInputNumber, NCheckbox, NSwitch, NSelect, useMessage } from "naive-ui";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { useServersStore } from "../stores/servers";
import ServerFileManager from "../components/ServerFileManager.vue";
import { CORE_COLORS, CORE_LABELS } from "../utils/cores";
import AppSheet from "../ui/AppSheet.vue";
import {
  IconBox,
  IconChevronLeft,
  IconCopy,
  IconDownload,
  IconFile,
  IconFolder,
  IconPlay,
  IconRefresh,
  IconStop,
  IconTrash,
} from "../components/icons";

const route = useRoute();
const router = useRouter();
const servers = useServersStore();
const message = useMessage();

const serverId = route.params.id as string;

const server = computed(() => servers.byId(serverId));
const running = computed(() => servers.isRunning(serverId));

// ---- tabs ----
const tab = ref<string>("logs");
const folders = ref<Record<string, boolean>>({});
const ALL_TABS = [
  { key: "logs", label: $t("common.logs") },
  { key: "settings", label: $t("router.settings") },
  { key: "config", label: $t("server-detail.config") },
  { key: "mods", label: $t("browse.mods"), folder: "mods" },
  { key: "plugins", label: $t("server-detail.plugins"), folder: "plugins" },
  { key: "files", label: $t("instance-detail.files") },
];
const tabs = computed(() =>
  ALL_TABS.filter((t) => !t.folder || folders.value[t.folder] || t.key === tab.value),
);
watch(tabs, (ts) => {
  if (!ts.some((t) => t.key === tab.value) && ts.length > 0) tab.value = ts[0].key;
});

// ---- 启动设置表单 ----
const form = ref({
  name: "",
  maxMem: 2048,
  minMem: 1024,
  eula: false,
  jvmArgs: "",
  stopCommand: "",
});
const saving = ref(false);

function syncForm() {
  const s = server.value;
  if (!s) return;
  form.value = {
    name: s.name,
    maxMem: s.max_memory_mb,
    minMem: s.min_memory_mb,
    eula: s.eula,
    jvmArgs: s.jvm_args ?? "",
    stopCommand: s.stop_command ?? "",
  };
}
watch(server, syncForm, { immediate: true });


async function saveConfig() {
  saving.value = true;
  try {
    await servers.update({
      id: serverId,
      name: form.value.name,
      max_memory_mb: form.value.maxMem,
      min_memory_mb: form.value.minMem,
      eula: form.value.eula,
      jvm_args: form.value.jvmArgs,
      stop_command: form.value.stopCommand,
    });
  } catch (e) {
    throw e;
  } finally {
    saving.value = false;
  }
}

// ---- 自动保存：修改任一设置字段即自动更新 ----
let autoSaveTimer: number | null = null;
let autoSaving = false;
function scheduleAutoSave() {
  if (autoSaveTimer !== null) window.clearTimeout(autoSaveTimer);
  autoSaveTimer = window.setTimeout(runAutoSave, 700);
}
async function runAutoSave() {
  autoSaveTimer = null;
  if (autoSaving || !server.value) return;
  autoSaving = true;
  try {
    await saveConfig();
  } catch (e) {
    message.error(String(e));
  } finally {
    autoSaving = false;
  }
}
onBeforeUnmount(() => {
  if (autoSaveTimer !== null) {
    window.clearTimeout(autoSaveTimer);
    autoSaveTimer = null;
  }
});

// ---- server.properties 配置项映射 ----
type PropField = {
  key: string;
  label: string;
  desc: string;
  type: "bool" | "int" | "string" | "enum";
  options?: string[];
  min?: number;
  max?: number;
  group: string;
};

const PROPS_SCHEMA: PropField[] = [
  { key: "server-port", label: $t("server-detail.server-port"), desc: $t("server-detail.int"), type: "int", min: 1, max: 65535, group: $t("server-detail.group") },
  { key: "max-players", label: $t("server-detail.max-players"), desc: $t("server-detail.tong-shi-zai-xian-wan-jia"), type: "int", min: 0, group: $t("server-detail.group") },
  { key: "motd", label: $t("server-detail.motd"), desc: $t("server-detail.string"), type: "string", group: $t("server-detail.group") },
  { key: "player-idle-timeout", label: $t("server-detail.player-idle-timeout"), desc: $t("server-detail.wan-jia-wu-cao-zuo-duo"), type: "int", min: 0, group: $t("server-detail.group") },
  { key: "gamemode", label: $t("server-detail.gamemode"), desc: $t("server-detail.enum"), type: "enum", options: ["survival", "creative", "adventure", "spectator"], group: $t("utils.categories.group") },
  { key: "difficulty", label: $t("utils.categories.difficulty"), desc: $t("server-detail.difficulty"), type: "enum", options: ["peaceful", "easy", "normal", "hard"], group: $t("utils.categories.group") },
  { key: "pvp", label: $t("server-detail.pvp"), desc: $t("server-detail.bool"), type: "bool", group: $t("utils.categories.group") },
  { key: "hardcore", label: $t("utils.categories.hardcore-mode"), desc: $t("server-detail.hardcore"), type: "bool", group: $t("utils.categories.group") },
  { key: "force-gamemode", label: $t("server-detail.force-gamemode"), desc: $t("server-detail.wan-jia-jia-ru-shi-qiang"), type: "bool", group: $t("utils.categories.group") },
  { key: "allow-flight", label: $t("server-detail.allow-flight"), desc: $t("server-detail.yun-xu-wan-jia-zai-sheng"), type: "bool", group: $t("utils.categories.group") },
  { key: "enable-command-block", label: $t("server-detail.enable-command-block"), desc: $t("server-detail.qi-yong-ming-ling-fang-kuai"), type: "bool", group: $t("utils.categories.group") },
  { key: "level-name", label: $t("server-detail.level-name"), desc: $t("server-detail.zhu-shi-jie-cun-dang-wen"), type: "string", group: $t("instance-detail.group") },
  { key: "level-seed", label: $t("server-detail.level-seed"), desc: $t("server-detail.liu-kong-ze-sui-ji-sheng"), type: "string", group: $t("instance-detail.group") },
  { key: "level-type", label: $t("server-detail.level-type"), desc: $t("server-detail.di-xing-sheng-cheng-fang-shi"), type: "enum", options: ["minecraft:normal", "minecraft:flat", "minecraft:large_biomes", "minecraft:amplified"], group: $t("instance-detail.group") },
  { key: "generate-structures", label: $t("server-detail.generate-structures"), desc: $t("server-detail.cun-zhuang-shen-dian-fei-qi"), type: "bool", group: $t("instance-detail.group") },
  { key: "allow-nether", label: $t("server-detail.allow-nether"), desc: $t("server-detail.sheng-cheng-xia-jie-wei-du"), type: "bool", group: $t("instance-detail.group") },
  { key: "spawn-animals", label: $t("server-detail.spawn-animals"), desc: $t("server-detail.sheng-cheng-niu-yang-zhu-deng"), type: "bool", group: $t("instance-detail.group") },
  { key: "spawn-monsters", label: $t("server-detail.spawn-monsters"), desc: $t("server-detail.sheng-cheng-jiang-shi-ku-lou"), type: "bool", group: $t("instance-detail.group") },
  { key: "spawn-npcs", label: $t("server-detail.spawn-npcs"), desc: $t("server-detail.sheng-cheng-cun-min-deng"), type: "bool", group: $t("instance-detail.group") },
  { key: "max-world-size", label: $t("server-detail.max-world-size"), desc: $t("server-detail.shi-jie-bian-jie-ban-jing"), type: "int", min: 0, group: $t("instance-detail.group") },
  { key: "online-mode", label: $t("server-detail.online-mode"), desc: $t("server-detail.yan-zheng-wan-jia-zhang-hao"), type: "bool", group: $t("server-detail.white-list") },
  { key: "white-list", label: $t("server-detail.bai-ming-dan"), desc: $t("server-detail.jin-bai-ming-dan-nei-wan"), type: "bool", group: $t("server-detail.white-list") },
  { key: "enforce-secure-profile", label: $t("server-detail.enforce-secure-profile"), desc: $t("server-detail.liao-tian-qian-ming-yan-zheng"), type: "bool", group: $t("server-detail.white-list") },
  { key: "prevent-proxy-connections", label: $t("server-detail.prevent-proxy-connections"), desc: $t("server-detail.ju-jue-tong-guo-dai-li"), type: "bool", group: $t("server-detail.white-list") },
  { key: "view-distance", label: $t("server-detail.view-distance"), desc: $t("server-detail.fa-song-gei-wan-jia-de"), type: "int", min: 3, max: 32, group: $t("utils.categories.int") },
  { key: "simulation-distance", label: $t("server-detail.simulation-distance"), desc: $t("server-detail.shi-ti-yu-fang-kuai-geng"), type: "int", min: 3, max: 32, group: $t("utils.categories.int") },
  { key: "network-compression-threshold", label: $t("server-detail.network-compression-threshold"), desc: $t("server-detail.shu-ju-bao-da-yu-ci"), type: "int", group: $t("utils.categories.int") },
  { key: "max-tick-time", label: $t("server-detail.max-tick-time"), desc: $t("server-detail.dan-t-i-c-k-chao"), type: "int", group: $t("utils.categories.int") },
  { key: "use-native-transport", label: $t("server-detail.use-native-transport"), desc: $t("server-detail.i-n-u-x-shang-shi"), type: "bool", group: $t("utils.categories.int") },
  { key: "sync-chunk-writes", label: $t("server-detail.sync-chunk-writes"), desc: $t("server-detail.qu-kuai-shu-ju-tong-bu"), type: "bool", group: $t("utils.categories.int") },
  { key: "entity-broadcast-range-percentage", label: $t("server-detail.entity-broadcast-range-percentage"), desc: $t("server-detail.shi-ti-dong-zuo-tong-bu"), type: "int", min: 0, max: 100, group: $t("utils.categories.int") },
  { key: "enable-jmx-monitoring", label: $t("server-detail.enable-jmx-monitoring"), desc: $t("server-detail.qi-yong-xing-neng-jian-kong"), type: "bool", group: $t("utils.categories.int") },
  { key: "enable-rcon", label: $t("server-detail.enable-rcon"), desc: $t("server-detail.yun-xu-tong-guo-xie-yi"), type: "bool", group: $t("server-detail.rcon-port") },
  { key: "rcon.port", label: $t("server-detail.duan-kou"), desc: $t("server-detail.fu-wu-duan-kou"), type: "int", min: 1, max: 65535, group: $t("server-detail.rcon-port") },
  { key: "rcon.password", label: $t("server-detail.rcon-password"), desc: $t("server-detail.ren-zheng-mi-ma"), type: "string", group: $t("server-detail.rcon-port") },
  { key: "enable-query", label: $t("server-detail.enable-query"), desc: $t("server-detail.yun-xu-wai-bu-cha-xun"), type: "bool", group: $t("server-detail.rcon-port") },
  { key: "query.port", label: $t("server-detail.query-port"), desc: $t("server-detail.u-e-r-y-fu-wu"), type: "int", min: 1, max: 65535, group: $t("server-detail.rcon-port") },
  { key: "resource-pack", label: $t("server-detail.resource-pack"), desc: $t("server-detail.wan-jia-jia-ru-shi-xia"), type: "string", group: $t("browse.group") },
  { key: "resource-pack-sha1", label: $t("server-detail.zi-yuan-bao-jiao-yan"), desc: $t("server-detail.zi-yuan-bao-ha-xi-zhi"), type: "string", group: $t("browse.group") },
  { key: "require-resource-pack", label: $t("server-detail.require-resource-pack"), desc: $t("server-detail.ju-jue-jia-zai-zi-yuan"), type: "bool", group: $t("browse.group") },
];

const PROPS_DEFAULTS: Record<string, string> = {
  "server-port": "25565", "max-players": "20", "motd": "A Minecraft Server", "player-idle-timeout": "0",
  "gamemode": "survival", "difficulty": "easy", "pvp": "true", "hardcore": "false",
  "force-gamemode": "false", "allow-flight": "false", "enable-command-block": "false",
  "level-name": "world", "level-seed": "", "level-type": "minecraft:normal",
  "generate-structures": "true", "allow-nether": "true", "spawn-animals": "true",
  "spawn-monsters": "true", "spawn-npcs": "true", "max-world-size": "29999984",
  "online-mode": "true", "white-list": "false", "enforce-secure-profile": "true",
  "prevent-proxy-connections": "false", "view-distance": "10", "simulation-distance": "10",
  "network-compression-threshold": "256", "max-tick-time": "60000", "use-native-transport": "true",
  "sync-chunk-writes": "true", "entity-broadcast-range-percentage": "100", "enable-jmx-monitoring": "false",
  "enable-rcon": "false", "rcon.port": "25575", "rcon.password": "", "enable-query": "false",
  "query.port": "25565", "resource-pack": "", "resource-pack-sha1": "", "require-resource-pack": "false",
};

const PROPS_GROUPS = computed(() => {
  const groups: string[] = [];
  for (const f of PROPS_SCHEMA) {
    if (!groups.includes(f.group)) groups.push(f.group);
  }
  return groups;
});

const propsMode = ref<"form" | "source">("form");
const propsData = ref<Record<string, string>>({});
const propsSource = ref("");
const propsExtra = ref<Record<string, string>>({});
const loadingProps = ref(false);
const savingProps = ref(false);

function parseProperties(text: string): { data: Record<string, string>; extra: Record<string, string> } {
  const data: Record<string, string> = {};
  const extra: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq < 0) continue;
    const key = trimmed.slice(0, eq).trim();
    const val = trimmed.slice(eq + 1).trim();
    data[key] = val;
    if (!PROPS_SCHEMA.some((f) => f.key === key)) extra[key] = val;
  }
  return { data, extra };
}

function buildProperties(): string {
  const lines: string[] = ["# Minecraft server properties", "# Generated by QookiX Launcher"];
  for (const f of PROPS_SCHEMA) {
    lines.push(`${f.key}=${propsData.value[f.key] ?? PROPS_DEFAULTS[f.key] ?? ""}`);
  }
  for (const [k, v] of Object.entries(propsExtra.value)) {
    lines.push(`${k}=${v}`);
  }
  return lines.join("\n") + "\n";
}

async function loadServerProperties() {
  loadingProps.value = true;
  try {
    const r = await api.readHostedServerFile(serverId, "server.properties");
    propsSource.value = r.content;
    const { data, extra } = parseProperties(r.content);
    const merged: Record<string, string> = {};
    for (const f of PROPS_SCHEMA) merged[f.key] = data[f.key] ?? PROPS_DEFAULTS[f.key] ?? "";
    propsData.value = merged;
    propsExtra.value = extra;
  } catch {
    propsData.value = { ...PROPS_DEFAULTS };
    propsExtra.value = {};
    propsSource.value = buildProperties();
  } finally {
    loadingProps.value = false;
  }
}

async function savePropsForm() {
  savingProps.value = true;
  try {
    const text = buildProperties();
    await api.writeHostedServerFile(serverId, "server.properties", text);
    propsSource.value = text;
    message.success($t("server-detail.config-saved"));
  } catch (e) {
    message.error(String(e));
  } finally {
    savingProps.value = false;
  }
}

async function savePropsSource() {
  savingProps.value = true;
  try {
    await api.writeHostedServerFile(serverId, "server.properties", propsSource.value);
    const { data, extra } = parseProperties(propsSource.value);
    const merged: Record<string, string> = {};
    for (const f of PROPS_SCHEMA) merged[f.key] = data[f.key] ?? PROPS_DEFAULTS[f.key] ?? "";
    propsData.value = merged;
    propsExtra.value = extra;
    message.success($t("server-detail.config-saved"));
  } catch (e) {
    message.error(String(e));
  } finally {
    savingProps.value = false;
  }
}

// ---- 其他配置文件 ----
const CONFIG_DOCS: Record<string, string> = {
  "eula.txt": $t("server-detail.eula-txt"),
  "ops.json": $t("server-detail.ops-json"),
  "whitelist.json": $t("server-detail.whitelist-json"),
  "banned-players.json": $t("server-detail.banned-players-json"),
  "banned-ips.json": $t("server-detail.banned-ips-json"),
  "spigot.yml": $t("server-detail.spigot-yml"),
  "paper.yml": $t("server-detail.paper-yml"),
  "paper-global.yml": $t("server-detail.paper-global-yml"),
  "paper-world-defaults.yml": $t("server-detail.paper-world-defaults-yml"),
  "purpur.yml": $t("server-detail.purpur-yml"),
  "bukkit.yml": $t("server-detail.bukkit-yml"),
  "commands.yml": $t("server-detail.commands-yml"),
  "pufferfish.yml": $t("server-detail.pufferfish-yml"),
  "permissions.yml": $t("server-detail.permissions-yml"),
  "help.yml": $t("server-detail.help-yml"),
  "fabric-server.properties": $t("server-detail.fabric-server-properties"),
  "logs.yml": $t("server-detail.logs-yml"),
};

type ConfigFile = { name: string; rel: string; size: number; modified: number };
const configFiles = ref<ConfigFile[]>([]);
const loadingConfigs = ref(false);

function configDoc(rel: string): string {
  const name = rel.split("/").pop() ?? rel;
  if (CONFIG_DOCS[name]) return CONFIG_DOCS[name];
  if (rel.startsWith("config/")) return $t("server-detail.config-desc");
  return $t("server-detail.custom-config");
}

async function loadConfigFiles() {
  loadingConfigs.value = true;
  try {
    const all = await api.listHostedServerConfigFiles(serverId);
    configFiles.value = all.filter((f) => f.rel !== "server.properties");
  } catch {
    configFiles.value = [];
  } finally {
    loadingConfigs.value = false;
  }
}

// ---- 配置文件编辑器 ----
type EditorState = {
  rel: string;
  name: string;
  doc: string;
  content: string;
  loading: boolean;
  saving: boolean;
  error: string | null;
};
const editor = ref<EditorState | null>(null);

async function openEditor(f: ConfigFile) {
  editor.value = {
    rel: f.rel,
    name: f.name,
    doc: configDoc(f.rel),
    content: "",
    loading: true,
    saving: false,
    error: null,
  };
  try {
    const r = await api.readHostedServerFile(serverId, f.rel);
    let content = r.content;
    if (f.name.endsWith(".json")) {
      try {
        content = JSON.stringify(JSON.parse(content), null, 2);
      } catch {
        /* 非合法 JSON，原样显示 */
      }
    }
    if (editor.value) editor.value.content = content;
  } catch (e) {
    if (editor.value) editor.value.error = String(e);
  } finally {
    if (editor.value) editor.value.loading = false;
  }
}

async function saveEditor() {
  const ed = editor.value;
  if (!ed) return;
  ed.saving = true;
  try {
    await api.writeHostedServerFile(serverId, ed.rel, ed.content);
    message.success($t("server-detail.config-saved"));
    editor.value = null;
    loadConfigFiles();
  } catch (e) {
    message.error(String(e));
  } finally {
    if (editor.value) editor.value.saving = false;
  }
}

// ---- mods / plugins 列表 ----
type FileEntry = { name: string; size: number; modified: number; isDir: boolean };
const fileList = ref<FileEntry[]>([]);
const loadingFiles = ref(false);

async function loadFileList(sub: string) {
  loadingFiles.value = true;
  try {
    const r = await api.listHostedServerFiles(serverId, sub);
    fileList.value = r.files.map((f) => ({
      name: f.name,
      size: f.size,
      modified: f.modified,
      isDir: f.isDir,
    }));
  } catch (e) {
    fileList.value = [];
    message.error(String(e));
  } finally {
    loadingFiles.value = false;
  }
}

watch(tab, (t) => {
  if (t === "mods") loadFileList("mods");
  else if (t === "plugins") loadFileList("plugins");
  else if (t === "config") {
    loadServerProperties();
    loadConfigFiles();
  }
});

// ---- 日志 ----
type LogLine = { stream: "out" | "err"; line: string };
const logs = ref<LogLine[]>([]);
const logBox = ref<HTMLElement | null>(null);
const MAX_LOG_LINES = 2000;

function pushLog(stream: "out" | "err", line: string) {
  logs.value.push({ stream, line });
  if (logs.value.length > MAX_LOG_LINES) {
    logs.value = logs.value.slice(-MAX_LOG_LINES);
  }
  nextTick(() => {
    if (logBox.value) logBox.value.scrollTop = logBox.value.scrollHeight;
  });
}
function clearLogs() {
  logs.value = [];
}

const logText = computed(() => logs.value.map((l) => l.line).join("\n"));

async function copyLogs() {
  if (!logText.value) return message.info($t("log-viewer.no-logs"));
  try {
    await navigator.clipboard.writeText(logText.value);
    message.success($t("log-viewer.all-copied"));
  } catch {
    const ta = document.createElement("textarea");
    ta.value = logText.value;
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(ta);
    if (ok) message.success($t("log-viewer.all-copied"));
    else message.error($t("crash-analyzer.copy-failed"));
  }
}

async function exportLogs() {
  if (!logText.value) return message.info($t("log-viewer.no-logs"));
  const ts = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const defaultName = `server-${ts.getFullYear()}${pad(ts.getMonth() + 1)}${pad(ts.getDate())}-${pad(ts.getHours())}${pad(ts.getMinutes())}${pad(ts.getSeconds())}.log`;
  const path = await save({
    defaultPath: defaultName,
    filters: [{ name: $t("log-viewer.log-file"), extensions: ["log", "txt"] }],
  });
  if (!path) return;
  try {
    await api.saveTextFile(path as string, logText.value);
    message.success($t("log-viewer.exported"));
  } catch (e) {
    message.error(String(e));
  }
}

// ---- 启动 / 停止 ----
const starting = ref(false);
async function start() {
  if (!server.value) return;
  if (!server.value.eula) {
    message.warning($t("server-detail.eula-required"));
    tab.value = "settings";
    return;
  }
  starting.value = true;
  tab.value = "logs";
  try {
    try {
      await servers.start(serverId);
      message.success($t("multiplayer.server-started"));
      tab.value = "logs";
    } catch (e) {
      const msg = String(e);
      if (msg.includes("server.jar 不存在")) {
        message.info($t("server-detail.preparing-core"));
        await servers.installCore(serverId);
        await servers.start(serverId);
        message.success($t("multiplayer.server-started"));
        tab.value = "logs";
      } else {
        throw e;
      }
    }
  } catch (e) {
    message.error(String(e));
  } finally {
    starting.value = false;
  }
}

async function stop() {
  try {
    await servers.stop(serverId);
    message.success($t("multiplayer.server-stopped"));
  } catch (e) {
    message.error(String(e));
  }
}

function openFolder(sub?: string) {
  api.openHostedServerFolder(serverId, sub).catch((e) => message.error(String(e)));
}

// ---- 删除 ----
const confirmDelete = ref(false);
async function doDelete() {
  try {
    await servers.remove(serverId);
    message.success($t("server-detail.server-deleted"));
    router.push("/multiplayer");
  } catch (e) {
    message.error(String(e));
  }
}

// ---- 生命周期 ----
let unlisteners: UnlistenFn[] = [];
onMounted(async () => {
  await servers.load();
  syncForm();
  loadConfigFiles();
  try {
    const r = await api.listHostedServerFolders(serverId);
    folders.value = Object.fromEntries(r.folders.map((f) => [f.name, f.exists]));
  } catch {
    /* ignore */
  }
  try {
    const history = await api.readHostedServerLog(serverId);
    if (history.length) {
      logs.value = history.map((line) => ({ stream: "out" as const, line }));
      nextTick(() => {
        if (logBox.value) logBox.value.scrollTop = logBox.value.scrollHeight;
      });
    }
  } catch {
    /* ignore */
  }
  const u1 = await listen<{ serverId: string; stream: "out" | "err"; line: string }>(
    "server://log",
    (ev) => {
      if (ev.payload.serverId === serverId) pushLog(ev.payload.stream, ev.payload.line);
    },
  );
  unlisteners.push(u1);
  const u2 = await listen<{ serverId: string; state: string; pid: number; code: number | null }>(
    "server://state",
    (ev) => {
      if (ev.payload.serverId !== serverId) return;
      servers.setRunning(serverId, ev.payload.state === "running");
      if (ev.payload.state === "exited") {
        pushLog("err", $t("server-detail.push-log", { p1: ev.payload.code ?? "?" }));
      }
    },
  );
  unlisteners.push(u2);
  const u3 = await listen<{ serverId: string; code: number; tail: string[] }>(
    "server://error",
    (ev) => {
      if (ev.payload.serverId !== serverId) return;
      const detail = ev.payload.tail.length
        ? "\n" + ev.payload.tail.join("\n")
        : "";
      message.error($t("server-detail.listen", { p1: ev.payload.code, p2: detail }), { duration: 8000 });
    },
  );
  unlisteners.push(u3);
});

onBeforeUnmount(() => {
  for (const u of unlisteners) u();
  unlisteners = [];
});
</script>

<template>
  <div class="server-detail" v-if="server">
    <button class="back" @click="router.push('/multiplayer')">
      <IconChevronLeft />{{ $t("server-detail.back-to-servers") }}</button>

    <div class="head glass">
      <div class="head-info">
        <span
          class="core-badge"
          :style="{ color: CORE_COLORS[server.core], borderColor: CORE_COLORS[server.core] }"
        >
          {{ CORE_LABELS[server.core] }}
        </span>
        <h2>{{ server.name }}</h2>
        <p class="head-sub">
          <span class="mono">{{ server.mc_version }}</span>{{ $t("server-detail.server-meta", { p1: server.port, p2: server.max_memory_mb }) }}</p>
      </div>
      <div class="head-ops">
        <button v-if="!running" class="btn primary" :disabled="starting" @click="start">
          <IconPlay /> {{ starting ? $t('instance-saves.launching') : $t('instance-saves.launch') }}
        </button>
        <button v-else class="btn warn" @click="stop">
          <IconStop />{{ $t("multiplayer.stop") }}</button>
        <button class="btn ghost" @click="openFolder()"><IconFolder />{{ $t("server-detail.directory") }}</button>
      </div>
    </div>

    <div class="tabs glass">
      <button
        v-for="t in tabs"
        :key="t.key"
        :class="{ active: tab === t.key }"
        @click="tab = t.key"
      >
        {{ t.label }}
      </button>
    </div>

    <!-- 设置 -->
    <div v-if="tab === 'settings'" class="panel glass">
      <div class="section">
        <h3 class="section-title">{{ $t("server-detail.group") }}</h3>
        <p class="section-hint">{{ $t("server-detail.autosave-hint") }}</p>
        <div class="field">
          <label>{{ $t("multiplayer.server-name") }}</label>
          <n-input v-model:value="form.name" maxlength="40" @update:value="scheduleAutoSave" />
        </div>
        <div class="field-row">
          <div class="field">
            <label>{{ $t("server-detail.max-memory") }}</label>
            <n-input-number v-model:value="form.maxMem" :min="256" :step="256" @update:value="scheduleAutoSave" />
          </div>
          <div class="field">
            <label>{{ $t("server-detail.min-memory") }}</label>
            <n-input-number v-model:value="form.minMem" :min="128" :step="256" @update:value="scheduleAutoSave" />
          </div>
        </div>
        <div class="field eula">
          <n-checkbox v-model:checked="form.eula" @update:checked="scheduleAutoSave">{{ $t("server-detail.eula-agree") }}<a href="https://account.mojang.com/documents/Minecraft_EULA" target="_blank" rel="noopener">Minecraft EULA</a>
          </n-checkbox>
        </div>
      </div>


      <div class="section">
        <h3 class="section-title">{{ $t("server-detail.jvm-args") }}</h3>
        <p class="section-hint">{{ $t("server-detail.jvm-args-hint") }}</p>
        <n-input v-model:value="form.jvmArgs" :placeholder="$t('instance-settings.jvm-args-hint')" @update:value="scheduleAutoSave" />
      </div>

      <div class="section">
        <h3 class="section-title">{{ $t("server-detail.stop-command") }}</h3>
        <p class="section-hint">{{ $t("server-detail.stop-command-hint") }}</p>
        <n-input v-model:value="form.stopCommand" placeholder="stop" @update:value="scheduleAutoSave" />
      </div>

      <div class="panel-foot">
        <button class="btn danger" @click="confirmDelete = true"><IconTrash />{{ $t("multiplayer.delete-server") }}</button>
        <span v-if="autoSaving" class="save-hint">{{ $t("file-manager.saving") }}</span>
      </div>
    </div>

    <!-- 配置文件 -->
    <div v-else-if="tab === 'config'" class="panel glass">
      <!-- server.properties -->
      <div class="section">
        <div class="section-head">
          <h3 class="section-title">server.properties</h3>
          <div class="props-mode-tabs">
            <button :class="{ active: propsMode === 'form' }" @click="propsMode = 'form'">{{ $t("server-detail.form") }}</button>
            <button :class="{ active: propsMode === 'source' }" @click="propsMode = 'source'">{{ $t("server-detail.source-file") }}</button>
          </div>
        </div>
        <p class="section-hint">{{ $t("server-detail.server-properties-desc") }}</p>

        <div v-if="propsMode === 'form'" class="props-form">
          <div v-if="loadingProps" class="empty-inline">{{ $t("server-detail.loading-config") }}</div>
          <template v-else>
            <div v-for="g in PROPS_GROUPS" :key="g" class="props-group">
              <h4 class="props-group-title">{{ g }}</h4>
              <div class="props-grid">
                <div
                  v-for="f in PROPS_SCHEMA.filter(s => s.group === g)"
                  :key="f.key"
                  class="prop-item"
                >
                  <div class="prop-label">
                    <span class="prop-name">{{ f.label }}</span>
                    <span class="prop-desc">{{ f.desc }}</span>
                  </div>
                  <div class="prop-control">
                    <n-switch
                      v-if="f.type === 'bool'"
                      :value="propsData[f.key] === 'true'"
                      @update:value="(v: boolean) => propsData[f.key] = v ? 'true' : 'false'"
                    />
                    <n-select
                      v-else-if="f.type === 'enum'"
                      :value="propsData[f.key]"
                      :options="f.options!.map(o => ({ label: o, value: o }))"
                      size="small"
                      @update:value="(v: string) => propsData[f.key] = v"
                    />
                    <n-input-number
                      v-else-if="f.type === 'int'"
                      :value="Number(propsData[f.key])"
                      :min="f.min"
                      :max="f.max"
                      size="small"
                      @update:value="(v: number | null) => propsData[f.key] = String(v ?? 0)"
                    />
                    <n-input
                      v-else
                      :value="propsData[f.key]"
                      size="small"
                      @update:value="(v: string) => propsData[f.key] = v"
                    />
                  </div>
                </div>
              </div>
            </div>
          </template>
          <div class="panel-foot">
            <button class="btn primary" :disabled="savingProps || loadingProps" @click="savePropsForm">
              {{ savingProps ? $t('file-manager.saving') : $t('server-detail.save-config') }}
            </button>
          </div>
        </div>

        <div v-else class="props-source">
          <div v-if="loadingProps" class="empty-inline">{{ $t("server-detail.loading-config") }}</div>
          <template v-else>
            <n-input
              v-model:value="propsSource"
              type="textarea"
              class="editor-textarea"
              :autosize="false"
              spellcheck="false"
            />
            <div class="panel-foot">
              <button class="btn primary" :disabled="savingProps" @click="savePropsSource">
                {{ savingProps ? $t('file-manager.saving') : $t('server-detail.save-config') }}
              </button>
            </div>
          </template>
        </div>
      </div>

      <!-- 其他配置文件 -->
      <div class="section">
        <div class="section-head">
          <h3 class="section-title">{{ $t("server-detail.other-configs") }}</h3>
          <button class="btn sm ghost" @click="loadConfigFiles"><IconRefresh />{{ $t("common.refresh") }}</button>
        </div>
        <p class="section-hint">{{ $t("server-detail.other-configs-desc") }}</p>
        <div v-if="loadingConfigs" class="empty-inline">{{ $t("server-detail.scanning-configs") }}</div>
        <div v-else-if="!configFiles.length" class="empty-inline">{{ $t("server-detail.no-other-configs") }}</div>
        <div v-else class="config-list">
          <button
            v-for="f in configFiles"
            :key="f.rel"
            class="config-row"
            @click="openEditor(f)"
          >
            <div class="config-info">
              <span class="config-name mono">{{ f.rel }}</span>
              <span class="config-doc">{{ configDoc(f.rel) }}</span>
            </div>
            <span class="config-size">{{ fmtSize(f.size) }}</span>
          </button>
        </div>
      </div>
    </div>

    <!-- 模组 -->
    <div v-else-if="tab === 'mods'" class="panel glass">
      <div class="panel-head">
        <h3><IconBox />{{ $t("browse.mods") }}</h3>
        <button class="btn sm ghost" @click="openFolder('mods')">{{ $t("server-detail.open-mods") }}</button>
      </div>
      <div v-if="loadingFiles" class="empty-inline">{{ $t("file-manager.loading") }}</div>
      <div v-else-if="!fileList.length" class="empty-inline">{{ $t("server-detail.mods-empty") }}<code>mods/</code>{{ $t("server-detail.mods-dir") }}</div>
      <div v-else class="file-list">
        <div v-for="f in fileList" :key="f.name" class="file-row">
          <span class="file-name">{{ f.name }}</span>
          <span class="file-size">{{ fmtSize(f.size) }}</span>
        </div>
      </div>
    </div>

    <!-- 插件 -->
    <div v-else-if="tab === 'plugins'" class="panel glass">
      <div class="panel-head">
        <h3><IconBox />{{ $t("server-detail.plugins") }}</h3>
        <button class="btn sm ghost" @click="openFolder('plugins')">{{ $t("server-detail.open-plugins") }}</button>
      </div>
      <div v-if="loadingFiles" class="empty-inline">{{ $t("file-manager.loading") }}</div>
      <div v-else-if="!fileList.length" class="empty-inline">{{ $t("server-detail.plugins-empty") }}<code>plugins/</code>{{ $t("server-detail.mods-dir") }}</div>
      <div v-else class="file-list">
        <div v-for="f in fileList" :key="f.name" class="file-row">
          <span class="file-name">{{ f.name }}</span>
          <span class="file-size">{{ fmtSize(f.size) }}</span>
        </div>
      </div>
    </div>

    <!-- 日志 -->
    <div v-else-if="tab === 'logs'" class="panel glass log-panel">
      <div class="panel-head">
        <h3><IconFile />{{ $t("server-detail.server-logs") }}</h3>
        <div class="log-ops">
          <span class="run-dot" :class="{ on: running }"></span>
          <span class="run-text">{{ running ? $t('multiplayer.running') : $t('server-detail.not-running') }}</span>
          <button class="btn sm ghost" :title="$t('log-viewer.copy-all')" :aria-label="$t('log-viewer.copy-all')" @click="copyLogs"><IconCopy />{{ $t("common.copy") }}</button>
          <button class="btn sm ghost" :title="$t('log-viewer.export-file')" :aria-label="$t('log-viewer.export-file')" @click="exportLogs"><IconDownload />{{ $t("log-viewer.export") }}</button>
          <button class="btn sm ghost" @click="clearLogs">{{ $t("log-viewer.clear") }}</button>
        </div>
      </div>
      <div ref="logBox" class="log-box">
        <div v-if="!logs.length" class="log-empty">{{ $t("server-detail.logs-hint") }}</div>
        <div
          v-for="(l, i) in logs"
          :key="i"
          class="log-line"
          :class="l.stream"
        >{{ l.line }}</div>
      </div>
    </div>

    <!-- 文件 -->
    <div v-else-if="tab === 'files'" class="panel glass">
      <div class="panel-head">
        <h3><IconFolder />{{ $t("server-detail.server-files") }}</h3>
      </div>
      <p class="section-hint">{{ $t("server-detail.files-hint") }}</p>
      <ServerFileManager :server-id="serverId" />
    </div>

    <!-- 配置文件编辑器 -->
    <app-sheet
      :show="editor !== null"
      :title="editor?.name ?? ''"
      @update:show="(v: boolean) => { if (!v) editor = null; }"
    >
      <div v-if="editor" class="editor-body">
        <p class="editor-doc">{{ editor.doc }}</p>
        <div v-if="editor.loading" class="editor-loading">{{ $t("server-detail.reading-file") }}</div>
        <div v-else-if="editor.error" class="editor-error">{{ editor.error }}</div>
        <n-input
          v-else
          v-model:value="editor.content"
          type="textarea"
          class="editor-textarea"
          :autosize="false"
          spellcheck="false"
        />
        <div class="editor-foot">
          <button class="btn ghost" @click="editor = null">{{ $t("common.cancel") }}</button>
          <button
            class="btn primary"
            :disabled="editor.loading || editor.saving"
            @click="saveEditor"
          >
            {{ editor.saving ? $t('file-manager.saving') : $t('common.save') }}
          </button>
        </div>
      </div>
    </app-sheet>

    <!-- 删除确认。Teleport 到 body 的原因见 FileManager `确认弹窗` 注释：
         `.body` 是层叠上下文，而 `.mobile-nav` 是它的兄弟，会恒在遮罩之上。 -->
    <Teleport to="body">
      <div v-if="confirmDelete" class="mask" @click="confirmDelete = false">
        <div class="confirm-card glass" @click.stop>
          <h3>{{ $t("multiplayer.delete-server") }}</h3>
          <p>{{ $t("multiplayer.delete-server-confirm-a") }}<b>{{ server.name }}</b>{{ $t("server-detail.delete-confirm-b") }}</p>
          <div class="confirm-foot">
            <button class="btn ghost" @click="confirmDelete = false">{{ $t("common.cancel") }}</button>
            <button class="btn danger" @click="doDelete">{{ $t("common.delete") }}</button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>

  <div v-else class="loading">{{ $t("file-manager.loading") }}</div>
</template>

<style scoped>
.server-detail {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.back {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  align-self: flex-start;
  background: none;
  border: none;
  color: var(--text-3);
  font-size: 13px;
  cursor: pointer;
  font-family: inherit;
  padding: 6px 10px;
  border-radius: 8px;
}
.back:hover {
  color: var(--text-1);
  background: rgba(255, 255, 255, 0.06);
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 20px 24px;
  flex-wrap: wrap;
}
.head-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.core-badge {
  align-self: flex-start;
  font-size: 11px;
  font-weight: 700;
  padding: 3px 9px;
  border-radius: 999px;
  border: 1px solid;
  background: rgba(255, 255, 255, 0.04);
  letter-spacing: 0.02em;
}
.head-info h2 {
  margin: 4px 0 0;
  font-size: 20px;
}
.head-sub {
  margin: 0;
  font-size: 12px;
  color: var(--text-3);
}
.head-ops {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  border: none;
  border-radius: 10px;
  padding: 9px 16px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.15s;
}
.btn.sm {
  padding: 6px 12px;
  font-size: 12px;
}
.btn.primary {
  background: linear-gradient(135deg, var(--accent), var(--accent-deep));
  color: #1a1208;
}
.btn.primary:hover:not(:disabled) {
  filter: brightness(1.08);
}
.btn.ghost {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-1);
  border: 1px solid var(--border);
}
.btn.ghost:hover {
  background: rgba(255, 255, 255, 0.1);
}
.btn.warn {
  background: rgba(224, 168, 90, 0.18);
  color: #e0a85a;
  border: 1px solid rgba(224, 168, 90, 0.4);
}
.btn.warn:hover {
  background: rgba(224, 168, 90, 0.28);
}
.btn.danger {
  background: rgba(229, 83, 75, 0.16);
  color: #f0907f;
  border: 1px solid rgba(229, 83, 75, 0.4);
}
.btn.danger:hover {
  background: rgba(229, 83, 75, 0.26);
}
.btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.tabs {
  display: inline-flex;
  gap: 4px;
  padding: 5px;
  align-self: flex-start;
}
.tabs button {
  border: none;
  background: transparent;
  color: var(--text-2);
  padding: 8px 18px;
  border-radius: 9px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
}
.tabs button.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.panel {
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.section {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.section-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
.section-hint {
  margin: -6px 0 0;
  font-size: 12px;
  color: var(--text-3);
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.panel-head h3 {
  margin: 0;
  font-size: 15px;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.field label {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
}
.field-row {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}
.field-row .field {
  flex: 1;
  min-width: 120px;
}
.eula a {
  color: var(--accent);
  text-decoration: none;
}
.eula a:hover {
  text-decoration: underline;
}
.panel-foot {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}
.save-hint {
  font-size: 13px;
  color: var(--text-3);
}
.empty-inline {
  padding: 24px;
  text-align: center;
  color: var(--text-3);
  font-size: 13px;
  border: 1px dashed var(--border);
  border-radius: 10px;
}
.empty-inline code {
  font-family: "Cascadia Code", Consolas, monospace;
  color: var(--text-2);
}
.config-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.config-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 11px 14px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.03);
  cursor: pointer;
  font-family: inherit;
  text-align: left;
  transition: all 0.13s;
}
.config-row:hover {
  background: var(--accent-soft);
  border-color: var(--accent-35);
}
.config-info {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
  flex: 1;
}
.config-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.config-doc {
  font-size: 11px;
  color: var(--text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.config-row:hover .config-doc {
  color: var(--accent-60);
}
.config-size {
  font-size: 11px;
  color: var(--text-3);
  flex-shrink: 0;
}
.file-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 420px;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 6px;
}
.file-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 13px;
}
.file-row:hover {
  background: rgba(255, 255, 255, 0.05);
}
.file-name {
  color: var(--text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-size {
  color: var(--text-3);
  font-size: 11px;
  flex-shrink: 0;
}
.log-panel {
  gap: 12px;
}
.log-ops {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.run-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-3);
}
.run-dot.on {
  background: #57c98a;
  box-shadow: 0 0 6px rgba(87, 201, 138, 0.6);
}
.run-text {
  font-size: 12px;
  color: var(--text-3);
}
.log-box {
  height: 420px;
  overflow-y: auto;
  background: rgba(0, 0, 0, 0.32);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px 14px;
  font-family: "Cascadia Code", Consolas, "Courier New", monospace;
  font-size: 12px;
  line-height: 1.55;
}
.log-empty {
  color: var(--text-3);
  text-align: center;
  padding: 40px 0;
}
.log-line {
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--text-2);
}
.log-line.err {
  color: #f0907f;
}
.file-shortcuts {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}
.shortcut {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 9px 14px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-2);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.13s;
}
.shortcut:hover {
  background: var(--accent-soft);
  color: var(--accent);
  border-color: var(--accent-35);
}
.loading {
  padding: 60px;
  text-align: center;
  color: var(--text-3);
}
.mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  /* 1200：要压过 `.mobile-nav` 底栏（原来 100，被底栏压住） */
  z-index: 1200;
}
.confirm-card {
  width: 420px;
  max-width: 92vw;
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.confirm-card h3 {
  margin: 0;
  font-size: 16px;
}
.confirm-card p {
  margin: 0;
  font-size: 13px;
  color: var(--text-2);
  line-height: 1.6;
}
.confirm-card b {
  color: var(--text-1);
}
.confirm-foot {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 4px;
}
.editor-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.editor-doc {
  margin: 0;
  font-size: 12px;
  color: var(--text-3);
  padding: 8px 12px;
  background: var(--accent-soft);
  border-radius: 8px;
  border-left: 3px solid var(--accent);
}
.editor-loading,
.editor-error {
  padding: 30px;
  text-align: center;
  font-size: 13px;
  color: var(--text-3);
}
.editor-error {
  color: #f0907f;
}
.editor-textarea {
  width: 100%;
  min-height: 360px;
  max-height: calc(56vh / var(--ui-scale, 1));
  resize: vertical;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px 14px;
  background: rgba(0, 0, 0, 0.28);
  color: var(--text-1);
  font-family: "Cascadia Code", Consolas, "Courier New", monospace;
  font-size: 12.5px;
  line-height: 1.55;
  outline: none;
  box-sizing: border-box;
  transition: border-color 0.13s;
}
.editor-textarea:focus {
  border-color: var(--accent-45);
}
.editor-foot {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
.props-mode-tabs {
  display: inline-flex;
  gap: 4px;
  padding: 3px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--border);
}
.props-mode-tabs button {
  border: none;
  background: transparent;
  color: var(--text-3);
  padding: 5px 14px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
}
.props-mode-tabs button.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.props-form {
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.props-group {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.props-group-title {
  margin: 0;
  font-size: 13px;
  font-weight: 700;
  color: var(--text-2);
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border);
}
.props-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(280px, 100%), 1fr));
  gap: 12px;
}
.prop-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border);
}
.prop-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.prop-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
}
.prop-desc {
  font-size: 11px;
  color: var(--text-3);
  line-height: 1.4;
}
.prop-control {
  display: flex;
  align-items: center;
}
.props-source {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
</style>
