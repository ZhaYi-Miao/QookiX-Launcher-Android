<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { NButton, useMessage } from "naive-ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { api } from "../api";
import { fmtDateStr as fmtDate, instanceLabel } from "../utils/format";
import { useInstancesStore } from "../stores/instances";
import { useSettingsStore } from "../stores/settings";
import { useSlidingIndicator } from "../composables/useSlidingIndicator";
import { IconCopy, IconExternal, IconGlobe } from "./icons";
import type { ProjectDependency, ProjectHit, ProjectVersion } from "../types";
import AppSheet from "../ui/AppSheet.vue";
import AppInput from "../ui/AppInput.vue";
import AppSelect from "../ui/AppSelect.vue";

const props = defineProps<{
  show: boolean;
  project: ProjectHit | null;
  defaultInstance?: string | null;
}>();
const emit = defineEmits<{
  "update:show": [v: boolean];
  "install-dep": [dep: ProjectDependency];
}>();

// 点击弹窗卡片外部即关闭（用 document 委托，不依赖 naive-ui 的 mask 机制）。
// 用 mousedown 而非 click：打开弹窗的那次按下发生在 show 变 true 之前，
// 会被 `!props.show` 拦截，避免弹窗刚打开就被自身触发的点击冒泡关掉。
const cardRef = ref<HTMLElement | null>(null);
function onDocMouseDown(e: MouseEvent) {
  if (!props.show) return;
  const t = e.target as Element | null;
  if (!t) return;
  // 点击弹窗卡片内部、或 naive-ui 的下拉/弹层（teleport 到 body）都不应关闭
  if (cardRef.value?.contains(t)) return;
  if (t.closest(".v-binder-follower-container, .n-base-select-menu, .n-popover, .n-dropdown")) return;
  emit("update:show", false);
}
onMounted(() => document.addEventListener("mousedown", onDocMouseDown));
onBeforeUnmount(() => document.removeEventListener("mousedown", onDocMouseDown));

const instances = useInstancesStore();
const router = useRouter();
const message = useMessage();

async function copyName() {
  if (!props.project?.title) return;
  try {
    await navigator.clipboard.writeText(props.project.title);
    message.success($t("install-dialog.name-copied"));
  } catch {
    message.error($t("crash-analyzer.copy-failed"));
  }
}

const versions = ref<ProjectVersion[]>([]);
const loadingVersions = ref(false);
const selectedVersion = ref<string | null>(null);
const selectedInstance = ref<string | null>(null);
const installing = ref(false);
const installMsg = ref("");
const typeFilter = ref<"all" | "release" | "beta" | "alpha">("all");

// 版本类型 tabs 的滑动高亮指示器
const typeTabBox = ref<HTMLElement | null>(null);
const { indicatorStyle: typeTabIndicatorStyle, refresh: refreshTypeTabIndicator } = useSlidingIndicator(
  typeTabBox,
  () => Array.from(typeTabBox.value?.querySelectorAll<HTMLElement>(".id-type-tabs button") ?? []),
  () => ["all", "release", "beta", "alpha"].indexOf(typeFilter.value),
  { axis: "horizontal" }
);
watch(typeFilter, () => nextTick(() => refreshTypeTabIndicator()));
const deps = ref<ProjectDependency[]>([]);
const loadingDeps = ref(false);

// ---- 翻译 ----
const settingsStore = useSettingsStore();
const translateService = computed(() => settingsStore.settings?.translate_provider ?? "default");
/// 内置服务只提供 Modrinth 的译文；自定义接口两个来源都能翻；百度网页模式走浏览器。
const descTranslatable = computed(() => {
  if (!props.project) return false;
  const svc = translateService.value;
  if (svc === "baidu_web") return false;
  return svc === "custom" || props.project.provider === "modrinth";
});

const descZh = ref<string | null>(null);
const descLoading = ref(false);

