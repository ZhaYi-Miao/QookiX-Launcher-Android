<script setup lang="ts">
/**
 * 顶部标题栏：显示当前页面 + 页面级动作按钮。
 *
 * 桌面的这些按钮（新建实例 / 新建分组 / 创建服务器 / 清除已完成）原来都挂在标题栏上，
 * 安卓端早期把标题栏整个删掉后这些入口就全丢了，所以这里做一个不含窗口控件的版本：
 * 页面动作照旧由路由 meta 与各 store 提供，标题栏只负责呈现。
 */
import { computed, inject } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  IconHome,
  IconCompass,
  IconDownload,
  IconGrid,
  IconNewspaper,
  IconPlus,
  IconSettings,
  IconUser,
  IconUsers,
  IconTrash,
} from "./icons";
import { useTasksStore } from "../stores/tasks";
import { useServersStore } from "../stores/servers";

const route = useRoute();
const router = useRouter();
const tasks = useTasksStore();
const servers = useServersStore();

/** 托管服务器后端未实现（`create_hosted_server` 等在 api.ts 里标注为未实现）。 */
const HOSTED_SERVER_ENABLED = false;

const pageIcons: Record<string, any> = {
  home: IconHome,
  compass: IconCompass,
  download: IconDownload,
  grid: IconGrid,
  newspaper: IconNewspaper,
  plus: IconPlus,
  settings: IconSettings,
  user: IconUser,
  users: IconUsers,
};
const pageIcon = computed(() => pageIcons[(route.meta.icon as string) ?? ""] ?? IconHome);

const actionIcons: Record<string, any> = {
  plus: IconPlus,
  trash: IconTrash,
};

/** 路由 meta 中配置的导航按钮（如实例页的「新建实例」） */
const pageAction = computed(
  () => (route.meta.action as { text?: string; icon?: string; to?: string }) ?? null
);

/** 供实例页触发的「新建分组」信号（由 App 提供） */
const groupDialogRequest = inject<{ value: number }>("groupDialogRequest", { value: 0 });
function requestCreateGroup() {
  groupDialogRequest.value++;
}

const finishedCount = computed(() => tasks.taskList.filter((t) => t.finished).length);
</script>

<template>
  <div class="titlebar">
    <div class="tb-left">
      <img src="/app-icon.png" class="tb-logo" alt="" />
      <span class="tb-title">QookiX Launcher</span>
      <span class="tb-divider">/</span>
      <component :is="pageIcon" class="tb-page-icon" />
      <span class="tb-page">{{ (route.meta.title as string) ?? "" }}</span>
    </div>
    <!--
      上下文区：当前视图把「属于这一页的标题与操作」Teleport 到这里。
      实例详情就是靠它把整张实例卡片（名称/版本/启动游戏/次要操作）搬进标题栏的 ——
      横屏手机上纵向最紧张，这样能省掉整张卡片约 78px。
    -->
    <div id="tb-context" class="tb-context"></div>
    <div class="tb-actions">
      <button v-if="route.name === 'instances'" class="tb-action" @click="requestCreateGroup">
        <IconPlus class="tb-action-icon" /> 新建分组
      </button>
      <!-- 「创建服务器」依赖后端 `create_hosted_server` 系列命令，这些命令在
           api.ts 里整批标注为**未实现**（Rust 侧零实现）。先隐藏入口，
           功能补齐后把 HOSTED_SERVER_ENABLED 打开即可恢复。 -->
      <button
        v-if="HOSTED_SERVER_ENABLED && route.name === 'multiplayer' && servers.canCreate"
        class="tb-action primary"
        @click="servers.requestCreate()"
      >
        <IconPlus class="tb-action-icon" /> 创建服务器
      </button>
      <button v-if="pageAction?.to" class="tb-action primary" @click="router.push(pageAction.to)">
        <component :is="actionIcons[pageAction.icon ?? '']" class="tb-action-icon" />
        {{ pageAction.text }}
      </button>
      <button
        v-if="route.name === 'downloads'"
        class="tb-action"
        :disabled="!finishedCount"
        @click="tasks.clearFinished()"
      >
        <IconTrash class="tb-action-icon" /> 清除已完成
      </button>
    </div>
  </div>
</template>

<style scoped>
.titlebar {
  /* 挖孔/刘海：横屏时两侧（--safe-l/--safe-r）、竖屏时顶部（--safe-t）都要避开。
     变量在 App.vue 的 .app 上定义；桌面端全是 0，行为不变。
     height 改 min-height：竖屏有顶部挖孔时标题栏整体下移一点，
     横屏（safe-t=0）仍然是 40px，与 MobileNav 左栏的 top:40px 对齐。 */
  min-height: 40px;
  padding-top: var(--safe-t, 0px);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding-left: calc(12px + var(--tb-inset-l, var(--safe-l, 0px)));
  padding-right: calc(12px + var(--safe-r, 0px));
  background: color-mix(in srgb, var(--bg-0) 86%, transparent);
  backdrop-filter: blur(var(--glass-blur, 8px));
  -webkit-backdrop-filter: blur(var(--glass-blur, 8px));
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
::global(.has-bg) .titlebar {
  background: var(--panel);
}
.tb-left {
  display: flex;
  align-items: center;
  gap: 9px;
  min-width: 0;
}
.tb-context {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
  min-width: 0;
  overflow: hidden;
}
.tb-context:empty {
  flex: 0;
}
.tb-logo {
  width: 20px;
  height: 20px;
  border-radius: 6px;
  flex-shrink: 0;
}
.tb-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
  letter-spacing: 0.2px;
  white-space: nowrap;
}
.tb-divider {
  color: var(--text-3);
  font-size: 13px;
}
.tb-page-icon {
  width: 14px;
  height: 14px;
  color: var(--text-3);
  flex-shrink: 0;
}
.tb-page {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.tb-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.tb-account {
  display: none;
  flex-shrink: 0;
}
.tb-action {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.05);
  color: var(--text-2);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  white-space: nowrap;
  transition: all 0.15s;
}
.tb-action:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-1);
}
.tb-action:disabled {
  opacity: 0.4;
  cursor: default;
}
.tb-action.primary {
  border: none;
  background: linear-gradient(135deg, var(--accent), var(--accent-deep));
  color: #1a1208;
  box-shadow: 0 4px 16px var(--accent-03);
}
.tb-action.primary:hover:not(:disabled) {
  filter: brightness(1.08);
}
.tb-action-icon {
  width: 14px;
  height: 14px;
}

/* 窄屏竖屏：让位给页面标题与动作按钮，隐藏品牌信息，并把账号入口挪到这里 */
@media (max-width: 1100px), (pointer: coarse) {
  .tb-logo,
  .tb-title,
  .tb-divider {
    display: none;
  }
  .tb-action {
    padding: 0 10px;
  }
  .tb-account {
    display: flex;
  }
  .tb-account :deep(.acct-chip) {
    height: 30px;
    padding: 0 6px;
    gap: 6px;
    border: none;
    background: transparent;
  }
  .tb-account :deep(.acct-info) {
    display: none;
  }
  .tb-account :deep(.chev) {
    display: none;
  }
}
</style>
