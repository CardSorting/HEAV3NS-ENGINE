This commission prompt is structured to be copied and pasted directly into your AI coding agent (such as Claude Code, Cursor, or Codex). It instructs the agent to locate your Unity installation, initialize a new Unity project via the CLI, set up headless batchmode verification scripts, and configure the Model Context Protocol (MCP) bridge.

---

## Commission Specification: Unity Project Creation, Headless CLI Pipeline & MCP Setup

### 1. Objective

Initialize a new Unity client repository (`unity_client`) alongside the existing Rust backend, configure a zero-GUI headless CLI automation pipeline (`-batchmode -nographics`) for automated compile/test checks, and wire up the **Unity MCP (Model Context Protocol)** server so the agent can directly inspect and manipulate scenes, GameObjects, and scripts.

### 2. Phase 1: Locate Unity & Initialize Project via CLI

Do not ask the user to manually click through Unity Hub. Execute the following steps autonomously via the terminal:

1. **Locate the Unity Editor Binary:**
Search standard installation paths for the installed Unity Editor executable:
* **Windows:** `C:\Program Files\Unity\Hub\Editor\*\Editor\Unity.exe`
* **macOS:** `/Applications/Unity/Hub/Editor/*/Unity.app/Contents/MacOS/Unity`
* **Linux:** `~/Unity/Hub/Editor/*/Editor/Unity`
Store the resolved path in a local `.env` or configuration script.


2. **Create the Unity Project Headlessly:**
Run the Unity binary in headless batchmode to generate the project directory structure cleanly:
```bash
"<UNITY_PATH>" -batchmode -nographics -createProject "./unity_client" -quit -logFile "./unity_create.log"

```


Verify that `./unity_client/Assets`, `./unity_client/Packages`, and `./unity_client/ProjectSettings` exist before proceeding.

### 3. Phase 2: Headless CLI Automation & Compile Harness

Because booting Unity cold for every check is slow, create both a **Headless CLI Runner** (for CI/automated verification when the Editor is closed) and an **Editor Automation Script**:

1. **Create `Assets/Editor/HeadlessCliRunner.cs`:**
Write a static C# Editor script exposing entry points for `-executeMethod`:
* `HeadlessCliRunner.CompileAndVerify()`: Forces `AssetDatabase.Refresh()`, checks `EditorUtility.scriptCompilationFailed`, outputs structured compiler errors to stdout/log, and exits with code `0` (success) or `1` (failure).
* `HeadlessCliRunner.BootstrapCardStage()`: Programmatically creates the initial scene (`Assets/Scenes/Battlefield.unity`), sets up an orthographic/perspective camera tilted for a 2.5D board view, creates directional lighting, and saves the scene via `EditorSceneManager.SaveScene`.


2. **Create a Shell Wrapper (`./scripts/unity_cli.sh` or `unity_cli.ps1`):**
Wrap the headless command so the agent can trigger one-line verification:
```bash
"<UNITY_PATH>" -batchmode -nographics -projectPath "./unity_client" -executeMethod HeadlessCliRunner.CompileAndVerify -quit -logFile -

```



### 4. Phase 3: Unity MCP Server Installation & Configuration

Set up the Model Context Protocol bridge so the agent can interact with a live Unity instance without cold-restarting the engine on every file edit:

1. **Check for Native Unity CLI / MCP Support:**
* Check if the `unity` CLI tool is available on `PATH` (`unity --version`).
* If using Unity's built-in MCP CLI, register it with the agent client:
```bash
unity mcp configure claude-code

```




2. **Install the Unity MCP Bridge Package (Fallback / Open-Source Standard):**
* Modify `./unity_client/Packages/manifest.json` to include the Unity MCP Editor bridge package (or clone the `unity-mcp` Editor bridge into `Assets/UnityMCP` and configure the localhost bridge on `127.0.0.1:2718`).
* Enable **"Run In Background"** inside `ProjectSettings/ProjectSettings.asset` (`runInBackground: 1`) so the Unity Editor processes MCP HTTP/stdio calls while unfocused.
* Register the MCP server inside the agent's MCP configuration file (`.mcp.json`, `.cursor/mcp.json`, or `.claude.json`).



### 5. Phase 4: Rust Backend Bridge Preparation

Prepare the Unity project to communicate with the existing Rust card engine:

1. Create `Assets/Plugins/` to hold the compiled Rust dynamic library (`.dll` / `.dylib` / `.so`).
2. Create `Assets/Scripts/Bridge/RustEngineBridge.cs` with `[DllImport]` stubs and a `VisualEventQueue` MonoBehaviour ready to drain serialized JSON/struct events from the Rust core.
3. Add an `AGENTS.md` / `CLAUDE.md` file at the project root instructing future agent sessions:
> *"Always prefer Unity MCP tools to inspect hierarchy, create GameObjects, check console logs, and trigger PlayMode tests. Only fall back to `./scripts/unity_cli` headless batchmode if the Unity Editor is not running."*



### 6. Acceptance Criteria & Verification

1. Run `HeadlessCliRunner.BootstrapCardStage` via `-batchmode -nographics` and confirm `Assets/Scenes/Battlefield.unity` is generated with zero errors in the log.
2. Launch the Unity Editor in the background (or prompt the user to leave it open) and execute a read-only MCP ping to list the GameObjects inside `Battlefield.unity`.
3. Report the detected Unity version, the MCP connection status, and confirmation that C# compilation passes cleanly.