function openBaiduTranslate() {
  if (!props.project) return;
  const q = encodeURIComponent(props.project.description || props.project.title);
  openUrl(`https://fanyi.baidu.com/mtpe-individual/transText?query=${q}&lang=en2zh`).catch(() =>
    message.error($t("install-dialog.open-browser-failed"))
  );
}

async function loadDescTranslation() {
  descZh.value = null;
  if (!props.show || !descTranslatable.value || !props.project) return;
  descLoading.value = true;
  try {
    const r = await api.translateModDescriptions(props.project.provider, [props.project.id]);
    descZh.value = r.translations[props.project.id] ?? null;
  } catch {
    // 翻译失败就退化成只显示原文，不打扰用户
    descZh.value = null;
  } finally {
    descLoading.value = false;
  }
}

// 反馈：过时（服务端重译后重新取一次） / 质量问题（选类型 + 可填建议）
const feedbackPanel = ref(false);
const feedbackMode = ref<"choose" | "quality">("choose");
const issueType = ref("wrong_translation");
const userSuggestion = ref("");
const userComment = ref("");
const submittingFeedback = ref(false);
const issueTypes = [
  { value: "wrong_translation", label: $t("install-dialog.wrong-translation") },
  { value: "unnatural", label: $t("install-dialog.unnatural") },
  { value: "missing", label: $t("install-dialog.missing") },
  { value: "other", label: $t("install-dialog.other") },
];

async function reportStale() {
  if (!props.project) return;
  try {
    const status = await api.reportStaleTranslation(props.project.provider, props.project.slug);
    message.success($t("install-dialog.feedback-sent"));
    feedbackPanel.value = false;
    if (status === "updated") await loadDescTranslation();
  } catch (e) {
    message.error(String(e));
  }
}

async function submitQuality() {
  if (!props.project || submittingFeedback.value) return;
  submittingFeedback.value = true;
  try {
    await api.reportTranslationQuality(
      props.project.provider,
      props.project.slug,
      issueType.value,
      userSuggestion.value,
      userComment.value
    );
    message.success($t("install-dialog.feedback-sent"));
    feedbackPanel.value = false;
    feedbackMode.value = "choose";
    userSuggestion.value = "";
    userComment.value = "";
  } catch (e) {
    message.error(String(e));
  } finally {
    submittingFeedback.value = false;
  }
}

// 正文：默认收起，展开时才请求（不展开就不会白拉一次项目详情）
const bodyOpen = ref(false);
const bodyLoading = ref(false);
const bodyZh = ref<string | null>(null);
const bodyOriginal = ref("");
const bodyNote = ref("");
const showZhBody = ref(false);
const bodyTranslatable = computed(() => ["default", "custom"].includes(translateService.value));

function renderMarkdown(src: string): string {
  return DOMPurify.sanitize(marked.parse(src, { async: false }) as string);
}

async function loadBody(translate: boolean) {
  if (!props.project) return;
  bodyLoading.value = true;
  bodyNote.value = "";
  try {
    const r = await api.translateProjectBody(
      props.project.provider,
      props.project.slug || props.project.id,
      translate
    );
    bodyOriginal.value = r.original ?? "";
    bodyZh.value = r.body ?? null;
    showZhBody.value = !!r.body;
    if (r.error) bodyNote.value = r.error;
    else if (!r.supported) bodyNote.value = $t("install-dialog.body-unsupported");
  } catch (e) {
    bodyNote.value = String(e);
  } finally {
    bodyLoading.value = false;
  }
}

async function toggleBody() {
  bodyOpen.value = !bodyOpen.value;
  if (!bodyOpen.value || bodyOriginal.value || bodyLoading.value) return;
  const auto = !!settingsStore.settings?.body_translate_auto;
  await loadBody(auto && bodyTranslatable.value);
}

async function translateBodyNow() {
  if (bodyZh.value) {
    showZhBody.value = !showZhBody.value;
    return;
  }
  if (!bodyTranslatable.value) {
    message.info($t("install-dialog.body-unsupported"));
    return;
  }
  await loadBody(true);
}

