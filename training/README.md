# Legion Training Dry-Run Scaffold

Phase 8 keeps training opt-in and consent-gated. The checked-in Python entrypoints validate and print operator plans without importing GPU training libraries by default.

Dry-run commands:

```sh
python3 training/qlora_train.py --dry-run
python3 training/convert_to_gguf.py --dry-run
```

Real training requires an operator-provisioned environment, consented trace exports, redaction/secret-scan evidence, and an explicit model run record.
