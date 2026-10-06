import type { TaskEntry } from "../stores/tasks";

/**
 * 任务总体进度（0–100）。
 *
 * 有字节数就按字节算，退回安装步骤数；两者都没有则为 0。
 * 下载页与首页的「正在下载」卡片共用同一口径，避免两处各写一份。
 */
export function taskPercent(t: TaskEntry): number {
  if (t.bytesTotal > 0) return Math.min(100, Math.round((t.bytesDone / t.bytesTotal) * 100));
  if (t.stepTotal > 0) return Math.min(100, Math.round((t.stepDone / t.stepTotal) * 100));
  return 0;
}