const isModpack = computed(() => props.project?.project_type === "modpack");

const mcWikiUrl = ref("");
const sourceUrl = computed(() => {
  const p = props.project;
  if (!p) return "";
  if (p.provider === "modrinth") {
    return `https://modrinth.com/${p.project_type}/${p.slug}`;
  }
  if (p.provider === "curseforge") {
    const kind = p.project_type === "modpack" ? "modpacks"
      : p.project_type === "resourcepack" ? "texture-packs"
      : p.project_type === "shader" ? "shaders"
      : "mc-mods";
    return `https://www.curseforge.com/minecraft/${kind}/${p.slug}`;
  }
  return "";
});

async function loadMcWikiUrl() {
  if (!props.project) return;
  try {
    mcWikiUrl.value = await api.mcWikiUrl(props.project.title, props.project.slug, props.project.provider);
  } catch {
    mcWikiUrl.value = "";
  }
}

const instanceOptions = () =>
  instances.instances.filter((i) => i.loader !== "vanilla").map((i) => ({
    label: instanceLabel(i),
    value: i.id,
  }));

function versionType(v: ProjectVersion): string {
  // modrinth: version_type; curseforge: release_type (1=release,2=beta,3=alpha)
  if (v.version_type) return v.version_type;
  if (v.release_type === 2) return "beta";
  if (v.release_type === 3) return "alpha";
  return "release";
}

function typeLabel(t: string) {
  return { release: $t("install-dialog.release"), beta: $t("install-dialog.beta"), alpha: $t("install-dialog.alpha") }[t] ?? $t("install-dialog.release");
}

const filteredVersions = computed(() => {
  if (typeFilter.value === "all") return versions.value;
  return versions.value.filter((v) => versionType(v) === typeFilter.value);
});

async function loadDeps() {
  deps.value = [];
  if (!props.project || !selectedVersion.value) return;
  loadingDeps.value = true;
  try {
    deps.value = await api.projectDependencies(props.project.provider, props.project.id, selectedVersion.value);
  } catch {
    deps.value = [];
  } finally {
    loadingDeps.value = false;
  }
}

watch(selectedVersion, () => { if (!isModpack.value) loadDeps(); });

async function loadVersions() {
  if (!props.project) return;
  loadingVersions.value = true;
  selectedVersion.value = null;
  deps.value = [];
  try {
    // 已选实例时用实例的 MC 版本 + 加载器请求上游，让 Modrinth/CurseForge
    // 只返回兼容的版本，从而 API 本身返回更少的数据（而非全量拉取后本地过滤）
    const inst = selectedInstance.value ? instances.get(selectedInstance.value) : null;
    const mc = inst?.mc_version ?? "";
    const ld = inst && inst.loader !== "vanilla" ? inst.loader : "";
    const res = await api.projectVersions(props.project.provider, props.project.id, mc, ld);
    versions.value = res.versions;
    if (inst && !res.versions.length) {
      // 上游按实例筛选无结果时，回退拉取全部版本供选择
      const all = await api.projectVersions(props.project.provider, props.project.id, "", "");
      versions.value = all.versions;
    }
    const picked = versions.value.find((v) => versionType(v) === "release") ?? versions.value[0];
    if (picked) selectedVersion.value = picked.id;
  } catch (e) {
    message.error(String(e));
  } finally {
    loadingVersions.value = false;
  }
}

// 切换实例时按新实例的 MC 版本/加载器重新拉取
watch(selectedInstance, () => {
  if (props.show && props.project) loadVersions();
});

