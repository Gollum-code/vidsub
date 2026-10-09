# vidsub 技术设计文档

> 对应《02-vidsub视频字幕 · 变现计划书》的实现设计。产品现为**全功能免费**，原 Pro 门控已下线。

## 1. 目标与边界

- 桌面应用：拖入视频/音频 → 本地识别 → 字幕编辑 → 导出/烧录。
- 本地优先：识别、波形、导出、烧录全部本地完成；翻译为可选 BYOK 在线调用。
- 无时长限制、无语言限制、无导出/烧录/翻译门控。

## 2. 总体架构

```
┌──────────────────────── Tauri v2 进程 ────────────────────────┐
│ WebView2 (Vite/原生JS)                                        │
│  拖放/选择 → transcribe → 波形 canvas + 预览 <video>          │
│  编辑文本/时间 → export_subtitles / burn_subtitles            │
│  翻译 → translate_segments（BYOK）                            │
└──────────────┬────────────────────────────────────────────────┘
               │ invoke(command) / event(vidsub-progress)
┌──────────────▼────────────────────────────────────────────────┐
│ Rust lib (vidsub_lib)                                         │
│  audio.rs ──ffmpeg──→ 16k mono wav                            │
│  whisper.rs ──whisper-cli──→ JSON(segments)                   │
│  align.rs：标点断句（分治）                                    │
│  export.rs：SRT/VTT/ASS 渲染 + 样式模板                        │
│  waveform.rs：PCM16 → 峰值桶                                  │
│  translate.rs：OpenAI 兼容协议 → JSON 数组                     │
│  tools.rs：bin/ 与模型的自动定位                               │
└───────────────────────────────────────────────────────────────┘
```

## 3. 数据模型

```rust
Segment { start: f64, end: f64, text: String }   // 秒

Settings { whisper_path?, model_path?, language?,
           api_provider?, api_model?, target_lang? }  // 存 app_config/settings.json
```

## 4. 核心流水线

1. `transcribe`：ffmpeg 提取 16k mono wav → whisper-cli `-ojf`（兼容新旧 JSON 结构）→ `align::align` 以 10s 上限按标点断句 → 写 `{base}.srt` → 返回 segments。
2. `waveform`：同 wav 提取，解析 PCM16 为 1200 个峰值桶。
3. `export_subtitles`：SRT/VTT/ASS + 内置样式模板。
4. `burn_subtitles`：生成临时 ASS → ffmpeg `ass=filename=...` 滤镜（cwd 指向输出目录，规避 Windows 盘符转义）→ mp4。
5. `translate_segments`：OpenAI 兼容协议，强制 JSON 数组输出，校验条数一致性。

## 5. 安全

- 翻译 API Key 存 Windows Credential Manager（keyring），不明文落盘。
- 工具/模型二进制随包或首次下载；私钥类文件不入库（tools/ 已 gitignore）。

## 6. 打包与模型分发

- `bundle.resources = ["resources/bin/*"]`：ffmpeg/ffprobe/whisper-cli+DLL 随包分发。
- 模型不打包，首次运行按 tiny/base/small 选择下载到 `app_config/models/`，支持 `.part` 断点续传式保存。
- `tools::find` 候选路径：cwd/tools、exe 上级根 tools、exe_dir/resources（安装布局），保证 dev/打包一致。

## 7. 前端交互要点

- 原生拖放 `onDragDropEvent` 拿真实路径；`convertFileSrc` 播放本地视频。
- 波形 canvas：点击定位播放、块边缘拖动改起止时间（带邻居约束）。
- 行内时间输入框 `m:ss.cs` 可直接改；文本 textarea 即改即存。
- 批量队列：`open({multiple:true})` → 顺序调用 transcribe。

## 8. 测试与质量

- `cargo test`：单元测试覆盖 align 断句、export 三种格式、waveform 解析等。
- `cargo clippy --all-targets` 零警告；`npm run build` 通过。
- 端到端验证：`subpipe` 示例跑通 jfk.wav；ffmpeg 烧录用生成测试视频验证。

## 9. 待办（可选）

- 批量烧录、字幕样式自定义编辑器、Word/PDF 导出等增值项。
