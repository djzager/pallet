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
2. Create a temp workspace with the project's `pallet.yaml`
3. Sync from [gwenneg/claude-engineering-toolkit](https://github.com/gwenneg/claude-engineering-toolkit) (requires internet)
4. Record the demo via VHS
5. Output `demo/demo.gif` and `demo/demo.mp4`
6. Clean up the temporary environment

## Files

| File | Purpose |
|------|---------|
| `record.sh` | Orchestrator — build, setup, record, cleanup |
| `setup.sh` | Creates temp workspace with real pallet.yaml |
| `demo.tape` | VHS tape template (scripted terminal commands) |