function resetForProject() {
  if (!props.project) return;
  const nonVanilla = instances.instances.filter((i) => i.loader !== "vanilla");
  const pref = props.defaultInstance && nonVanilla.some((i) => i.id === props.defaultInstance)
    ? props.defaultInstance
    : nonVanilla[0]?.id ?? null;
  selectedInstance.value = isModpack.value ? null : pref;
  installMsg.value = "";
  typeFilter.value = "all";
  // 换项目时把翻译状态清干净，免得看到上一个项目的译文
  descZh.value = null;
  descLoading.value = false;
  feedbackPanel.value = false;
  feedbackMode.value = "choose";
  bodyOpen.value = false;
  bodyZh.value = null;
  bodyOriginal.value = "";
  bodyNote.value = "";
  showZhBody.value = false;
  loadVersions();
  loadMcWikiUrl();
  loadDescTranslation();
}

async function onOpen() {
  if (!props.show) return;
  api.logDebug(
    `[fe] 对话框打开 project=${props.project?.provider}/${props.project?.id} type=${props.project?.project_type}`
  );
  resetForProject();
}

// 弹窗内切换项目（点击前置依赖）时重新加载版本与依赖
watch(
  () => props.project,
  (proj, oldProj) => {
    if (proj && oldProj && proj !== oldProj) resetForProject();
  }
);

function depLabel(t: string) {
  return { required: $t("install-dialog.required"), optional: $t("install-dialog.optional"), incompatible: $t("install-dialog.incompatible"), embedded: $t("install-dialog.embedded") }[t] ?? t;
}

async function install() {
  if (!props.project) return;
  api.logDebug(
    `[fe] install() 进入 provider=${props.project.provider} id=${props.project.id} type=${props.project.project_type} isModpack=${isModpack.value} versions=${versions.value.length}`
  );
  if (!isModpack.value && !selectedInstance.value) {
    api.logDebug("[fe] 拦截：未选择实例");
    message.warning($t("install-dialog.pick-instance"));
    return;
  }
  if (!selectedVersion.value) {
    api.logDebug(`[fe] 拦截：未选择版本 selectedVersion=${String(selectedVersion.value)}`);
    message.warning(versions.value.length ? $t("install-dialog.pick-version") : $t("install-dialog.no-compatible-version"));
    return;
  }
  api.logDebug(
    `[fe] 发起 invoke instanceId=${String(selectedInstance.value)} version=${String(selectedVersion.value)} kind=${props.project.project_type}`
  );
  installing.value = true;
  message.success(isModpack.value ? $t("install-dialog.install-started") : $t("install-dialog.queued"));
  // 整合包安装是长任务（下载整包 + 逐个拉取 mod 元数据 + 装游戏本体），
  // 不阻塞对话框——立即关闭，进度与成败都通过 install://progress 事件进下载中心。
  api
    .installContent(
      selectedInstance.value ?? "",
      props.project.provider,
      props.project.id,
      selectedVersion.value,
      props.project.project_type
    )
    .then((r) => {
      api.logDebug(`[fe] 返回成功 ${JSON.stringify(r)}`);
      if (!isModpack.value) message.success($t("install-dialog.computed"));
    })
    .catch((e) => {
      api.logDebug(`[fe] 返回失败 ${String(e)}`);
      message.error(String(e));
    })
    .finally(() => {
      installing.value = false;
    });
  emit("update:show", false);
}

</script>

