import { defineStore } from "pinia";
import { api } from "../api";
import type { Settings } from "../types";

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    settings: null as Settings | null,
    loading: false,
  }),
  getters: {
    maxMemory: (s) => s.settings?.max_memory_mb ?? 4096,
  },
  actions: {
    async load() {
      this.loading = true;
      try {
        this.settings = await api.getSettings();
      } finally {
        this.loading = false;
      }
    },
    async patch(patch: Record<string, unknown>) {
      this.settings = await api.setSettings(patch);
    },
    async save() {
      if (this.settings) {
        this.settings = await api.setSettings(this.settings as unknown as Record<string, unknown>);
      }
    },
  },
});
