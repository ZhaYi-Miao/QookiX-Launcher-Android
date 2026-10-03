<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useMessage } from "../composables/message";
import { useRouter } from "vue-router";
import { useServersStore } from "../stores/servers";
import { useAccountsStore } from "../stores/accounts";
import { api } from "../api";
import { Button as VanButton, Empty as VanEmpty } from "vant";
import { IconPlus } from "../components/icons";
import AppPopup from "../ui/AppPopup.vue";
import AppInput from "../ui/AppInput.vue";

const servers = useServersStore();
const accounts = useAccountsStore();
const message = useMessage();
const router = useRouter();

/** 进入服务器详情页（控制面板：状态 / 日志 / 控制台 / 联机地址 / 参数） */
function openDetail(id: string) {
  void router.push(`/multiplayer/${id}`);
}
const showCreate = ref(false);const form = ref({ name: "", core: "paper", mcVersion: "" });
const saving = ref(false);
const starting = ref("");

async function createServer() {
  if (!form.value.name.trim() || !form.value.mcVersion) return;
  saving.value = true;
  try {
    const s = await servers.create(form.value.name.trim(), form.value.core, form.value.mcVersion);
    showCreate.value = false;
    message.success($t("downloads.finished"));
    void servers.installCore(s.id);
  } catch (e) {
    message.error(String(e));
  } finally {
    saving.value = false;
  }
}

async function toggleRun(id: string) {
  starting.value = id;
  try {
    if (servers.isRunning(id)) await servers.stop(id);
    else await servers.start(id);
  } catch (e) {
    message.error(String(e));
  } finally {
    starting.value = "";
  }
}


// ── 陶瓦联机（Terracotta）──────────────────────────────────────────────
// 隧道跑在独立进程 :tunnel（libterracotta.so 只在那里加载），这里只通过 HTTP 调它，
// 启动器不链接该库 —— 满足上游 AGPL 例外条款，保持 GPL-3.0（署名见页面底部）。
const tcState = ref<"off" | "ready" | "hosting" | "guesting">("off");
const tcRoom = ref("");
const tcRoomInput = ref("");
const tcBusy = ref(false);
let tcTimer: ReturnType<typeof setInterval> | null = null;

const tcHint = computed(() => {
  if (tcState.value === "off") return $t("terracotta.hint-off");
  if (tcState.value === "ready") return $t("terracotta.hint-ready");
  if (tcState.value === "guesting") return $t("terracotta.hint-guesting");
  return tcRoom.value ? $t("terracotta.hint-hosting") : $t("terracotta.hint-scanning");
});

/** 拉一次隧道状态。Terracotta 返回形如 {"index":1,"state":"host-scanning"} */
async function tcRefresh() {
  try {
    const raw = await api.terracottaTunnelRequest("/state");
    const s = JSON.parse(raw) as { state?: string; room?: string; code?: string };
    const st = s.state ?? "";
    if (st.includes("host")) {
      tcState.value = "hosting";
      tcRoom.value = String(s.room ?? s.code ?? "");
    } else if (st.includes("guest")) {
      tcState.value = "guesting";
    } else {
      tcState.value = "ready";
      tcRoom.value = "";
    }
  } catch {
    tcState.value = "off";
    tcRoom.value = "";
  }
}

async function startHosting() {
  tcBusy.value = true;
  try {
    const name = accounts.current?.username ?? "player";
    await api.terracottaTunnelRequest("/host?player=" + encodeURIComponent(name));
    for (let i = 0; i < 8; i++) {
      await new Promise((r) => setTimeout(r, 1500));
      await tcRefresh();
      if (tcRoom.value) break;
    }
    message.success(tcRoom.value ? $t("terracotta.hosted") : $t("terracotta.waiting-world"));
  } catch (e) {
    message.error(String(e));
  } finally {
    tcBusy.value = false;
  }
}

async function joinRoom() {
  const code = tcRoomInput.value.trim();
  if (!code) return;
  tcBusy.value = true;
  try {
    const name = accounts.current?.username ?? "player";
    const raw = await api.terracottaTunnelRequest(
      "/guest?room=" + encodeURIComponent(code) + "&player=" + encodeURIComponent(name),
    );
    if (raw.includes('"ok":true')) {
      message.success($t("terracotta.joining"));
      await tcRefresh();
    } else {
      message.error($t("terracotta.bad-room"));
    }
  } catch (e) {
    message.error(String(e));
  } finally {
    tcBusy.value = false;
  }
}

async function stopHosting() {
  try {
    await api.terracottaTunnelRequest("/waiting");
    tcRoom.value = "";
    await tcRefresh();
  } catch (e) {
    message.error(String(e));
  }
}

async function copyRoom() {
  try {
    await navigator.clipboard.writeText(tcRoom.value);
    message.success($t("terracotta.copied"));
  } catch {
    message.error($t("terracotta.copy-failed"));
  }
}

onMounted(async () => {
  void servers.load();
  setTimeout(async () => {
    await tcRefresh();
    tcTimer = setInterval(() => void tcRefresh(), 5000);
  }, 1500);
});

onBeforeUnmount(() => {
  if (tcTimer) clearInterval(tcTimer);
});
</script>

