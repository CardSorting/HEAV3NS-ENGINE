# HEAV3NS-ENGINE — Unity Client & Rust Backend Agent Instructions

> *"Always prefer Unity MCP tools to inspect hierarchy, create GameObjects, check console logs, and trigger PlayMode tests. Only fall back to `./scripts/unity_cli` headless batchmode if the Unity Editor is not running."*

## Quick Reference

- **Unity Editor Binary**: `/Applications/Unity/Hub/Editor/6000.6.7f1/Unity.app/Contents/MacOS/Unity` (`6000.6.7f1`)
- **Unity Project Path**: `./unity_client`
- **Primary Scene**: `./unity_client/Assets/Scenes/Battlefield.unity`
- **Unity MCP Server**:
  - Native Unity CLI MCP (`unity-editor-mcp`): `/Applications/Unity Hub.app/Contents/Resources/unity mcp --project-path /Users/bozoegg/Desktop/HEAV3NS-ENGINE/unity_client` (`com.unity.pipeline@0.9.1-exp.1`)
  - Localhost Editor HTTP Bridge (`unity-mcp-bridge`): `http://127.0.0.1:2718/mcp` (`Assets/UnityMCP/Editor/UnityMcpEditorBridge.cs`)
- **Headless CLI Verification Fallback**:
  - Compile & Verify C# Scripts: `./scripts/unity_cli.sh HeadlessCliRunner.CompileAndVerify` (or `./scripts/unity_cli`)
  - Bootstrap Battlefield Scene: `./scripts/unity_cli.sh HeadlessCliRunner.BootstrapCardStage`
- **Rust Backend Bridge**:
  - Native Dynamic Library: `./unity_client/Assets/Plugins/libheav3ns_engine.dylib` (built via `cargo build --release -p heav3ns-engine`)
  - C# P/Invoke & Event Queue: `./unity_client/Assets/Scripts/Bridge/RustEngineBridge.cs` (`RustEngineBridge`) and `./unity_client/Assets/Scripts/Bridge/VisualEventQueue.cs` (`VisualEventQueue`)


