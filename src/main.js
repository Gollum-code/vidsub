import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";

const $ = (id) => document.getElementById(id);

const els = {
  dropZone: $("drop-zone"),
  pickBtn: $("pick-btn"),
  fileRow: $("file-row"),
  fileName: $("file-name"),
  changeFile: $("change-file"),
  whisperPath: $("whisper-path"),
  modelPath: $("model-path"),
  language: $("language"),
  startBtn: $("start-btn"),
  modelSize: $("model-size"),
  downloadModelBtn: $("download-model-btn"),
  limitHint: $("limit-hint"),
  queueHint: $("queue-hint"),
  depsState: $("deps-state"),
  progressWrap: $("progress-wrap"),
  progressBar: $("progress-bar"),
  progressMessage: $("progress-message"),
  resultCard: $("result-card"),
  segmentList: $("segment-list"),
  exportBtn: $("export-btn"),
  exportFormat: $("export-format"),
  exportStyle: $("export-style"),
  burnBtn: $("burn-btn"),
  apiProvider: $("api-provider"),
  apiKey: $("api-key"),
  targetLang: $("target-lang"),
  translateBtn: $("translate-btn"),
  restoreBtn: $("restore-btn"),
  preview: $("preview"),
  waveform: $("waveform")
};

let currentFile = null;
let queue = [];
let currentSegments = [];
let currentSrtPath = null;
let deps = null;
let originalSegments = null;
let waveData = null;
let dragState = null;
let startRefs = [];
let endRefs = [];

function fmtDuration(sec) {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function fmtTimeInput(sec) {
  const m = Math.floor(sec / 60);
  const s = sec - m * 60;
  return `${m}:${s.toFixed(2).padStart(5, "0")}`;
}

function parseTimeInput(text) {
  const t = text.trim();
  if (!t) return NaN;
  if (t.includes(":")) {
    const parts = t.split(":");
    if (parts.length === 2) {
      const m = Number(parts[0]);
      const s = Number(parts[1]);
      if (Number.isFinite(m) && Number.isFinite(s)) return m * 60 + s;
    }
  }
  const n = Number(t);
  return Number.isFinite(n) ? n : NaN;
}

function baseName(path) {
  return (path || "subtitle").split(/[\\/]/).pop();
}

function cleanSegments() {
  return currentSegments.map((s) => ({ start: s.start, end: s.end, text: s.text }));
}

async function refreshDeps() {
  try {
    deps = await invoke("check_dependencies");
    const parts = [];
    parts.push(`ffmpeg ${deps.ffmpegOk ? "正常" : "缺失"}`);
    parts.push(`whisper ${deps.whisperOk ? "正常" : "缺失"}`);
    parts.push(`模型 ${deps.modelOk ? "正常" : "未配置"}`);
    els.depsState.textContent = parts.join(" · ");
    els.depsState.classList.toggle("missing", !(deps.ffmpegOk && deps.whisperOk && deps.modelOk));
    els.downloadModelBtn.classList.toggle("hidden", deps.modelOk);
    els.modelSize.classList.toggle("hidden", deps.modelOk);
    refreshLimitHint();
    refreshStartState();
    refreshResultActions();
  } catch (err) {
    els.depsState.textContent = "依赖检测失败";
  }
}

function refreshLimitHint() {
  els.limitHint.textContent = "全功能免费版 · 无时长限制 · 支持导出/翻译/烧录";
}

function refreshStartState() {
  const ok = deps && deps.ffmpegOk && deps.whisperOk && deps.modelOk && currentFile != null;
  els.startBtn.disabled = !ok;
}

function refreshResultActions() {
  const hasResult = currentSegments.length > 0;
  els.exportBtn.disabled = !hasResult;
  els.burnBtn.disabled = !hasResult || !currentFile;
}

function refreshQueueHint() {
  els.queueHint.textContent = queue.length > 1 ? `队列：${queue.length} 个文件` : "";
}

function persistSettings() {
  const settings = {
    whisperPath: els.whisperPath.value.trim() || null,
    modelPath: els.modelPath.value.trim() || null,
    language: els.language.value,
    apiProvider: els.apiProvider.value,
    targetLang: els.targetLang.value
  };
  invoke("set_settings", { settings });
  refreshDeps();
}

async function loadSettings() {
  try {
    const s = await invoke("get_settings");
    els.whisperPath.value = s.whisperPath || "";
    els.modelPath.value = s.modelPath || "";
    els.language.value = s.language || "auto";
    els.apiProvider.value = s.apiProvider || "deepseek";
    els.targetLang.value = s.targetLang || "zh";
  } catch (err) {
    els.depsState.textContent = "加载设置失败";
  }
}

async function loadExportStyles() {
  try {
    const names = await invoke("get_export_styles");
    els.exportStyle.innerHTML = "";
    names.forEach((n, i) => {
      const opt = document.createElement("option");
      opt.value = String(i);
      opt.textContent = n;
      els.exportStyle.appendChild(opt);
    });
  } catch (err) {
    /* noop */
  }
}

async function chooseFile() {
  const selected = await open({
    multiple: true,
    filters: [
      { name: "视频/音频", extensions: ["mp4", "mkv", "mov", "avi", "webm", "ts", "m4a", "mp3", "wav", "flac", "aac"] }
    ]
  });
  if (!selected) return;
  const files = Array.isArray(selected) ? selected : [selected];
  queue = files.slice();
  setFile(files[0]);
  refreshQueueHint();
}

function setFile(path) {
  currentFile = path;
  els.fileName.textContent = path;
  els.dropZone.classList.add("hidden");
  els.fileRow.classList.remove("hidden");
  refreshStartState();
  refreshResultActions();
}

async function transcribeOne(file) {
  const result = await invoke("transcribe", {
    inputPath: file,
    modelPath: els.modelPath.value.trim() || null,
    whisperPath: els.whisperPath.value.trim() || null,
    language: els.language.value || "auto"
  });
  currentSrtPath = result.srtPath;
  originalSegments = null;
  try {
    waveData = await invoke("waveform", { inputPath: file });
  } catch (err) {
    waveData = null;
  }
  currentSegments = result.segments;
  els.preview.src = convertFileSrc(file);
  els.preview.classList.remove("hidden");
  renderSegments(currentSegments);
  return result;
}

async function startTranscribe() {
  const files = queue.length > 0 ? queue.slice() : [currentFile].filter(Boolean);
  if (files.length === 0) return;
  els.startBtn.disabled = true;
  els.resultCard.classList.add("hidden");
  els.progressWrap.classList.remove("hidden");
  els.progressBar.style.width = "0%";

  try {
    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      currentFile = file;
      els.progressMessage.textContent = files.length > 1 ? `正在处理 ${i + 1}/${files.length}：${baseName(file)}` : "识别中...";
      const result = await transcribeOne(file);
      els.progressMessage.textContent = `完成 ${i + 1}/${files.length}：${result.srtPath}`;
    }
    queue = [];
    refreshQueueHint();
    els.resultCard.classList.remove("hidden");
    showDone("全部完成：" + currentSrtPath);
  } catch (err) {
    els.progressMessage.textContent = "失败：" + err;
  } finally {
    els.startBtn.disabled = true;
    refreshDeps();
  }
}

