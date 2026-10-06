<script setup lang="ts">
import { t as $t } from "../i18n";
import { ref, watch } from "vue";
import AppButton from "../ui/AppButton.vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useAccountsStore } from "../stores/accounts";
import AppSheet from "../ui/AppSheet.vue";

const accounts = useAccountsStore();
const show = ref(false);

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
  }
}

watch(
  () => accounts.msFlow,
  (f) => {
    show.value = !!f;
    if (f) {
      if (f.userCode) copyText(f.userCode).catch(() => {});
      if (f.verificationUri) openUrl(f.verificationUri).catch(() => {});
    }
  },
  { immediate: true }
);

async function copyCode() {
  if (!accounts.msFlow?.userCode) return;
  await copyText(accounts.msFlow.userCode);
}

function close() {
  show.value = false;
  accounts.msFlow = null;
  accounts.msError = "";
}

async function retry() {
  accounts.msError = "";
  try {
    await accounts.startMs();
  } catch (e) {
    // 拉设备码失败（断网/接口异常）时不捕获会变成未处理的 Promise 拒绝，界面上什么都没有
    accounts.msError = String(e).replace(/^Error:\s*/, "");
  }
}
</script>

<template>
  <app-sheet
    :show="show || !!accounts.msError"
    :title="$t('ms-login-dialog.login-title')"
    :mask-closable="false"
  >
    <div class="qkms-box">
      <template v-if="accounts.msError">
        <div class="qkms-error-box">{{ accounts.msError }}</div>
      </template>
      <template v-else>
        <p>{{ $t("ms-login-dialog.auto-copied-hint") }}</p>
        <a class="qkms-link" @click="accounts.msFlow && openUrl(accounts.msFlow.verificationUri)">{{ accounts.msFlow?.verificationUri || "…" }}</a>
        <div class="qkms-code mono">{{ accounts.msFlow?.userCode || $t('ms-login-dialog.waiting') }}</div>
        <p class="qkms-hint">{{ $t("ms-login-dialog.waiting-hint") }}</p>
        <div v-if="accounts.msPolling" class="qkms-polling">{{ $t("ms-login-dialog.awaiting-auth") }}</div>
      </template>
    </div>
    <template #footer>
      <div class="qkms-footer">
        <app-button @click="close">{{ $t("common.close") }}</app-button>
        <template v-if="!accounts.msError">
          <app-button @click="copyCode">{{ $t("ms-login-dialog.copy-code") }}</app-button>
          <app-button type="primary" @click="accounts.manualCheck">{{ $t("ms-login-dialog.done") }}</app-button>
        </template>
        <app-button v-if="accounts.msError" type="primary" @click="retry">{{ $t("first-run-setup.retry") }}</app-button>
      </div>
    </template>
  </app-sheet>
</template>

<style>
.qkms-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}
.qkms-box p {
  margin: 0;
  font-size: 13px;
  color: #c6c8d2;
  text-align: center;
}
.qkms-link {
  color: #e89a4b;
  font-size: 14px;
  word-break: break-all;
  cursor: pointer;
  text-decoration: underline;
}
.qkms-code {
  font-size: 30px;
  font-weight: 800;
  letter-spacing: 10px;
  text-align: center;
  color: #e89a4b;
  background: var(--accent-08);
  border: 1px dashed var(--accent-04);
  border-radius: 12px;
  padding: 14px 0;
  width: 100%;
}
.qkms-copy {
  border: 1px solid var(--accent-04);
  background: var(--accent-01);
  color: #e89a4b;
  border-radius: 8px;
  padding: 6px 18px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
}
.qkms-copy:hover {
  background: var(--accent-02);
}
.qkms-hint {
  font-size: 12px;
  color: #8b8e9c;
}
.qkms-polling {
  font-size: 12px;
  color: #e89a4b;
}
.qkms-error-box {
  color: #e5534b;
  font-size: 13px;
  line-height: 1.6;
  word-break: break-all;
  user-select: text;
  -webkit-user-select: text;
  cursor: text;
  background: rgba(229, 83, 75, 0.08);
  border: 1px solid rgba(229, 83, 75, 0.3);
  border-radius: 8px;
  padding: 12px;
  width: 100%;
}
.qkms-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
</style>
