# HEAV3NS-ENGINE — Unity Client & Rust Backend Agent Instructions

> Always prefer Unity MCP tools for live Editor work. Do not infer that Unity is stopped solely because port 2718 is unavailable: the native Unity MCP and the local bridge are independent.

## Editor and MCP startup

1. Run `./scripts/unity_ready` to check the native Pipeline and a live Editor command, and try the `unity-editor-mcp` tools. Unity Hub alone is not the Editor. A server handshake or tool list alone does not prove the Editor is connected.
2. If the Editor is not running and live Editor work is needed, run `./scripts/unity_ready --start`. Allow time for import and compilation.
3. Codex uses the globally registered `unity-editor-mcp` stdio server; a new Codex session may be needed after configuration changes. The project `.mcp.json` is for compatible MCP clients and does not itself register a server with Codex.
4. If the Editor is running but MCP is unavailable, inspect `unity_client/Logs/Editor.log` (or `~/Library/Logs/Unity/Editor.log` during early startup), compilation errors, and the MCP configuration. As a fallback for live automation, use `/Applications/Unity\ Hub.app/Contents/Resources/unity command --project-path "$PWD/unity_client"` and consult `unity command --help` or `unity skill show`. The optional local bridge can be started from **HEAV3NS > MCP > Start Localhost Bridge**. Do not start a second batchmode instance against an open project.
5. Use `./scripts/unity_cli` headless batchmode only when the Editor is confirmed closed and headless work is appropriate. Report startup or licensing blockers accurately.

## Quick Reference

- **Unity Editor Binary**: `/Applications/Unity/Hub/Editor/6000.6.7f1/Unity.app/Contents/MacOS/Unity` (`6000.6.7f1`)
- **Unity Project Path**: `./unity_client`
- **Primary Scene**: `./unity_client/Assets/Scenes/Battlefield.unity`
- **Unity MCP Server**:
  - Native Unity CLI MCP (`unity-editor-mcp`): `/Applications/Unity Hub.app/Contents/Resources/unity mcp --project-path /Users/bozoegg/Desktop/HEAV3NS-ENGINE/unity_client` (`com.unity.pipeline@0.9.1-exp.1`)
  - Optional custom HTTP bridge: `http://127.0.0.1:2718/` (`Assets/UnityMCP/Editor/UnityMcpEditorBridge.cs`). Its MCP implementation is limited; use the native server for MCP.
- **Headless CLI Verification Fallback**:
  - Compile & Verify C# Scripts: `./scripts/unity_cli.sh HeadlessCliRunner.CompileAndVerify` (or `./scripts/unity_cli`)
  - Bootstrap Battlefield Scene: `./scripts/unity_cli.sh HeadlessCliRunner.BootstrapCardStage`
- **Rust Backend Bridge**:
  - Native Dynamic Library: `./unity_client/Assets/Plugins/libheav3ns_engine.dylib` (built via `cargo build --release -p heav3ns-engine`)
  - C# P/Invoke & Event Queue: `./unity_client/Assets/Scripts/Bridge/RustEngineBridge.cs` (`RustEngineBridge`) and `./unity_client/Assets/Scripts/Bridge/VisualEventQueue.cs` (`VisualEventQueue`)