function renderSegments(segments) {
  currentSegments = segments;
  startRefs = [];
  endRefs = [];
  els.segmentList.innerHTML = "";
  segments.forEach((seg, i) => {
    const row = document.createElement("div");
    row.className = "segment";

    const idx = document.createElement("div");
    idx.className = "idx";
    idx.textContent = String(i + 1).padStart(2, "0");

    const time = document.createElement("div");
    time.className = "time";

    const startInput = document.createElement("input");
    startInput.type = "text";
    startInput.value = fmtTimeInput(seg.start);
    startInput.title = "开始时间（秒 或 分:秒）";
    const endInput = document.createElement("input");
    endInput.type = "text";
    endInput.value = fmtTimeInput(seg.end);
    endInput.title = "结束时间（秒 或 分:秒）";
    startRefs.push(startInput);
    endRefs.push(endInput);

    const sep = document.createElement("span");
    sep.textContent = "→";

    const input = document.createElement("textarea");
    input.value = seg.text.trim();
    input.addEventListener("input", () => {
      currentSegments[i].text = input.value;
    });

    const applyTime = () => {
      const s = parseTimeInput(startInput.value);
      const e = parseTimeInput(endInput.value);
      if (Number.isFinite(s) && Number.isFinite(e) && s >= 0 && e > s) {
        const prevEnd = i > 0 ? currentSegments[i - 1].end : -Infinity;
        const nextStart = i < currentSegments.length - 1 ? currentSegments[i + 1].start : Infinity;
        currentSegments[i].start = Math.max(s, prevEnd);
        currentSegments[i].end = Math.min(e, nextStart);
        startInput.value = fmtTimeInput(currentSegments[i].start);
        endInput.value = fmtTimeInput(currentSegments[i].end);
        drawWaveform();
      }
    };
    startInput.addEventListener("change", applyTime);
    endInput.addEventListener("change", applyTime);

    time.append(startInput, sep, endInput);
    row.append(idx, time, input);
    els.segmentList.appendChild(row);
  });

  els.restoreBtn.classList.toggle("hidden", originalSegments == null);
  drawWaveform();
  refreshResultActions();
}

