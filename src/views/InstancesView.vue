<script setup lang="ts">
import { t as $t } from "../i18n";
import { computed, inject, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useInstancesStore } from "../stores/instances";
import { useMessage } from "naive-ui";
import { Button as VanButton } from "vant";
import InstanceCard from "../components/InstanceCard.vue";
import { IconPlus } from "../components/icons";
import type { Instance, InstanceGroup } from "../types";
import type { Ref } from "vue";

const router = useRouter();
const message = useMessage();
const instances = useInstancesStore();
const pageAction = inject<Ref<{ text: string; run: () => void } | null>>("pageAction");

const PALETTE = ["#e89a4b", "#5ab0ff", "#7ad08a", "#c78aff", "#ff7a90", "#ffd166", "#4ecdc4", "#a0a4b8"];
const FILTER_KEY = "qookix.instances.filter";

const filter = ref<string>(localStorage.getItem(FILTER_KEY) ?? "all");
const showNewGroup = ref(false);
const groupName = ref("");
const groupColor = ref<string | null>(null);
const savingGroup = ref(false);
const moving = ref<Instance | null>(null);

const totalCount = computed(() => instances.instances.length);

const sections = computed(() => {
  if (filter.value !== "all") return [];
  const list: { key: string; name: string; color: string | null; group: InstanceGroup | null; items: Instance[] }[] = instances.groups
    .map((g) => ({ key: g.id, name: g.name, color: g.color as string | null, group: g, items: instances.inGroup(g.id) }))
    .filter((s) => s.items.length || s.group);
  const rest = instances.ungrouped;
  if (rest.length) list.push({ key: "__ungrouped__", name: $t("create-instance.ungrouped"), color: null, group: null, items: rest });
  return list;
});

const filtered = computed(() => {
  if (filter.value === "all") return [];
  if (filter.value === "ungrouped") return instances.ungrouped;
  return instances.inGroup(filter.value);
});

function setFilter(id: string) {
  filter.value = id;
  localStorage.setItem(FILTER_KEY, id);
}

async function saveGroup() {
  if (!groupName.value.trim()) return;
  savingGroup.value = true;
  try {
    await instances.createGroup(groupName.value.trim(), groupColor.value);
    groupName.value = "";
    groupColor.value = null;
    showNewGroup.value = false;
    message.success($t("instances.on-ok"));
  } catch (e) {
    message.error(String(e));
  } finally {
    savingGroup.value = false;
  }
}

async function moveTo(groupId: string | null) {
  if (!moving.value) return;
  try {
    await instances.moveToGroup(moving.value.id, groupId);
    moving.value = null;
  } catch (e) {
    message.error(String(e));
  }
}

onMounted(() => {
  if (pageAction) pageAction.value = { text: $t("router.plus"), run: () => router.push("/create") };
});
</script>

<template>
  <div class="iv">
    <div v-if="instances.loading" class="loading">{{ $t("file-manager.loading") }}</div>

    <template v-else-if="totalCount">
      <div class="chips">
        <button class="chip" :class="{ active: filter === 'all' }" @click="setFilter('all')">
          {{ $t("install-dialog.all") }}<span class="cc">{{ totalCount }}</span>
        </button>
        <button v-for="g in instances.groups" :key="g.id" class="chip" :class="{ active: filter === g.id }" @click="setFilter(g.id)">
          <i class="dot" :style="{ background: g.color || 'var(--accent)' }"></i>{{ g.name }}
          <span class="cc">{{ instances.inGroup(g.id).length }}</span>
        </button>
        <button class="chip" :class="{ active: filter === 'ungrouped' }" @click="setFilter('ungrouped')">
          {{ $t("create-instance.ungrouped") }}<span class="cc">{{ instances.ungrouped.length }}</span>
        </button>
      </div>

      <template v-if="filter === 'all'">
        <section v-for="s in sections" :key="s.key" class="grp">
          <h2 class="grp-head">
            <i class="dot" :style="{ background: s.color || 'var(--text-3)' }"></i>{{ s.name }}
            <span class="grp-n">{{ s.items.length }}</span>
          </h2>
          <div class="grid">
            <InstanceCard v-for="i in s.items" :key="i.id" :instance="i" @move="moving = $event" />
          </div>
        </section>
      </template>
      <div v-else class="grid">
        <InstanceCard v-for="i in filtered" :key="i.id" :instance="i" @move="moving = $event" />
      </div>
    </template>

    <div v-else class="empty glass">
      <p>{{ $t("instances.empty-hint") }}</p>
      <van-button type="primary" @click="router.push('/create')"><IconPlus /> {{ $t("home.create-first") }}</van-button>
    </div>

    <app-popup :show="showNewGroup" position="bottom" round @update:show="(v: boolean) => (showNewGroup = v)">
      <div class="sheet">
        <div class="sheet-title">{{ $t("title-bar.new-group") }}</div>
        <app-input v-model:value="groupName" :placeholder="$t('instances.name-hint')" maxlength="40" />
        <div class="palette">
          <button v-for="c in PALETTE" :key="c" class="sw" :class="{ on: groupColor === c }" :style="{ background: c }" @click="groupColor = c"></button>
        </div>
        <van-button block type="primary" :loading="savingGroup" @click="saveGroup">{{ $t("common.save") }}</van-button>
      </div>
    </app-popup>

    <app-popup :show="moving !== null" position="bottom" round @update:show="(v: boolean) => { if (!v) moving = null; }">
      <div v-if="moving" class="sheet">
        <div class="sheet-title">{{ $t("instance-card.move-to-group") }}</div>
        <button class="mv" :class="{ cur: !moving.group }" @click="moveTo(null)">{{ $t("create-instance.ungrouped") }}</button>
        <button v-for="g in instances.groups" :key="g.id" class="mv" :class="{ cur: moving.group === g.id }" @click="moveTo(g.id)">
          <i class="dot" :style="{ background: g.color || 'var(--accent)' }"></i>{{ g.name }}
        </button>
      </div>
    </app-popup>
  </div>
</template>

<style scoped>
.iv {
  padding: 4px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.loading {
  text-align: center;
  color: var(--text-3);
  padding: 40px 0;
}
.chips {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  scrollbar-width: none;
  padding-bottom: 2px;
}
.chip {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 36px;
  padding: 6px 12px;
  border-radius: 18px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-2);
  font-family: inherit;
  font-size: 13px;
  white-space: nowrap;
}
.chip.active {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-08);
}
.cc {
  font-size: 11px;
  color: var(--text-3);
}
.grp-head {
  display: flex;
  align-items: center;
  gap: 7px;
  margin: 0 0 10px;
  font-size: 14px;
  font-weight: 600;
}
.grp-n {
  font-size: 11px;
  color: var(--text-3);
  font-weight: 400;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(160px, 100%), 1fr));
  gap: 10px;
  grid-auto-rows: max-content;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.empty {
  padding: 40px 16px;
  text-align: center;
  color: var(--text-3);
  display: flex;
  flex-direction: column;
  gap: 12px;
  align-items: center;
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
.palette {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.sw {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 2px solid transparent;
  padding: 0;
}
.sw.on {
  border-color: var(--text-1);
}
.mv {
  min-height: 44px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 12px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-1);
  font-family: inherit;
  font-size: 14px;
}
.mv.cur {
  border-color: var(--accent);
  color: var(--accent);
}
</style>