<template>
  <app-sheet
    class="install-dialog-card"
    :show="props.show"
    :title="props.project?.title ?? $t('install-dialog.install-content')"
    :mask-closable="true"
    @update:show="(v: boolean) => emit('update:show', v)"
    @after-enter="onOpen"
  >
    <div v-if="props.project" ref="cardRef" class="id-modal">
      <div class="id-head">
        <img v-if="props.project.icon_url" :src="props.project.icon_url" class="id-icon" alt="" />
        <div class="id-info">
          <div class="id-title">{{ props.project.title }}</div>
          <div class="id-meta">
            <span class="id-author">{{ props.project.author }}</span>
            <span class="id-dl">{{ $t("install-dialog.downloads-count", { p1: (props.project.downloads / 10000).toFixed(1) }) }}</span>
            <span class="id-type">{{ props.project.project_type }}</span>
          </div>
          <div v-if="descLoading" class="id-desc">{{ $t("install-dialog.translating") }}</div>
          <template v-else-if="descZh">
            <div class="id-desc id-desc-zh">{{ descZh }}</div>
            <div class="id-desc id-desc-en">{{ props.project.description }}</div>
          </template>
          <div v-else class="id-desc">{{ props.project.description }}</div>
          <div class="id-links">
            <a v-if="mcWikiUrl" :href="mcWikiUrl" target="_blank" class="id-link"><IconGlobe />{{ $t("install-dialog.mc-wiki") }}</a>
            <a v-if="sourceUrl" :href="sourceUrl" target="_blank" class="id-link"><IconExternal />{{ $t("install-dialog.open-in-browser") }}</a>
            <button class="id-link" @click="copyName"><IconCopy />{{ $t("install-dialog.copy-name") }}</button>
            <button
              v-if="translateService === 'baidu_web'"
              class="id-link"
              @click="openBaiduTranslate"
            >{{ $t("install-dialog.baidu-translate") }}</button>
            <button
              v-else-if="descZh"
              class="id-link"
              :class="{ on: feedbackPanel }"
              @click="feedbackPanel = !feedbackPanel"
            >{{ $t("install-dialog.feedback") }}</button>
          </div>
          <div v-if="feedbackPanel" class="id-feedback">
            <template v-if="feedbackMode === 'choose'">
              <div class="id-feedback-title">{{ $t("install-dialog.feedback-what") }}</div>
              <div class="id-feedback-actions">
                <n-button size="small" @click="reportStale">{{ $t("install-dialog.feedback-outdated") }}</n-button>
                <n-button size="small" @click="feedbackMode = 'quality'">{{ $t("install-dialog.feedback-wrong") }}</n-button>
                <n-button size="small" @click="feedbackPanel = false">{{ $t("common.cancel") }}</n-button>
              </div>
            </template>
            <template v-else>
              <app-select v-model:value="issueType" :options="issueTypes" size="small" />
              <app-input v-model:value="userSuggestion" size="small" :placeholder="$t('install-dialog.feedback-suggestion')" />
              <app-input v-model:value="userComment" size="small" :placeholder="$t('install-dialog.feedback-note')" />
              <div class="id-feedback-actions">
                <n-button size="small" type="primary" :loading="submittingFeedback" @click="submitQuality">{{ $t("install-dialog.submit") }}</n-button>
                <n-button size="small" @click="feedbackMode = 'choose'">{{ $t("install-dialog.back") }}</n-button>
              </div>
            </template>
          </div>
        </div>
      </div>

      <div class="id-form">
        <label v-if="!isModpack" class="id-field">
          <span>{{ $t("install-dialog.install-to") }}</span>
          <app-select v-model:value="selectedInstance" :options="instanceOptions()" :placeholder="$t('install-dialog.select-instance')" />
        </label>

        <div v-if="isModpack" class="id-field">
          <span class="id-modpack-hint">{{ $t("install-dialog.modpack-new-instance") }}</span>
        </div>

        <div class="id-field">
          <div class="id-ver-head">
            <span>{{ $t("install-dialog.select-version") }}</span>
            <div ref="typeTabBox" class="id-type-tabs">
              <div class="indicator" :style="typeTabIndicatorStyle"></div>
              <button :class="{ active: typeFilter === 'all' }" @click="typeFilter = 'all'">{{ $t("install-dialog.all") }}</button>
              <button :class="{ active: typeFilter === 'release' }" @click="typeFilter = 'release'">{{ $t("install-dialog.release") }}</button>
              <button :class="{ active: typeFilter === 'beta' }" @click="typeFilter = 'beta'">{{ $t("install-dialog.beta") }}</button>
              <button :class="{ active: typeFilter === 'alpha' }" @click="typeFilter = 'alpha'">{{ $t("install-dialog.alpha") }}</button>
            </div>
          </div>
          <div v-if="loadingVersions" class="id-loading">{{ $t("file-manager.loading") }}</div>
          <div v-else class="id-ver-list">
            <button
              v-for="v in filteredVersions.slice(0, 80)"
              :key="v.id"
              class="id-ver-row"
              :class="{ active: selectedVersion === v.id }"
              @click="selectedVersion = v.id"
            >
              <span class="id-ver-num mono">{{ v.version_number }}</span>
              <span class="id-ver-type" :class="versionType(v)">{{ typeLabel(versionType(v)) }}</span>
              <span class="id-ver-mc">{{ (v.game_versions ?? []).slice(-2).join(", ") }}</span>
              <span class="id-ver-date">{{ fmtDate(v.date_published) }}</span>
            </button>
            <div v-if="!filteredVersions.length" class="id-empty">{{ $t("install-dialog.no-versions") }}</div>
          </div>
        </div>

        <!-- dependencies -->
        <div v-if="(deps.length || loadingDeps) && !isModpack" class="id-deps">
          <span class="id-deps-label">{{ $t("install-dialog.dependencies") }}</span>
          <div v-if="!loadingDeps" class="id-deps-list">
            <button
              v-for="d in deps"
              :key="d.projectId"
              class="id-dep-chip"
              :class="d.dependencyType"
              :title="$t('install-dialog.view-details', { p1: d.title })" :aria-label="$t('install-dialog.view-details', { p1: d.title })"
              @click="emit('install-dep', d)"
            >
              <span class="id-dep-tag">{{ depLabel(d.dependencyType) }}</span>
              {{ d.title }}
            </button>
          </div>
          <div v-if="loadingDeps" class="id-deps-loading">{{ $t("install-dialog.querying-deps") }}</div>
        </div>
      </div>

      <!-- 正文按需展开：不展开就不发请求 -->
      <div class="id-body">
        <div class="id-body-head">
          <button class="id-link" @click="toggleBody">{{ bodyOpen ? $t('install-dialog.collapse-body') : $t('install-dialog.show-body') }}</button>
          <button
            v-if="bodyOpen"
            class="id-link"
            :class="{ on: showZhBody }"
            :disabled="bodyLoading"
            @click="translateBodyNow"
          >
            {{ bodyLoading ? $t('file-manager.loading') : bodyZh ? (showZhBody ? $t('install-dialog.show-source') : $t('install-dialog.show-translation')) : $t('install-dialog.translate-body') }}
          </button>
        </div>
        <div v-if="bodyOpen" class="id-body-box">
          <div v-if="bodyLoading" class="id-loading">{{ $t("file-manager.loading") }}</div>
          <template v-else>
            <p v-if="bodyNote" class="id-body-note">{{ bodyNote }}</p>
            <div
              v-if="(showZhBody ? bodyZh : bodyOriginal)"
              class="id-body-content"
              v-html="renderMarkdown((showZhBody ? bodyZh : bodyOriginal) ?? '')"
            ></div>
            <p v-else-if="!bodyNote" class="id-body-note">{{ $t("install-dialog.no-body") }}</p>
          </template>
        </div>
      </div>

      <div v-if="installMsg" class="id-msg">{{ installMsg }}</div>
      <div v-if="!isModpack && !instances.instances.length" class="id-noinst">{{ $t("install-dialog.no-instance-yet") }}<a @click="emit('update:show', false); router.push('/instances')">{{ $t("install-dialog.create-instance-first") }}</a>
      </div>

      <!-- 操作按钮放在内容区内（而非 #footer）：
           版本列表较长时会把窗口撑高，footer 会跑到视口外点不到。 -->
      <div class="id-footer">
        <n-button @click="emit('update:show', false)">{{ $t("common.close") }}</n-button>
        <n-button
          type="primary"
          :loading="installing"
          @click="
            api.logDebug('[fe] 一键安装按钮被点击');
            install();
          "
        >{{ $t("install-dialog.mcreator") }}</n-button>
      </div>
    </div>
  </app-sheet>
