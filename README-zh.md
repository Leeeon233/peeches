<p align="center">
    <img
        width="96px"
        alt="Vibe logo"
        src="./src-tauri/icons/Square310x310Logo.png"
    />
</p>

<h1 align="center">Peeches  </h1>
<h3 align="center"> 实时系统音频转录和翻译 </h3>

<p align="center">
    <a href="./README.md">English</a> | <a href="./README-zh.md">中文</a>
</p>

![Image](https://github.com/user-attachments/assets/b5b0692b-bde5-4c3f-8284-7545f0846333)

# 功能

- 🎙️ 实时转录系统音频
- 💻 支持 macOS 和 Windows
- 🤖 完全本地的 AI 模型
- 🎵 歌词样式的文本显示
- 🦀 用纯 Rust 编写
- 🌐 目前仅支持英语到中文的翻译

# 模型

- whisper: https://huggingface.co/ggerganov/whisper.cpp
- opus-mt-en-zh: https://huggingface.co/Helsinki-NLP/opus-mt-en-zh

# 致谢

- [tauri](https://tauri.app/): 使用 Web 前端构建更小、更快、更安全的桌面和移动应用程序。
- [whisper-rs](https://github.com/tazz4843/whisper-rs): Rust 绑定到 https://github.com/ggerganov/whisper.cpp
- [candle](https://github.com/huggingface/candle): Rust 的极简主义 ML 框架
