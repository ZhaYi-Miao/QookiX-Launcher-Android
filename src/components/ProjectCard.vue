<script setup lang="ts">
import { t as $t } from "../i18n";
import { ref } from "vue";
import { Button as VanButton } from "vant";
import { fmtCount as fmt } from "../utils/format";
import { translateCategory } from "../utils/categories";
import { IconDownload, IconHeart, IconPlus } from "./icons";
import type { ProjectHit } from "../types";

/** 手机形态的内容卡：方块图标 + 标题/作者 + 两行描述 + 底行「安装」。
 *  桌面的悬停显示按钮、右侧操作列在手机上都不成立 —— 主操作直接常显。 */
defineProps<{
  project: ProjectHit;
  /** 已翻译的描述（整页翻译成功后由父组件传入；为空则显示原文） */
  translatedDesc?: string | null;
  /** 这条正在翻译中：描述区显示骨架，避免用户以为「点了没反应」 */
  translating?: boolean;
}>();
const emit = defineEmits<{ install: [p: ProjectHit] }>();
const iconError = ref(false);
</script>

<template>
  <article class="pc glass" @click="emit('install', project)">
    <div class="top">
      <img v-if="project.icon_url && !iconError" :src="project.icon_url" class="ico" alt="" loading="lazy" @error="iconError = true" />
      <div v-else class="ico ph"><IconPlus /></div>
      <div class="tt">
        <div class="title">{{ project.title }}</div>
        <div class="author">{{ project.author }}</div>
      </div>
    </div>
    <p v-if="translating" class="desc skeleton" aria-busy="true">
      <span class="sk-line"></span>
      <span class="sk-line short"></span>
    </p>
    <p v-else class="desc">{{ translatedDesc ?? project.description }}</p>
    <div class="cats">
      <span v-for="c in project.categories.slice(0, 2)" :key="c" class="cat">{{ translateCategory(c) }}</span>
    </div>
    <div class="foot">
      <span class="stat"><IconDownload /> {{ fmt(project.downloads) }}</span>
      <span v-if="project.follows" class="stat"><IconHeart /> {{ fmt(project.follows) }}</span>
      <van-button size="small" type="primary" class="ins" @click.stop="emit('install', project)">
        {{ $t("common.install") }}
      </van-button>
    </div>
  </article>
</template>

<style scoped>
.pc {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  cursor: pointer;
}
.top {
  display: flex;
  gap: 10px;
  align-items: center;
}
.ico {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  object-fit: cover;
  flex-shrink: 0;
  background: var(--panel-hover);
}
.ico.ph {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--accent);
}
.tt {
  min-width: 0;
  flex: 1;
}
.title {
  font-size: 14px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.author {
  font-size: 11px;
  color: var(--text-3);
  margin-top: 2px;
}
.desc {
  margin: 0;
  font-size: 12px;
  color: var(--text-2);
  line-height: 1.45;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.cats {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
/* 翻译中的骨架：两行灰条，别让卡片看起来「卡住了」 */
.desc.skeleton {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 33px;
}
.sk-line {
  height: 10px;
  border-radius: 5px;
  background: linear-gradient(90deg, var(--panel-hover) 25%, rgba(255, 255, 255, 0.09) 37%, var(--panel-hover) 63%);
  background-size: 400% 100%;
  animation: sk 1.2s ease-in-out infinite;
}
.sk-line.short {
  width: 62%;
}
@keyframes sk {
  0% {
    background-position: 100% 50%;
  }
  100% {
    background-position: 0 50%;
  }
}
.cat {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 5px;
  background: var(--panel-hover);
  color: var(--text-3);
}
.foot {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: auto;
}
.stat {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
  color: var(--text-3);
}
.ins {
  margin-left: auto;
  min-height: 34px;
}
</style>
