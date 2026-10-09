# vidsub — 本地视频一键转字幕

桌面应用：拖入视频 → 本地 Whisper 识别 → 生成 SRT/ASS/VTT 字幕 → 可选翻译/烧录。全程本地处理，隐私不出机器，**全功能免费**。

## 功能

- 导入：拖放/选择视频音频，支持批量队列
- 识别：whisper.cpp 本地推理，自动语言检测，无时长限制
- 时间轴：波形图 + 可编辑时间/文本，点击定位，拖动改起止时间
- 导出：SRT / VTT / ASS（内置 4 套样式模板）
- 翻译：DeepSeek / OpenAI BYOK（中英日韩）
- 烧录：ffmpeg 字幕嵌入导出 mp4
- 模型：tiny / base / small 可选首启下载

## 技术栈

- Tauri 2 + Rust + Vite（原生 JS）
- whisper.cpp（whisper-cli）本地推理
- ffmpeg / ffprobe 音轨提取与烧录
- OpenAI 兼容协议翻译（自带 Key）

## 目录结构

```
src/                前端（index.html, main.js, style.css）
src-tauri/src/      Rust：lib.rs 命令入口 + 模块
  audio.rs          音轨提取 / 时长探测
  whisper.rs        whisper-cli 调用与 JSON 解析
  align.rs          标点断句时间轴对齐
  export.rs         SRT/VTT/ASS 渲染与样式模板
  translate.rs      翻译 BYOK
  waveform.rs       PCM16 波形峰值提取
  tools.rs          工具与模型自动定位
src-tauri/resources/bin/  打包附带 ffmpeg/whisper 二进制
src-tauri/examples/subpipe.rs        命令行核心链路
tools/              开发用二进制与模型（不入库）
```

## 开发

要求：Rust 1.98+、Node 20+、WebView2（Windows 自带）。

```bash
npm install
npm run tauri dev
```

命令行验证核心链路：

```bash
cargo run --example subpipe -- <媒体文件> <输出目录>
```

## 构建

```bash
npx tauri build --debug   # 或去掉 --debug 出 release 安装包
```

产物位于 `src-tauri/target/*/bundle/`（NSIS/msi）。ffmpeg/whisper 随包分发，模型首次运行按需下载。

## 环境变量

- `VIDSUB_WHISPER` / `VIDSUB_MODEL`：覆盖 whisper / 模型路径

## 测试

```bash
cargo test
cargo clippy --all-targets
```