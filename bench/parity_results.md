# STT parity re-bench

{
  "generated": "2026-11-02 14:32:26",
  "note": "OV parity with stt._load_and_prep + language/en + beam + initial_prompt",
  "prior_gap": "Original matrix used raw audio, default num_beams=1, no initial_prompt"
}

- **ov-whisper-small.en-int8-NPU-parity-beam5**: ok=True p95=0.318 acc=0.818 (same cmd acc as non-parity; p95 ~+20ms)
- **ov-whisper-small.en-int4-NPU**: compiles (~25–81s) but `generate` fails (`infer_request` ROI / lang_to_id). **Do not ship int4 Whisper.**

After `phrases.py` safe variants + word-boundary `on`/`off`, rescored stored hyps:
- CUDA small: 20/22 → **0.909** (remaining: `It's fun.` garbage for desk-on)
- NPU small: 21/22 → **0.955** (same remaining garbage clip)
