# Deploy tarang to edge fleet

## Target
- role: edge
- arch: aarch64

## Tasks
- Install `tarang` version `2026.3.18` via ark
- Enable `tarang.service` via argonaut
- Verify port 8070 is listening
- Report status to daimon