function timeFromEvent(ev) {
  const rect = els.waveform.getBoundingClientRect();
  const frac = (ev.clientX - rect.left) / rect.width;
  return Math.max(0, frac * (waveData.duration || 1));
}

function drawWaveform() {
  const canvas = els.waveform;
  if (!waveData || waveData.peaks.length === 0 || currentSegments.length === 0) {
    canvas.classList.add("hidden");
    return;
  }
  canvas.classList.remove("hidden");
  const width = canvas.clientWidth || 960;
  const height = canvas.height || 120;
  canvas.width = width * 2;
  canvas.height = height * 2;
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  const ctx = canvas.getContext("2d");
  ctx.scale(2, 2);
  ctx.fillStyle = "#1b1d23";
  ctx.fillRect(0, 0, width, height);

  const peaks = waveData.peaks;
  const barW = width / peaks.length;
  ctx.fillStyle = "#3a4052";
  for (let i = 0; i < peaks.length; i++) {
    const h = Math.max(1, peaks[i] * height * 0.9);
    const x = i * barW;
    ctx.fillRect(x, height / 2 - h / 2, Math.max(1, barW - 1), h);
  }

  const dur = waveData.duration || 1;
  const segW = (seg) => {
    const x = (seg.start / dur) * width;
    const w = Math.max(2, ((seg.end - seg.start) / dur) * width);
    return { x, w };
  };
  currentSegments.forEach((seg) => {
    const { x, w } = segW(seg);
    ctx.fillStyle = "rgba(79, 140, 255, 0.28)";
    ctx.fillRect(x, 0, w, height);
    ctx.fillStyle = "rgba(79, 140, 255, 0.9)";
    ctx.fillRect(x, height - 6, 4, 6);
    ctx.fillRect(x + w - 4, height - 6, 4, 6);
  });
}

function waveformPointerDown(ev) {
  if (!waveData || currentSegments.length === 0) return;
  const rect = els.waveform.getBoundingClientRect();
  const t = timeFromEvent(ev);
  const pxSec = 8 / rect.width * (waveData.duration || 1);
  for (let i = 0; i < currentSegments.length; i++) {
    const seg = currentSegments[i];
    if (Math.abs(t - seg.start) <= pxSec) {
      dragState = { i, mode: "start" };
    } else if (Math.abs(t - seg.end) <= pxSec) {
      dragState = { i, mode: "end" };
    }
  }
  if (dragState) {
    ev.preventDefault();
    els.waveform.setPointerCapture(ev.pointerId);
  }
}

function waveformPointerMove(ev) {
  if (!dragState) return;
  const t = timeFromEvent(ev);
  const i = dragState.i;
  const seg = currentSegments[i];
  const prevEnd = i > 0 ? currentSegments[i - 1].end : -Infinity;
  const nextStart = i < currentSegments.length - 1 ? currentSegments[i + 1].start : Infinity;
  if (dragState.mode === "start") {
    seg.start = Math.min(Math.max(t, prevEnd), seg.end - 0.05);
  } else {
    seg.end = Math.max(Math.min(t, nextStart), seg.start + 0.05);
  }
  if (startRefs[i]) startRefs[i].value = fmtTimeInput(seg.start);
  if (endRefs[i]) endRefs[i].value = fmtTimeInput(seg.end);
  drawWaveform();
}

function waveformPointerUp(ev) {
  if (!dragState) return;
  dragState = null;
  try {
    els.waveform.releasePointerCapture(ev.pointerId);
  } catch (err) {
    /* noop */
  }
  drawWaveform();
}

function waveformClick(ev) {
  if (dragState) return;
  const t = timeFromEvent(ev);
  const v = els.preview;
  if (v && v.duration) {
    v.currentTime = Math.min(t, v.duration);
    v.play().catch(() => {});
  }
}

async function exportSubtitle() {
  if (!currentSrtPath || currentSegments.length === 0) return;
  const fmt = els.exportFormat.value;
  const ext = fmt === "vtt" ? "vtt" : fmt === "ass" ? "ass" : "srt";
  const suggested = (baseName(currentSrtPath).replace(/\.(srt|vtt|ass)$/i, "") || "subtitle") + "." + ext;
  const outPath = await save({
    defaultPath: suggested,
    filters: [{ name: fmt.toUpperCase(), extensions: [ext] }]
  });
  if (!outPath) return;
  const styleIndex = fmt === "ass" ? Number(els.exportStyle.value || 0) : 0;
  try {
    const saved = await invoke("export_subtitles", {
      segments: cleanSegments(),
      path: outPath,
      format: fmt,
      styleIndex
    });
    showDone("已导出：" + saved);
  } catch (err) {
    els.progressMessage.textContent = "导出失败：" + err;
  }
}