<template>
  <div class="mp">
    <!-- 手机：这个页面只有「本地服务器」一件事，原来那个「Minecraft 新闻」tab 里
         只有两句提示、没有真实内容（新闻已经有独立的页面），属于多余入口，去掉。 -->
    <div v-if="servers.servers.length" class="list">
      <div v-for="s in servers.servers" :key="s.id" class="srv glass">
        <!-- 整块可点进详情页（控制面板：状态 / 日志 / 控制台 / 联机地址 / 参数）。
             没有这个入口的话，服务器详情页只能靠手输路由到达，等于进不去。 -->
        <div class="srv-main" role="button" tabindex="0" @click="openDetail(s.id)" @keydown.enter="openDetail(s.id)">
          <div class="srv-name">{{ s.name }}</div>
          <div class="srv-meta">{{ s.core }} · {{ s.mc_version }} · :{{ s.port }}</div>
        </div>
        <van-button size="small" :type="servers.isRunning(s.id) ? 'default' : 'primary'" :loading="starting === s.id" @click="toggleRun(s.id)">
          {{ servers.isRunning(s.id) ? $t("multiplayer.stop") : $t("instance-card.launch") }}
        </van-button>
        <van-button size="small" @click="servers.remove(s.id)">{{ $t("common.delete") }}</van-button>
      </div>
    </div>
    <van-empty v-else :description="$t('instance-saves.no-servers')">
      <van-button type="primary" @click="showCreate = true"><IconPlus /> {{ $t("title-bar.new-server") }}</van-button>
    </van-empty>

    <!-- ── 陶瓦联机（Terracotta）：和朋友异地联机 ──────────────────────
         .so 跑在独立进程 :tunnel，主进程只通过 localhost HTTP 通信（不链接它，
         这样启动器保持 GPL-3.0，署名见本页底部与设置→第三方声明）。 -->
    <section class="tc glass">
      <div class="tc-head">
        <span class="tc-title">{{ $t("terracotta.title") }}</span>
        <span class="tc-dot" :class="tcState"></span>
      </div>
      <p class="tc-hint">{{ tcHint }}</p>

      <template v-if="tcRoom">
        <div class="tc-room">
          <span class="tc-room-label">{{ $t("terracotta.room-code") }}</span>
          <span class="tc-room-code">{{ tcRoom }}</span>
        </div>
        <div class="tc-actions">
          <van-button size="small" @click="copyRoom">{{ $t("terracotta.copy") }}</van-button>
          <van-button size="small" @click="stopHosting">{{ $t("terracotta.stop") }}</van-button>
        </div>
      </template>

      <template v-else>
        <div class="tc-actions">
          <van-button size="small" type="primary" :loading="tcBusy" @click="startHosting">
            {{ $t("terracotta.host") }}
          </van-button>
        </div>
        <div class="tc-join">
          <app-input
            v-model:value="tcRoomInput"
            :placeholder="$t('terracotta.room-placeholder')"
            maxlength="12"
          />
          <van-button size="small" :loading="tcBusy" @click="joinRoom">{{ $t("terracotta.join") }}</van-button>
        </div>
      </template>

      <p class="tc-credit">
        {{ $t("terracotta.credit") }}
      </p>
    </section>

    <app-popup :show="showCreate" position="bottom" round @update:show="(v: boolean) => (showCreate = v)">
      <div class="sheet">
        <div class="sheet-title">{{ $t("title-bar.new-server") }}</div>
        <app-input v-model:value="form.name" :placeholder="$t('multiplayer.server-name-hint')" maxlength="40" />
        <app-input v-model:value="form.mcVersion" :placeholder="$t('create-instance.select-version')" maxlength="16" />
        <van-button block type="primary" :loading="saving" @click="createServer">{{ $t("multiplayer.create") }}</van-button>
      </div>
    </app-popup>
  </div>
</template>

<style scoped>
.mp {
  padding: 4px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.srv {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
}
.srv-main {
  flex: 1;
  min-width: 0;
  /* 可点进详情：加按压反馈，否则用户不知道这块能点 */
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
}
.srv-main:active {
  opacity: 0.6;
}
.srv-name {
  font-weight: 600;
  font-size: 14px;
}
.srv-meta {
  font-size: 12px;
  color: var(--text-3);
}
.hint {
  margin: 0 0 8px;
  font-size: 13px;
  color: var(--text-3);
}
.online {
  padding: 20px 16px;
}
.sheet {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  padding-bottom: calc(16px + env(safe-area-inset-bottom, 0px));
}
.sheet-title {
  font-size: 16px;
  font-weight: 600;
}
/* ── 陶瓦联机卡片 ───────────────────────────────────────────────── */
.tc {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
}
.tc-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.tc-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--text-1);
}
.tc-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--text-3);
  flex-shrink: 0;
}
.tc-dot.ready { background: #4ecdc4; }
.tc-dot.hosting { background: var(--accent); }
.tc-dot.guesting { background: #ffd166; }
.tc-hint {
  margin: 0;
  font-size: 13px;
  color: var(--text-3);
  line-height: 1.5;
}
.tc-room {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 12px 14px;
  border-radius: 12px;
  background: var(--accent-soft);
  border: 1px solid var(--accent);
}
.tc-room-label {
  font-size: 13px;
  color: var(--text-2);
}
.tc-room-code {
  font-size: 22px;
  font-weight: 700;
  color: var(--accent);
  letter-spacing: 2px;
}
.tc-actions {
  display: flex;
  gap: 8px;
}
.tc-actions :deep(.van-button) {
  flex: 1;
  min-height: 40px;
}
.tc-join {
  display: flex;
  gap: 8px;
  align-items: center;
}
.tc-join > :first-child {
  flex: 1;
  min-width: 0;
}
.tc-join :deep(.van-button) {
  min-height: 44px;
  padding: 0 16px;
}
/* 署名：AGPL 例外条款要求在界面明显处标识版权 */
.tc-credit {
  margin: 0;
  font-size: 11px;
  color: var(--text-3);
  line-height: 1.5;
}
</style>
