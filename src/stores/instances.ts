import { defineStore } from "pinia";
import { api } from "../api";
import type { Instance, InstanceGroup } from "../types";
import { autoRendererFor, effectiveRendererKey, rendererLabel } from "../utils/renderer";

/** 启动前的渲染器确认（由全局弹窗消费），见 `rendererGuard`。 */
export interface RendererGuard {
  instanceId: string;
  mcVersion: string;
  /** 当前会用的渲染器键 */
  used: string;
  usedName: string;
  /** 版本推荐的渲染器键 */
  recommended: string;
  recommendedName: string;
  world?: string;
  server?: string;
}

/**
 * 本地「已经问过」标记。两套 key：
 *  - `warned`：启动后（日志里发现失败证据）已经问过；
 *  - `guardOk`：启动前确认弹窗里用户已经选过「仍用 x 启动」。
 * 分开记是因为两者的确认语义不同：启动前点「仍用」= 每次启动就这么跑；
 * 事后点「暂不切换」≠ 允许以后每次启动都再白屏一次。
 */
function rendererWarnKey(instanceId: string, renderer: string) {
  return `qk.rendererWarned.${instanceId}.${renderer}`;
}
function rendererGuardKey(instanceId: string, renderer: string) {
  return `qk.rendererGuardOk.${instanceId}.${renderer}`;
}
function readFlag(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}
function writeFlag(key: string): void {
  try {
    localStorage.setItem(key, "1");
  } catch {
    /* 隐私模式下写不了就算了，最多多问一次 */
  }
}

export function hasRendererWarned(instanceId: string, renderer: string): boolean {
  return readFlag(rendererWarnKey(instanceId, renderer));
}
export function markRendererWarned(instanceId: string, renderer: string): void {
  writeFlag(rendererWarnKey(instanceId, renderer));
}
/** 用户在启动前确认里选了「仍用该渲染器启动」，记下来别再问。 */
export function markRendererGuardOk(instanceId: string, renderer: string): void {
  writeFlag(rendererGuardKey(instanceId, renderer));
}

