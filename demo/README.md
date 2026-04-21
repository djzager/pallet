# Demo Recording

Scripted terminal recording of pallet in action, generated with
[VHS](https://github.com/charmbracelet/vhs).

## Prerequisites

```bash
brew install vhs tree
```

## Generate the recording

```bash
./demo/record.sh
```

This will:
1. Build pallet (`cargo build --release`)
2. Create temporary git repos with sample governance rules and skills
3. Record the demo via VHS
4. Output `demo/demo.gif` and `demo/demo.mp4`
5. Clean up the temporary environment

## Files

| File | Purpose |
|------|---------|
| `record.sh` | Orchestrator — build, setup, record, cleanup |
| `setup.sh` | Creates temp git repos and workspace with fixture data |
| `demo.tape` | VHS tape template (scripted terminal commands) |
| `fixtures/` | Sample governance rules and skills for the demo |
