import { defineStore } from "pinia";
import { api } from "../api";
import type { ServerConfig } from "../types";

export const useServersStore = defineStore("servers", {
  state: () => ({
    servers: [] as ServerConfig[],
    loaded: false,
    runningIds: {} as Record<string, boolean>,
    /** 空闲休眠中的服务器：JVM 已停，但端口被唤醒监听占着，有人连接会自动启动 */
    sleepingIds: {} as Record<string, boolean>,
    // 由标题栏的“创建服务器”按钮递增，多人游戏页监听后打开创建对话框
    createRequest: 0,
    // 仅在“服务器”标签页时标题栏才显示创建按钮
    canCreate: true,
    /** 最近一次成功拉取的时间戳，用于短 TTL 内跳过重复请求 */
    lastLoadedAt: 0,
  }),
  getters: {
    count: (state) => (state.servers ?? []).length,
    byId: (state) => (id: string) => (state.servers ?? []).find((s) => s.id === id) ?? null,
    isRunning: (state) => (id: string) => !!state.runningIds[id],
    isSleeping: (state) => (id: string) => !!state.sleepingIds[id],
  },
  actions: {
    /** 后台静默刷新服务器列表 + 运行状态，失败保留旧数据 */
    async refresh() {
      try {
        // 后端未实现该命令时会返回 undefined，必须兜底成空数组
        this.servers = (await api.listHostedServers()) ?? [];
        this.loaded = true;
        const next: Record<string, boolean> = {};
        const sleeping: Record<string, boolean> = {};
        await Promise.all(
          this.servers.map(async (s) => {
            try {
              // 一次调用同时拿「在跑」与「休眠」。分两次问（isHostedServerRunning +
              // 单独的休眠查询）中间隔着一次休眠/唤醒，会拿到自相矛盾的组合 ——
              // 界面就会同时显示「休眠中」和「运行中」。
              const rt = await api.hostedServerRuntime(s.id);
              next[s.id] = !!rt?.running;
              sleeping[s.id] = !!rt?.sleeping;
            } catch {
              next[s.id] = false;
              sleeping[s.id] = false;
            }
          }),
        );
        this.runningIds = next;
        this.sleepingIds = sleeping;
        this.lastLoadedAt = Date.now();
      } catch {
        /* ignore */
      }
    },
    /**
     * 拉取服务器列表与运行状态。
     * - 已有数据时采用 stale-while-revalidate：立即返回旧数据，后台静默刷新替换。
     * - `force=true` 时始终前台拉取（用于创建/删除/启停后强制刷新）。
     */
    async load(force = false) {
      const now = Date.now();
      if (!force && this.lastLoadedAt && now - this.lastLoadedAt < 3000) return;
      const hasData = this.servers.length > 0;
      if (hasData && !force) {
        void this.refresh();
        return;
      }
      await this.refresh();
    },
    async create(name: string, core: string, mcVersion: string): Promise<ServerConfig> {
      const s = await api.createHostedServer(name, core, mcVersion);
      this.servers = [...this.servers, s];
      return s;
    },
    async update(patch: Record<string, unknown>): Promise<ServerConfig> {
      const s = await api.updateHostedServer(patch);
      const idx = this.servers.findIndex((x) => x.id === s.id);
      if (idx >= 0) this.servers[idx] = s;
      return s;
    },
    async remove(id: string) {
      await api.deleteHostedServer(id);
      this.servers = this.servers.filter((s) => s.id !== id);
    },
    async installCore(id: string) {
      await api.installHostedServerCore(id);
    },
    async start(id: string): Promise<number> {
      const { pid } = await api.startHostedServer(id);
      // 启动即离开休眠态：唤醒走的也是这条路径
      this.runningIds = { ...this.runningIds, [id]: true };
      this.setSleeping(id, false);
      const idx = this.servers.findIndex((s) => s.id === id);
      if (idx >= 0) this.servers[idx] = { ...this.servers[idx], last_started: Date.now() };
      return pid;
    },
    async stop(id: string): Promise<string> {
      // Rust 侧停服会区分「优雅停服（世界已存盘）」与「强制停止」，
      // 这句话要透传给 UI 展示，不能在 store 里吞掉。
      const note = await api.stopHostedServer(id);
      this.runningIds = { ...this.runningIds, [id]: false };
      // 手动停止是「真的停」：休眠是无人时自动进入的状态，用户主动停掉的服务器
      // 界面上不该显示成「休眠」（否则他以为有人连就会自己起来）。
      this.setSleeping(id, false);
      return note;
    },
    setRunning(id: string, running: boolean) {
      this.runningIds = { ...this.runningIds, [id]: running };
    },
    setSleeping(id: string, sleeping: boolean) {
      this.sleepingIds = { ...this.sleepingIds, [id]: sleeping };
    },
    requestCreate() {
      this.createRequest++;
    },
    setCanCreate(v: boolean) {
      this.canCreate = v;
    },
  },
});