export const useInstancesStore = defineStore("instances", {
  state: () => ({
    instances: [] as Instance[],
    groups: [] as InstanceGroup[],
    loading: false,
    installingId: null as string | null,
    launchingId: null as string | null,
    installStage: "",
    installDone: 0,
    installTotal: 0,
    /** 最近一次成功拉取的时间戳，用于短 TTL 内跳过重复请求 */
    lastLoadedAt: 0,
    /** 等用户决定的「渲染器不推荐」确认（弹窗消费后置 null） */
    pendingRendererGuard: null as RendererGuard | null,
    /** 全局渲染器（Pojav 偏好，懒加载缓存） */
    globalRenderer: null as string | null,
  }),
  getters: {
    /** 分组名 -> 分组对象，便于 UI 快速取色 */
    groupMap(state): Record<string, InstanceGroup> {
      const map: Record<string, InstanceGroup> = {};
      for (const g of state.groups) map[g.id] = g;
      return map;
    },
    /** 未分组实例 */
    ungrouped(state): Instance[] {
      return state.instances.filter((i) => !i.group);
    },
    /** 取某个分组下的实例（保持 instances 的默认排序） */
    inGroup: (state) => (groupId: string) =>
      state.instances.filter((i) => i.group === groupId),
  },
  actions: {
    /** 后台静默刷新（不改动 loading），失败保留旧数据 */
    async refresh() {
      try {
        const [list, groups] = await Promise.all([api.listInstances(), api.listGroups()]);
        this.instances = list;
        this.groups = groups;
        this.lastLoadedAt = Date.now();
      } catch {
        /* 后台刷新失败保留旧数据 */
      }
    },
    /**
     * 拉取实例列表。
     * - 已有数据时采用 stale-while-revalidate：立即返回旧数据，后台静默刷新替换。
     * - `force=true` 时始终前台拉取并设 loading（用于写操作后强制刷新）。
     */
    async load(force = false) {
      const now = Date.now();
      // 3 秒内刚拉过，直接跳过，避免切换页面时重复请求
      if (!force && this.lastLoadedAt && now - this.lastLoadedAt < 3000) return;
      const hasData = this.instances.length > 0 || this.groups.length > 0;
      if (hasData && !force) {
        void this.refresh();
        return;
      }
      this.loading = true;
      try {
        const [list, groups] = await Promise.all([api.listInstances(), api.listGroups()]);
        this.instances = list;
        this.groups = groups;
        this.lastLoadedAt = Date.now();
      } finally {
        this.loading = false;
      }
    },
    get(id: string) {
      return this.instances.find((i) => i.id === id);
    },
    groupById(id: string | null | undefined): InstanceGroup | null {
      if (!id) return null;
      return this.groups.find((g) => g.id === id) ?? null;
    },
    // ---- 分组管理 ----
    async loadGroups() {
      this.groups = await api.listGroups();
    },
    async createGroup(name: string, color?: string | null) {
      const g = await api.createGroup(name, color ?? null);
      await this.loadGroups();
      return g;
    },
    async renameGroup(id: string, name: string, color?: string | null) {
      const g = await api.renameGroup(id, name, color ?? null);
      await this.loadGroups();
      return g;
    },
    async deleteGroup(id: string) {
      await api.deleteGroup(id);
      // 组内实例被后端移回未分组，重新拉取保持一致
      await this.load(true);
    },
    async reorderGroups(ids: string[]) {
      this.groups = await api.reorderGroups(ids);
    },
    /** 移动实例到分组（groupId 为空表示移出分组） */
    async moveToGroup(instanceId: string, groupId: string | null) {
      const inst = await api.updateInstance({ id: instanceId, group: groupId ?? "" });
      const idx = this.instances.findIndex((i) => i.id === inst.id);
      if (idx >= 0) this.instances[idx] = inst;
      return inst;
    },
    async create(name: string, mc: string, loader: string, loaderVersion: string | null) {
      const inst = await api.createInstance(name, mc, loader, loaderVersion);
      await this.load(true);
      return inst;
    },
    async patch(patch: Record<string, unknown>) {
      const inst = await api.updateInstance(patch);
      const idx = this.instances.findIndex((i) => i.id === inst.id);
      if (idx >= 0) this.instances[idx] = inst;
      return inst;
    },
    async remove(id: string) {
      await api.deleteInstance(id);
      await this.load(true);
    },
    async installGame(id: string) {
      this.installingId = id;
      this.installStage = "准备中…";
      this.installDone = 0;
      this.installTotal = 0;
      try {
        await api.installGame(id);
        await this.load(true);
      } finally {
        this.installingId = null;
      }
    },
    /**
     * 取消安装的任务请走「下载中心」卡片上的取消按钮
     * （`DownloadsView` → `api.cancelInstall(taskId)`）。
     *
     * 这里原来有个 `cancelInstall()`：它调用的是被列为「未实现」的命令，
     * 唯一效果是把 `installingId` 清空 —— 一个没人读的状态字段，纯 UI 假动作，
     * 后端那次安装其实照跑不误。已删除。
     */
    /**
     * 把后端抛出的「错误码」翻成人话。
     *
     * 后端很多失败用的是 `INSTANCE_NOT_INSTALLED` 这类码，直接 message.error(String(e))
     * 用户只会看到一串英文常量。所有启动入口都走这里，翻译一次到处受益。
     */
    async launch(id: string, world?: string, server?: string, opts?: { force?: boolean }) {
      // ── 启动前拦一道：渲染器和版本推荐不一致时先问一句 ──────────────────
      // 这是**秒级**判断，不用等游戏跑到一半再发现渲染器不行（1.8.9 + MG 会白屏卡住，
      // 等系统把 Activity 收掉要几十秒）。点「仍用 xxx 启动」会记住，不再重复问。
      if (!opts?.force) {
        const guard = await this.rendererGuard(id);
        if (guard) {
          this.pendingRendererGuard = { ...guard, world, server };
          return null;
        }
      }
      this.launchingId = id;
      try {
        const res = await api.launchInstance(id, world, server);
        return res;
      } catch (e) {
        const raw = String(e);
        if (raw.includes("INSTANCE_NOT_INSTALLED")) {
          throw new Error("这个实例还没安装游戏文件，请先在实例页点「安装游戏本体」");
        }
        if (raw.includes("GAME_ALREADY_RUNNING")) {
          throw new Error("游戏已经在运行了");
        }
        if (raw.includes("INSTANCE_CONFIG_CORRUPT")) {
          throw new Error("实例配置损坏（instance.json 读不出来），建议删除后重新创建");
        }
        throw e;
      } finally {
        this.launchingId = null;
      }
    },
    async stop() {
      await api.stopGame();
    },

    /** 读全局渲染器（存在安卓 SharedPreferences 里，只有后端读得到），带缓存。 */
    async ensureGlobalRenderer(): Promise<string> {
      if (this.globalRenderer) return this.globalRenderer;
      try {
        const p = await api.getPojavPrefs();
        const r = p?.renderer;
        this.globalRenderer = typeof r === "string" && r ? r : "opengles2";
      } catch {
        this.globalRenderer = "opengles2";
      }
      return this.globalRenderer ?? "opengles2";
    },

    /**
     * 启动前的渲染器判断：会用的 ≠ 版本推荐的 → 返回一份待确认信息（否则 null）。
     *
     * `auto`（缺省）不会触发 —— 它本身就是推荐值；只有「跟随全局」和手动指定才可能跑偏。
     */
    async rendererGuard(id: string): Promise<RendererGuard | null> {
      let inst = this.get(id);
      // 冷启动时列表可能还没加载（例如从实例详情页点启动），先拉一次再判断
      if (!inst) {
        await this.load(true);
        inst = this.get(id);
      }
      if (!inst) return null;
      const key = inst.renderer ?? "auto";
      if (key === "auto" || key === "") return null;
      const globalKey = key === "global" ? await this.ensureGlobalRenderer() : null;
      const used = effectiveRendererKey(key, inst.mc_version, globalKey);
      const recommended = autoRendererFor(inst.mc_version);
      if (used === recommended) return null;
      if (readFlag(rendererGuardKey(id, used))) return null;
      return {
        instanceId: id,
        mcVersion: inst.mc_version,
        used,
        usedName: rendererLabel(used),
        recommended,
        recommendedName: rendererLabel(recommended),
      };
    },
  },
});
