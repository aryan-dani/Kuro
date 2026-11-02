# Kuro STT bench results

Generated: 2026-11-02 14:16:03
Power: {'is_charging': True, 'battery_percent': 56, 'is_battery_present': True, 'ac_line_status': 1}

| Backend | OK | Cold s | Compile s | Warm p50 | Warm p95 | WER | Cmd acc | Idle mW | Busy mW |
|---------|----|--------|-----------|----------|----------|-----|---------|---------|---------|
| fw-medium.en-cuda-float16 | False | - | - | - | - | - | - | - | - |
| fw-small.en-cuda-float16 | True | 0.793 | - | 0.110 | 0.130 | 0.446 | 0.864 | - | - |
| ov-whisper-base.en-int8-NPU | True | 1.117 | 0.726 | 0.093 | 0.107 | 0.467 | 0.727 | - | - |
| ov-whisper-base.en-int8-CPU | True | 0.667 | 0.529 | 0.122 | 0.151 | 0.467 | 0.727 | - | - |
| ov-whisper-base.en-int8-GPU.0 | True | 1.866 | 1.609 | 0.115 | 0.135 | 0.467 | 0.727 | - | - |
| ov-whisper-small.en-int8-NPU | True | 2.354 | 1.794 | 0.266 | 0.299 | 0.467 | 0.818 | - | - |
| ov-whisper-small.en-int8-CPU | True | 1.360 | 0.839 | 0.424 | 0.491 | 0.467 | 0.818 | - | - |
| ov-whisper-small.en-int8-GPU.0 | True | 3.955 | 3.178 | 0.340 | 0.375 | 0.467 | 0.818 | - | - |

## Errors
### fw-medium.en-cuda-float16
```
RuntimeError: medium.en unavailable (incomplete cache): Unable to open file 'model.bin' in model 'C:\Users\dania\.cache\huggingface\hub\models--Systran--faster-whisper-medium.en\snapshots\a29b04bd15381511a9af671baec01072039215e3'
Traceback (most recent call last):
  File "scripts\bench\run_stt.py", line 173, in load
    self.model = WhisperModel(
                 ^^^^^^^^^^^^^
  File "C:\Users\dania\Documents\Stuff\My_Repositories\Domain_Based\Agentic_AI\Kuro\.venv-npu\Lib\site-packages\faster_whisper\transcribe.py", line 689, in __init__
    self.model = ctranslate2.models.Whisper(
                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
RuntimeError: Unable to open file 'model.bin' in model 'C:\Users\dania\.cache\huggingface\hub\models--Systran--faster-whisper-medium.en\snapshots\a29b04bd15381511a9af671baec01072039215e3'

The above exception was the direct cause of the following exception:

Traceback (most recent call last):
  File "scripts\bench\run_stt.py", line 266, in evaluate_backend
    cold = backend.load()
           ^^^^^^^^^^^^^^
  File "scripts\bench\run_stt.py", line 181, in load
    raise RuntimeError(f"medium.en unavailable (incomplete cache): {e}") from e
RuntimeError: medium.en unavailable (incomplete cache): Unable to open file 'model.bin' in model 'C:\Users\dania\.cache\huggingface\hub\models--Systran--faster-whisper-medium.en\snapshots\a29b04bd15381511a9af671baec01072039215e3'

```

## NPU stability (50 runs)
- ov-whisper-base.en-int8-NPU: `{"ok": true, "runs": 50, "errors": []}`

## Gate rule
NPU wins on battery if warm p95 (clips <=5s) < 1.5s, command accuracy within 3 points of CUDA baseline, and 50-run check did not crash.