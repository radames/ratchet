# Ratchet

### A web-first, cross-platform ML developer toolkit.

[Documentation](https://hf.co)    |     [Discord](https://discord.gg/XFe33KQTG4)

---

**Ship AI inference to your Web, Electron or Tauri apps with ease.**

## Getting Started

Check out our [HuggingFace space](https://huggingface.co/spaces/FL33TW00D-HF/ratchet-whisper) for a live demo!

```javascript
// Asynchronous loading & caching with IndexedDB
let model = await Model.load(AvailableModels.WHISPER_TINY, Quantization.Q8, (p: number) => setProgress(p))
let result = await model.run({ input });
```

## Philosophy

We want a toolkit for developers to make integrating performant AI functionality into existing production applications easy.
The following principles will help us accomplish this:
1. **Inference only**
2. **WebGPU/CPU only**
3. First class quantization support
4. Lazy computation
5. Inplace by default

Any issues regarding training or different backends will be closed!

## Supported Models
- Whisper