</template>

<style>
/* modal content is teleported to <body>; keep styles global */
.id-modal {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.id-head {
  display: flex;
  gap: 14px;
  align-items: flex-start;
}
.id-icon {
  width: 56px;
  height: 56px;
  border-radius: 12px;
  object-fit: cover;
  flex-shrink: 0;
  background: var(--panel);
}
.id-title {
  font-size: 16px;
  font-weight: 700;
  margin-bottom: 4px;
}
.id-meta {
  display: flex;
  gap: 10px;
  font-size: 12px;
  color: var(--text-3);
  margin-bottom: 6px;
}
.id-type {
  background: var(--accent-soft);
  color: var(--accent);
  border-radius: 6px;
  padding: 0 7px;
  font-weight: 600;
}
.id-desc {
  font-size: 12px;
  color: var(--text-3);
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.id-links {
  display: flex;
  gap: 8px;
  margin-top: 6px;
}
.id-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--text-3);
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 7px;
  padding: 3px 9px;
  text-decoration: none;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.12s;
}
.id-link:hover {
  color: var(--accent, #e89a4b);
  border-color: var(--accent-45);
}
.id-link.on {
  color: var(--accent);
  border-color: var(--accent-45);
}
.id-link:disabled {
  opacity: 0.55;
  cursor: default;
}
/* 译文放松行数限制：被裁成两行就看不出翻了什么 */
.id-desc-zh {
  color: var(--text-2);
  -webkit-line-clamp: 4;
}
.id-desc-en {
  margin-top: 4px;
  opacity: 0.72;
}
.id-feedback {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--panel);
}
.id-feedback-title {
  font-size: 12px;
  color: var(--text-2);
}
.id-feedback-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.id-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.id-body-head {
  display: flex;
  gap: 8px;
}
/* 手机横屏视口只有 390px 上下，正文必须自己滚，不能把弹窗撑破 */
.id-body-box {
  max-height: 34vh;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 12px;
  background: var(--panel);
}
.id-body-note {
  margin: 0;
  font-size: 12px;
  color: var(--text-3);
}
.id-body-content {
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-2);
  word-break: break-word;
}
.id-body-content :deep(img),
.id-body-content img {
  max-width: 100%;
  height: auto;
}
.id-body-content h1,
.id-body-content h2,
.id-body-content h3 {
  font-size: 13px;
  margin: 10px 0 6px;
}
.id-body-content p {
  margin: 6px 0;
}
.id-body-content a {
  color: var(--accent);
}
.id-body-content table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11px;
}
.id-body-content th,
.id-body-content td {
  border: 1px solid var(--border);
  padding: 3px 6px;
}
.id-site {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-2);
  border-radius: 8px;
  padding: 6px 11px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
  flex-shrink: 0;
}
.id-site:hover {
  color: var(--accent);
  border-color: var(--accent-45);
}
.id-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.id-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 13px;
  color: var(--text-2);
}
.id-modpack-hint {
  color: var(--accent, #50c878);
  font-weight: 600;
  font-size: 13px;
}
.id-ver-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.id-type-tabs {
  position: relative;
  display: flex;
  gap: 3px;
  background: var(--panel);
  border-radius: 8px;
  padding: 2px;
}
.id-type-tabs .indicator {
  position: absolute;
  top: 2px;
  bottom: 2px;
  border-radius: 6px;
  background: var(--accent-soft);
  pointer-events: none;
}
.id-type-tabs button {
  border: none;
  background: transparent;
  color: var(--text-3);
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
}
.id-type-tabs button.active {
  color: var(--accent);
}
.id-loading {
  padding: 20px;
  text-align: center;
  color: var(--text-3);
  font-size: 13px;
}
.id-ver-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  /* 手机：不再自己开小滚动区（200px 高只能看 3 行），版式交给 AppSheet 的
     整体滚动，列表有多长都能顺畅滑完 */
  max-height: none;
  overflow: visible;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 6px;
  background: var(--panel);
}
.id-ver-row {
  display: flex;
  align-items: center;
  gap: 8px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-2);
  /* 手机：版本行是点选目标，行高按手指给足（原来 6px 内边距只有 ~30px 高） */
  min-height: 46px;
  padding: 10px 12px;
  border-radius: 10px;
  cursor: pointer;
  font-family: inherit;
  text-align: left;
}
.id-ver-row:hover {
  background: var(--panel);
}
.id-ver-row.active {
  border-color: var(--accent-05);
  background: var(--accent-soft);
}
.id-ver-num {
  font-size: 12px;
  font-weight: 600;
  min-width: 0;
  flex-shrink: 1;
}
.id-ver-type {
  font-size: 10px;
  padding: 1px 7px;
  border-radius: 6px;
  font-weight: 600;
  flex-shrink: 0;
}
.id-ver-type.release {
  background: rgba(78, 201, 160, 0.14);
  color: #4ec9a0;
}
.id-ver-type.beta {
  background: var(--accent-soft);
  color: var(--accent);
}
.id-ver-type.alpha {
  background: rgba(229, 83, 75, 0.14);
  color: #e5534b;
}
.id-ver-mc {
  font-size: 11px;
  color: var(--text-3);
  flex: 1;
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.id-ver-date {
  font-size: 11px;
  color: var(--text-3);
  flex-shrink: 0;
}
.id-empty {
  text-align: center;
  color: var(--text-3);
  font-size: 13px;
  padding: 16px 0;
}
.id-deps {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.id-deps-label {
  font-size: 12px;
  font-weight: 700;
  color: var(--text-3);
}
.id-deps-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.id-dep-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-2);
  border-radius: 8px;
  padding: 4px 10px;
  font-size: 12px;
  cursor: pointer;
  font-family: inherit;
}
.id-dep-chip:hover {
  border-color: var(--accent-45);
}
.id-dep-tag {
  font-size: 10px;
  font-weight: 700;
  padding: 0 6px;
  border-radius: 5px;
}
.id-dep-chip.required .id-dep-tag {
  background: rgba(229, 83, 75, 0.16);
  color: #e5534b;
}
.id-dep-chip.optional .id-dep-tag {
  background: rgba(90, 162, 240, 0.15);
  color: #7cb8f5;
}
.id-dep-chip.incompatible .id-dep-tag {
  background: var(--border);
  color: var(--text-3);
}
.id-deps-loading {
  font-size: 12px;
  color: var(--text-3);
}
.id-msg {
  font-size: 13px;
  color: var(--accent);
}
.id-noinst {
  font-size: 12px;
  color: var(--text-3);
}
.id-noinst a {
  color: var(--accent);
  cursor: pointer;
}
.id-footer {
  display: flex;
  gap: 10px;
}
/* 手机：安装是这一步的主操作，两个键平分一行、高度给足（sticky 由 AppSheet 负责）。
   注意这里用的是全局样式块（弹层内容 teleport 到 #van-layer，scoped 够不到）。 */
.id-footer .n-button {
  flex: 1;
  min-height: 46px;
  font-size: 15px;
}
/* 反馈面板的小按钮也抬到可点高度 */
.id-feedback-actions .n-button {
  min-height: 38px;
}
</style>

<!--
  安装弹窗的额外约束：内容区必须**内部滚动**，否则版本列表一长就把底部按钮顶出屏幕。
  AppSheet 的 .sheet-body 已经处理好滚动与安全区，这里只剩「让弹窗本身别超过一屏」。 -->
<style>
.install-dialog-card {
  max-height: 92vh;
  display: flex;
  flex-direction: column;
}
</style>
