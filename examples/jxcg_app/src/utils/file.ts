// ============================================================================
// 浏览器文件导出/导入小工具（行情 JSON、存档导出、粘贴导入用）
// ============================================================================

/** 触发浏览器下载。 */
export function downloadText(filename: string, text: string, mime = "application/json") {
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  // 延迟回收，避免部分浏览器取消下载。
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** 下载 JSON 文件（缩进 2 空格，便于用户查看/手改）。 */
export function downloadJson(filename: string, data: unknown) {
  downloadText(filename, JSON.stringify(data, null, 2));
}

/** 弹出文件选择器并读取文本；用户取消返回 null。 */
export function pickTextFile(accept = ".json,application/json"): Promise<{ name: string; text: string } | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) {
        resolve(null);
        return;
      }
      file
        .text()
        .then((text) => resolve({ name: file.name, text }))
        .catch(() => resolve(null));
    };
    input.click();
  });
}

/** 文件名用日期戳：jxcg-cundang-YYYYMMDD-HHmm.json */
export function dateStamp(date = new Date()): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}`;
}