async function burnSubtitle() {
  if (!currentFile || currentSegments.length === 0) return;
  const outPath = await save({
    defaultPath: (baseName(currentFile).replace(/\.[^.]+$/, "") || "burned") + "-sub.mp4",
    filters: [{ name: "MP4", extensions: ["mp4"] }]
  });
  if (!outPath) return;
  els.progressWrap.classList.remove("hidden");
  els.progressBar.style.width = "0%";
  els.progressMessage.textContent = "烧录中，请稍候...";
  try {
    const saved = await invoke("burn_subtitles", {
      inputPath: currentFile,
      outPath,
      segments: cleanSegments(),
      styleIndex: Number(els.exportStyle.value || 0)
    });
    showDone("烧录完成：" + saved);
  } catch (err) {
    els.progressMessage.textContent = "烧录失败：" + err;
  }
}

function showDone(msg) {
  els.progressWrap.classList.remove("hidden");
  els.progressBar.style.width = "100%";
  els.progressMessage.textContent = msg;
}

async function translateSegments() {
  if (currentSegments.length === 0) return;
  const key = els.apiKey.value.trim();
  if (!key) {
    els.progressMessage.textContent = "请先在结果栏输入 API Key";
    return;
  }
  try {
    await invoke("set_api_key", { key });
  } catch (err) {
    els.progressMessage.textContent = "保存 API Key 失败：" + err;
    return;
  }
  const provider = els.apiProvider.value;
  const target = els.targetLang.value;
  if (originalSegments == null) {
    originalSegments = JSON.parse(JSON.stringify(currentSegments));
  }
  els.progressWrap.classList.remove("hidden");
  els.progressBar.style.width = "0%";
  els.progressMessage.textContent = "翻译中...";
  try {
    const lines = await invoke("translate_segments", {
      segments: cleanSegments(),
      targetLang: target,
      provider
    });
    currentSegments = currentSegments.map((s, i) => ({
      ...s,
      text: lines[i] != null ? lines[i] : s.text
    }));
    renderSegments(currentSegments);
    showDone("翻译完成");
  } catch (err) {
    els.progressMessage.textContent = "翻译失败：" + err;
  }
}

function restoreOriginal() {
  if (originalSegments == null) return;
  currentSegments = originalSegments;
  originalSegments = null;
  renderSegments(currentSegments);
}

async function downloadModel() {
  els.downloadModelBtn.disabled = true;
  els.progressWrap.classList.remove("hidden");
  els.progressBar.style.width = "0%";
  try {
    const p = await invoke("download_model", { size: els.modelSize.value });
    showDone("模型已就绪：" + p);
  } catch (err) {
    els.progressMessage.textContent = "模型下载失败：" + err;
  } finally {
    els.downloadModelBtn.disabled = false;
    await refreshDeps();
  }
}

function bindEvents() {
  els.pickBtn.addEventListener("click", chooseFile);
  els.changeFile.addEventListener("click", chooseFile);
  els.startBtn.addEventListener("click", startTranscribe);
  els.downloadModelBtn.addEventListener("click", downloadModel);
  els.exportBtn.addEventListener("click", exportSubtitle);
  els.burnBtn.addEventListener("click", burnSubtitle);
  els.translateBtn.addEventListener("click", translateSegments);
  els.restoreBtn.addEventListener("click", restoreOriginal);
  els.exportFormat.addEventListener("change", () => {
    els.exportStyle.classList.toggle("hidden", els.exportFormat.value !== "ass");
  });
  els.waveform.addEventListener("pointerdown", waveformPointerDown);
  els.waveform.addEventListener("pointermove", waveformPointerMove);
  els.waveform.addEventListener("pointerup", waveformPointerUp);
  els.waveform.addEventListener("pointercancel", waveformPointerUp);
  els.waveform.addEventListener("click", waveformClick);

  getCurrentWebview().onDragDropEvent((event) => {
    if (event.type === "enter" || event.type === "over") {
      els.dropZone.classList.add("dragover");
    } else if (event.type === "leave") {
      els.dropZone.classList.remove("dragover");
    } else if (event.type === "drop") {
      els.dropZone.classList.remove("dragover");
      if (event.paths && event.paths.length > 0) {
        queue = event.paths.slice();
        setFile(event.paths[0]);
        refreshQueueHint();
      }
    }
  });

  ["whisperPath", "modelPath", "apiProvider", "targetLang"].forEach((id) => {
    els[id].addEventListener("change", persistSettings);
  });
  els.language.addEventListener("change", persistSettings);

  listen("vidsub-progress", (ev) => {
    const p = ev.payload;
    els.progressBar.style.width = `${p.percent}%`;
    els.progressMessage.textContent = `[${p.stage}] ${p.message}`;
  });
}

async function init() {
  bindEvents();
  await loadSettings();
  await loadExportStyles();
  await refreshDeps();
}

init();